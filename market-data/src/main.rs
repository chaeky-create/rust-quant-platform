use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use prometheus::{Encoder, IntCounter, Registry, TextEncoder};
use redis::AsyncCommands;
use serde_json::json;
use shared_types::{Candle, MarketTick, OrderBookLevel, OrderBookSnapshot};
use std::{cmp::Ordering, collections::BTreeMap, net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Price(f64);

impl Eq for Price {}

impl PartialOrd for Price {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for Price {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

async fn publish_to_redis(redis_client: &redis::Client, channel: &str, message: String) {
    if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
        let _: redis::RedisResult<()> = conn.publish(channel, message).await;
    }
}

#[derive(Clone)]
struct AppState {
    registry: Arc<Registry>,
    tick_events: IntCounter,
    candle_events: IntCounter,
    orderbook_events: IntCounter,
}

#[tokio::main]
async fn main() {
    let (tx, _) = broadcast::channel::<String>(10_000);

    let redis_client = redis::Client::open("redis://127.0.0.1:6379/")
        .expect("Failed to create Redis client");

        let registry = Registry::new();

        let tick_events = IntCounter::new(
            "market_data_tick_events_total",
            "Total tick events processed",
        )
        .unwrap();
        
        let candle_events = IntCounter::new(
            "market_data_candle_events_total",
            "Total candle events processed",
        )
        .unwrap();
        
        let orderbook_events = IntCounter::new(
            "market_data_orderbook_events_total",
            "Total order book events processed",
        )
        .unwrap();
        
        registry.register(Box::new(tick_events.clone())).unwrap();
        registry.register(Box::new(candle_events.clone())).unwrap();
        registry
            .register(Box::new(orderbook_events.clone()))
            .unwrap();
        
        let state = AppState {
            registry: Arc::new(registry),
            tick_events,
            candle_events,
            orderbook_events,
        };

    let feed_tx = tx.clone();
    let feed_redis = redis_client.clone();

    let feed_state = state.clone();

tokio::spawn(async move {
    run_coinbase_feed(feed_tx, feed_redis, feed_state).await;
});

let app = Router::new()
.route(
    "/ws/btc",
    get(move |ws: WebSocketUpgrade| handle_ws(ws, tx.subscribe())),
)
.route("/metrics", get(metrics_endpoint))
.with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 9001));

    println!("Market data WebSocket server running on ws://127.0.0.1:9001/ws/btc");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind market-data server");

    axum::serve(listener, app)
        .await
        .expect("Market-data server failed");
}

