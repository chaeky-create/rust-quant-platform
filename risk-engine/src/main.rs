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
    execution_fills_received: IntCounter,
    risk_snapshots_generated: IntCounter,
    risk_warnings_total: IntCounter,
    risk_breaches_total: IntCounter,
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

    let execution_fills_received = IntCounter::new(
        "risk_engine_execution_fills_received_total",
        "Total execution fill events received",
    )
    .unwrap();

    let risk_snapshots_generated = IntCounter::new(
        "risk_engine_snapshots_generated_total",
        "Total risk snapshots generated",
    )
    .unwrap();

    let risk_warnings_total = IntCounter::new(
        "risk_engine_warnings_total",
        "Total WARNING risk states",
    )
    .unwrap();

    let risk_breaches_total = IntCounter::new(
        "risk_engine_breaches_total",
        "Total LIMIT_BREACH or LOSS_LIMIT risk states",
    )
    .unwrap();

    registry
        .register(Box::new(execution_fills_received.clone()))
        .unwrap();

    registry
        .register(Box::new(risk_snapshots_generated.clone()))
        .unwrap();

    registry
        .register(Box::new(risk_warnings_total.clone()))
        .unwrap();

    registry
        .register(Box::new(risk_breaches_total.clone()))
        .unwrap();

    let state = AppState {
        registry: Arc::new(registry),
        execution_fills_received,
        risk_snapshots_generated,
        risk_warnings_total,
        risk_breaches_total,
    };

    let risk_tx = tx.clone();
    let risk_state = state.clone();
    
    tokio::spawn(async move {
        run_risk_engine(risk_tx, risk_state).await;
    });

   let app = Router::new()
    .route(
        "/ws/risk",
        get(move |ws: WebSocketUpgrade| handle_ws(ws, tx.subscribe())),
    )
    .route("/metrics", get(metrics_endpoint))
    .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 9301));

    println!("Risk WebSocket running on ws://127.0.0.1:9301/ws/risk");
    println!("Risk engine subscribing to Redis channel execution:fills");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind risk server");

    axum::serve(listener, app)
        .await
        .expect("Risk server failed");
}

async fn publish_to_redis(redis_client: &redis::Client, channel: &str, message: String) {
    if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
        let _: redis::RedisResult<()> = conn.publish(channel, message).await;
    }
}

async fn run_risk_engine(
    tx: broadcast::Sender<String>,
    state: AppState,
) {
        println!("Connecting risk engine to Redis...");

    let client = redis::Client::open("redis://127.0.0.1:6379/")
        .expect("Failed to create Redis client");

    let mut pubsub = client
        .get_async_pubsub()
        .await
        .expect("Failed to connect to Redis pubsub");

    pubsub
        .subscribe("execution:fills")
        .await
        .expect("Failed to subscribe to execution:fills");

    println!("Risk engine subscribed to Redis execution:fills.");

    let mut stream = pubsub.on_message();

    while let Some(message) = stream.next().await {
        let Ok(text): Result<String, _> = message.get_payload() else {
            continue;
        };

        state.execution_fills_received.inc();

        let Ok(data) = serde_json::from_str::<Value>(&text) else {
            continue;
        };

        let qty = data["qty"].as_f64().unwrap_or(0.0);
        let mark = data["mark"].as_f64().unwrap_or(0.0);
        let unrealized_pnl = data["unrealized_pnl"].as_f64().unwrap_or(0.0);
        let realized_pnl = data["realized_pnl"].as_f64().unwrap_or(0.0);

        let notional_exposure = qty.abs() * mark;
        let total_pnl = realized_pnl + unrealized_pnl;

        let max_notional_limit = 250_000.0;
        let max_loss_limit = -2_500.0;

        let exposure_utilization = if max_notional_limit > 0.0 {
            notional_exposure / max_notional_limit
        } else {
            0.0
        };

        let risk_state = if notional_exposure > max_notional_limit {
            "LIMIT_BREACH"
        } else if total_pnl < max_loss_limit {
            "LOSS_LIMIT"
        } else if exposure_utilization > 0.75 {
            "WARNING"
        } else {
            "OK"
        };

        let risk_state = if notional_exposure > max_notional_limit {
            "LIMIT_BREACH"
        } else if total_pnl < max_loss_limit {
            "LOSS_LIMIT"
        } else if exposure_utilization > 0.75 {
            "WARNING"
        } else {
            "OK"
        };
        
        if risk_state == "WARNING" {
            state.risk_warnings_total.inc();
        }
        
        if risk_state == "LIMIT_BREACH" || risk_state == "LOSS_LIMIT" {
            state.risk_breaches_total.inc();
        }
        
        let var_95 = notional_exposure * 0.02 * 1.65;
        let expected_shortfall = notional_exposure * 0.02 * 2.06;

        let var_95 = notional_exposure * 0.02 * 1.65;
        let expected_shortfall = notional_exposure * 0.02 * 2.06;

        let payload = json!({
            "risk_state": risk_state,
            "qty": qty,
            "mark": mark,
            "notional_exposure": notional_exposure,
            "exposure_utilization": exposure_utilization,
            "realized_pnl": realized_pnl,
            "unrealized_pnl": unrealized_pnl,
            "total_pnl": total_pnl,
            "var_95": var_95,
            "expected_shortfall": expected_shortfall
        });

        let risk_text = payload.to_string();
        
        state.risk_snapshots_generated.inc();

        println!("{}", payload);

        let _ = tx.send(risk_text.clone());

        publish_to_redis(&client, "risk:snapshots", risk_text).await;
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