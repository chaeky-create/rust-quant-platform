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
    redis_signals_received: IntCounter,
    execution_events: IntCounter,
    fills_generated: IntCounter,
} 

#[derive(Debug, Clone)]
struct Position {
    qty: f64,
    avg_price: f64,
    realized_pnl: f64,
}

impl Position {
    fn new() -> Self {
        Self {
            qty: 0.0,
            avg_price: 0.0,
            realized_pnl: 0.0,
        }
    }

    fn buy(&mut self, price: f64, qty: f64) {
        let new_qty = self.qty + qty;

        self.avg_price = if new_qty.abs() > 0.0 {
            (self.avg_price * self.qty + price * qty) / new_qty
        } else {
            0.0
        };

        self.qty = new_qty;
    }

    fn sell(&mut self, price: f64, qty: f64) {
        if self.qty > 0.0 {
            let closing_qty = qty.min(self.qty);
            self.realized_pnl += closing_qty * (price - self.avg_price);
        }

        self.qty -= qty;

        if self.qty.abs() < 1e-9 {
            self.qty = 0.0;
            self.avg_price = 0.0;
        }
    }

    fn unrealized_pnl(&self, mark: f64) -> f64 {
        self.qty * (mark - self.avg_price)
    }
}

async fn publish_to_redis(redis_client: &redis::Client, channel: &str, message: String) {
    if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
        let _: redis::RedisResult<()> = conn.publish(channel, message).await;
    }
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

    let redis_signals_received = IntCounter::new(
        "execution_engine_redis_signals_received_total",
        "Total Redis strategy signals received",
    )
    .unwrap();

    let execution_events = IntCounter::new(
        "execution_engine_events_total",
        "Total execution events emitted",
    )
    .unwrap();

    let fills_generated = IntCounter::new(
        "execution_engine_fills_generated_total",
        "Total non-NONE fills generated",
    )
    .unwrap();

    registry
        .register(Box::new(redis_signals_received.clone()))
        .unwrap();

    registry
        .register(Box::new(execution_events.clone()))
        .unwrap();

    registry
        .register(Box::new(fills_generated.clone()))
        .unwrap();

    let state = AppState {
        registry: Arc::new(registry),
        redis_signals_received,
        execution_events,
        fills_generated,
    };

    let execution_tx = tx.clone();
    let execution_state = state.clone();
    
    tokio::spawn(async move {
        run_execution_engine(execution_tx, execution_state).await;
    });

    let app = Router::new()
    .route(
        "/ws/execution",
        get(move |ws: WebSocketUpgrade| handle_ws(ws, tx.subscribe())),
    )
    .route("/metrics", get(metrics_endpoint))
    .with_state(state);


    let addr = SocketAddr::from(([127, 0, 0, 1], 9201));

    println!("Execution WebSocket running on ws://127.0.0.1:9201/ws/execution");
    println!("Execution engine subscribing to Redis channel strategy:signals");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind execution server");

    axum::serve(listener, app)
        .await
        .expect("Execution server failed");
}

async fn run_execution_engine(
    tx: broadcast::Sender<String>,
    state: AppState,
) {
        println!("Connecting execution engine to Redis...");

    let client = redis::Client::open("redis://127.0.0.1:6379/")
        .expect("Failed to create Redis client");

    let mut pubsub = client
        .get_async_pubsub()
        .await
        .expect("Failed to connect to Redis pubsub");

    pubsub
        .subscribe("strategy:signals")
        .await
        .expect("Failed to subscribe to strategy:signals");

    println!("Execution engine subscribed to Redis strategy:signals.");

    let mut stream = pubsub.on_message();

    let mut position = Position::new();
    let mut last_signal = String::from("FLAT");

    while let Some(message) = stream.next().await {
        let Ok(text): Result<String, _> = message.get_payload() else {
            continue;
        };
        state.redis_signals_received.inc();

        let Ok(json_value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };

        let signal = json_value["signal"].as_str().unwrap_or("FLAT");
        let price = json_value["price"].as_f64().unwrap_or(0.0);
        let timestamp = json_value["timestamp"].as_str().unwrap_or("");

        if price <= 0.0 {
            continue;
        }

        let mut fill = "NONE";

        if signal != last_signal {
            match signal {
                "LONG" => {
                    position.buy(price, 1.0);
                    fill = "BUY";
                }
                "SHORT" => {
                    position.sell(price, 1.0);
                    fill = "SELL";
                }
                _ => {}
            }

            last_signal = signal.to_string();
        }

        let payload = json!({
            "signal": signal,
            "fill": fill,
            "qty": position.qty,
            "avg_price": position.avg_price,
            "mark": price,
            "realized_pnl": position.realized_pnl,
            "unrealized_pnl": position.unrealized_pnl(price),
            "timestamp": timestamp
        });

        let execution_text = payload.to_string();

        state.execution_events.inc();
        
        if fill != "NONE" {
            state.fills_generated.inc();
        }
        
        println!("{}", payload);
        
        let _ = tx.send(execution_text.clone());
        
        publish_to_redis(&client, "execution:fills", execution_text).await;
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