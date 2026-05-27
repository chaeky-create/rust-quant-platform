use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures_util::StreamExt;
use prometheus::{Encoder, IntCounter, Registry, TextEncoder};
use redis::AsyncCommands;
use serde_json::{json, Value};
use shared_types::MarketTick;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;

#[derive(Clone)]
struct AppState {
    registry: Arc<Registry>,
    signals_generated: IntCounter,
    redis_ticks_received: IntCounter,
}

async fn metrics_endpoint(State(state): State<AppState>) -> Response {
    let encoder = TextEncoder::new();
    let metric_families = state.registry.gather();

    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    Response::builder()
        .header("Content-Type", encoder.format_type())
        .body(axum::body::Body::from(buffer))
        .unwrap()
}

#[tokio::main]
async fn main() {
    let (tx, _) = broadcast::channel::<String>(1000);

    let registry = Registry::new();

let signals_generated = IntCounter::new(
    "strategy_engine_signals_generated_total",
    "Total strategy signals generated",
)
.unwrap();

let redis_ticks_received = IntCounter::new(
    "strategy_engine_redis_ticks_received_total",
    "Total Redis tick messages received",
)
.unwrap();

registry
    .register(Box::new(signals_generated.clone()))
    .unwrap();

registry
    .register(Box::new(redis_ticks_received.clone()))
    .unwrap();

let state = AppState {
    registry: Arc::new(registry),
    signals_generated,
    redis_ticks_received,
};

    let strategy_tx = tx.clone();
    let strategy_state = state.clone();

    tokio::spawn(async move {
        run_strategy_engine(strategy_tx, strategy_state).await;
    });

    let app = Router::new()
    .route(
        "/ws/strategy",
        get(move |ws: WebSocketUpgrade| handle_ws(ws, tx.subscribe())),
    )
    .route("/metrics", get(metrics_endpoint))
    .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 9101));

    println!("Strategy WebSocket running on ws://127.0.0.1:9101/ws/strategy");
    println!("Strategy engine subscribing to Redis channel market-data:ticks");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind strategy server");

    axum::serve(listener, app)
        .await
        .expect("Strategy server failed");
}

async fn run_strategy_engine(
    tx: broadcast::Sender<String>,
    state: AppState,
) {
        println!("Connecting strategy engine to Redis...");

    let client = redis::Client::open("redis://127.0.0.1:6379/")
        .expect("Failed to create Redis client");

    let mut pubsub = client
        .get_async_pubsub()
        .await
        .expect("Failed to connect to Redis pubsub");

    pubsub
        .subscribe("market-data:ticks")
        .await
        .expect("Failed to subscribe to market-data:ticks");

    println!("Strategy engine subscribed to Redis market-data:ticks.");

    let mut stream = pubsub.on_message();
    let mut prices: Vec<f64> = Vec::new();

    while let Some(msg) = stream.next().await {
        let Ok(text): Result<String, _> = msg.get_payload() else {
            continue;
        };
        state.redis_ticks_received.inc();

        let Ok(json_value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };

        if json_value["event_type"] != "tick" {
            continue;
        }

        let Ok(tick) = serde_json::from_value::<MarketTick>(json_value["data"].clone()) else {
            continue;
        };

        prices.push(tick.price);

        if prices.len() > 50 {
            prices.remove(0);
        }

        if prices.len() < 20 {
            continue;
        }

        let short_ma = prices[prices.len() - 5..].iter().sum::<f64>() / 5.0;
        let long_ma = prices[prices.len() - 20..].iter().sum::<f64>() / 20.0;

        let signal = if short_ma > long_ma {
            "LONG"
        } else if short_ma < long_ma {
            "SHORT"
        } else {
            "FLAT"
        };

        let payload = json!({
            "signal": signal,
            "price": tick.price,
            "short_ma": short_ma,
            "long_ma": long_ma,
            "timestamp": tick.timestamp
        });

        let signal_text = payload.to_string();
        state.signals_generated.inc();

        let _ = tx.send(signal_text.clone());

        if let Ok(mut conn) = client.get_multiplexed_async_connection().await {
            let _: redis::RedisResult<()> = conn
                .publish("strategy:signals", signal_text)
                .await;
        }

        println!("{}", payload);
    }
}

async fn handle_ws(
    ws: WebSocketUpgrade,
    rx: broadcast::Receiver<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket_loop(socket, rx))
}

async fn websocket_loop(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
}