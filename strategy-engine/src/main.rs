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

fn parse_features(json_value: &Value) -> Option<FeatureSnapshot> {
    Some(FeatureSnapshot {
        price: json_value["price"].as_f64()?,
        return_1: json_value["return_1"].as_f64().unwrap_or(0.0),
        return_5: json_value["return_5"].as_f64().unwrap_or(0.0),
        volatility_20: json_value["volatility_20"].as_f64().unwrap_or(0.0),
        short_ma: json_value["short_ma"].as_f64().unwrap_or(0.0),
        long_ma: json_value["long_ma"].as_f64().unwrap_or(0.0),
        trend_strength: json_value["trend_strength"].as_f64().unwrap_or(0.0),
        spread: json_value["spread"].as_f64().unwrap_or(0.0),
        orderbook_imbalance: json_value["orderbook_imbalance"].as_f64().unwrap_or(0.0),
        microprice: json_value["microprice"].as_f64().unwrap_or(0.0),
        regime: json_value["regime"].as_str().unwrap_or("UNKNOWN").to_string(),
        timestamp: json_value["timestamp"].as_str().unwrap_or("").to_string(),
    })
}

fn decide_signal(features: &FeatureSnapshot) -> &'static str {
    let trend_up = features.short_ma > features.long_ma;
    let trend_down = features.short_ma < features.long_ma;

    let low_vol = features.volatility_20 < 0.01;
    let spread_ok = features.spread < features.price * 0.0005;

    let book_buy_support = features.orderbook_imbalance > 0.08;
    let book_sell_support = features.orderbook_imbalance < -0.08;

    let momentum_up = features.return_5 > 0.0002;
    let momentum_down = features.return_5 < -0.0002;

    if low_vol && spread_ok && trend_up && momentum_up && book_buy_support {
        "LONG"
    } else if low_vol && spread_ok && trend_down && momentum_down && book_sell_support {
        "SHORT"
    } else {
        "FLAT"
    }
}

#[derive(Debug)]
struct FeatureSnapshot {
    price: f64,
    return_1: f64,
    return_5: f64,
    volatility_20: f64,
    short_ma: f64,
    long_ma: f64,
    trend_strength: f64,
    spread: f64,
    orderbook_imbalance: f64,
    microprice: f64,
    regime: String,
    timestamp: String,
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
    println!("Connecting strategy engine to Redis features...");

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string());

    let client = redis::Client::open(redis_url)
        .expect("Failed to create Redis client");

    let mut pubsub = client
        .get_async_pubsub()
        .await
        .expect("Failed to connect to Redis pubsub");

    pubsub
        .subscribe("features:latest")
        .await
        .expect("Failed to subscribe to features:latest");

    println!("Strategy engine subscribed to Redis features:latest.");

    let mut stream = pubsub.on_message();

    while let Some(msg) = stream.next().await {
        let Ok(text): Result<String, _> = msg.get_payload() else {
            continue;
        };

        state.redis_ticks_received.inc();

        let Ok(json_value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };

        let Some(features) = parse_features(&json_value) else {
            continue;
        };

        let signal = decide_signal(&features);

        let payload = json!({
            "signal": signal,
            "price": features.price,
            "short_ma": features.short_ma,
            "long_ma": features.long_ma,
            "trend_strength": features.trend_strength,
            "volatility_20": features.volatility_20,
            "return_1": features.return_1,
            "return_5": features.return_5,
            "spread": features.spread,
            "orderbook_imbalance": features.orderbook_imbalance,
            "microprice": features.microprice,
            "regime": features.regime,
            "timestamp": features.timestamp
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