async fn run_coinbase_feed(
    tx: broadcast::Sender<String>,
    redis_client: redis::Client,
    state: AppState,
) {
        let url = "wss://ws-feed.exchange.coinbase.com";

    println!("Connecting to Coinbase BTC-USD ticker + level2 stream...");

    let (mut ws_stream, _) = connect_async(url)
        .await
        .expect("Failed to connect to Coinbase websocket");

    let subscribe = json!({
        "type": "subscribe",
        "product_ids": ["BTC-USD"],
        "channels": ["ticker", "level2"]
    });

    ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Text(
            subscribe.to_string(),
        ))
        .await
        .expect("Failed to subscribe");

    println!("Connected to Coinbase ticker + level2.");

    let mut current_candle: Option<Candle> = None;
    let mut bids: BTreeMap<Price, f64> = BTreeMap::new();
    let mut asks: BTreeMap<Price, f64> = BTreeMap::new();

    while let Some(message) = ws_stream.next().await {
        let Ok(message) = message else {
            continue;
        };

        if !message.is_text() {
            continue;
        }

        let Ok(text) = message.to_text() else {
            continue;
        };

        let Ok(json_value) = serde_json::from_str::<serde_json::Value>(text) else {
            continue;
        };

        let Some(msg_type) = json_value["type"].as_str() else {
            continue;
        };

        match msg_type {
            "snapshot" => {
                bids.clear();
                asks.clear();

                if let Some(bid_arr) = json_value["bids"].as_array() {
                    for item in bid_arr.iter().take(80) {
                        let price = item[0]
                            .as_str()
                            .unwrap_or("0")
                            .parse::<f64>()
                            .unwrap_or(0.0);

                        let size = item[1]
                            .as_str()
                            .unwrap_or("0")
                            .parse::<f64>()
                            .unwrap_or(0.0);

                        if price > 0.0 && size > 0.0 {
                            bids.insert(Price(price), size);
                        }
                    }
                }

                if let Some(ask_arr) = json_value["asks"].as_array() {
                    for item in ask_arr.iter().take(80) {
                        let price = item[0]
                            .as_str()
                            .unwrap_or("0")
                            .parse::<f64>()
                            .unwrap_or(0.0);

                        let size = item[1]
                            .as_str()
                            .unwrap_or("0")
                            .parse::<f64>()
                            .unwrap_or(0.0);

                        if price > 0.0 && size > 0.0 {
                            asks.insert(Price(price), size);
                        }
                    }
                }

                publish_order_book(&tx, &redis_client, &state, &bids, &asks).await;
            }

            "l2update" => {
                if let Some(changes) = json_value["changes"].as_array() {
                    for change in changes {
                        let side = change[0].as_str().unwrap_or("");

                        let price = change[1]
                            .as_str()
                            .unwrap_or("0")
                            .parse::<f64>()
                            .unwrap_or(0.0);

                        let size = change[2]
                            .as_str()
                            .unwrap_or("0")
                            .parse::<f64>()
                            .unwrap_or(0.0);

                        if price <= 0.0 {
                            continue;
                        }

                        match side {
                            "buy" => {
                                if size == 0.0 {
                                    bids.remove(&Price(price));
                                } else {
                                    bids.insert(Price(price), size);
                                }
                            }
                            "sell" => {
                                if size == 0.0 {
                                    asks.remove(&Price(price));
                                } else {
                                    asks.insert(Price(price), size);
                                }
                            }
                            _ => {}
                        }
                    }

                    publish_order_book(&tx, &redis_client, &state, &bids, &asks).await;
                }
            }

            "ticker" => {
                let price = json_value["price"]
                    .as_str()
                    .unwrap_or("0")
                    .parse::<f64>()
                    .unwrap_or(0.0);

                let size = json_value["last_size"]
                    .as_str()
                    .unwrap_or("0")
                    .parse::<f64>()
                    .unwrap_or(0.0);

                let timestamp = json_value["time"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();

                if price <= 0.0 || timestamp.is_empty() {
                    continue;
                }

                let tick = MarketTick {
                    symbol: "BTC-USD".to_string(),
                    price,
                    size,
                    timestamp: timestamp.clone(),
                };

                let tick_msg = json!({
                    "event_type": "tick",
                    "data": tick
                });

                let tick_text = tick_msg.to_string();

                let _ = tx.send(tick_text.clone());
                state.tick_events.inc();

                publish_to_redis(&redis_client, "market-data:ticks", tick_text).await;

                let second = timestamp.chars().take(19).collect::<String>();

                match &mut current_candle {
                    Some(candle) if candle.second == second => {
                        candle.high = candle.high.max(price);
                        candle.low = candle.low.min(price);
                        candle.close = price;
                        candle.volume += size;

                        let candle_msg = json!({
                            "event_type": "candle",
                            "data": candle
                        });

                        let candle_text = candle_msg.to_string();

                        let _ = tx.send(candle_text.clone());
                        state.candle_events.inc();

                        publish_to_redis(
                            &redis_client,
                            "market-data:candles",
                            candle_text,
                        )
                        .await;
                    }

                    _ => {
                        current_candle = Some(Candle {
                            symbol: "BTC-USD".to_string(),
                            timeframe: "1s".to_string(),
                            second,
                            open: price,
                            high: price,
                            low: price,
                            close: price,
                            volume: size,
                        });

                        if let Some(candle) = &current_candle {
                            let candle_msg = json!({
                                "event_type": "candle",
                                "data": candle
                            });

                            let candle_text = candle_msg.to_string();

                            let _ = tx.send(candle_text.clone());
                            state.candle_events.inc();

                            publish_to_redis(
                                &redis_client,
                                "market-data:candles",
                                candle_text,
                            )
                            .await;
                        }
                    }
                }
            }

            _ => {}
        }
    }
}

async fn publish_order_book(
    tx: &broadcast::Sender<String>,
    redis_client: &redis::Client,
    state: &AppState,
    bids: &BTreeMap<Price, f64>,
    asks: &BTreeMap<Price, f64>,
) {
    let bid_levels: Vec<OrderBookLevel> = bids
        .iter()
        .rev()
        .take(20)
        .map(|(price, size)| OrderBookLevel {
            price: price.0,
            size: *size,
        })
        .collect();

    let ask_levels: Vec<OrderBookLevel> = asks
        .iter()
        .take(20)
        .map(|(price, size)| OrderBookLevel {
            price: price.0,
            size: *size,
        })
        .collect();

    let snapshot = OrderBookSnapshot {
        symbol: "BTC-USD".to_string(),
        bids: bid_levels,
        asks: ask_levels,
    };

    let msg = json!({
        "event_type": "order_book",
        "data": snapshot
    });

    let book_text = msg.to_string();

    let _ = tx.send(book_text.clone());
    state.orderbook_events.inc();
    state.candle_events.inc();

    publish_to_redis(redis_client, "market-data:orderbook", book_text).await;
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