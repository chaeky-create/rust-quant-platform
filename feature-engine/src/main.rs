use axum::{
    extract::State,
    response::Response,
    routing::get,
    Router,
};
use futures_util::StreamExt;
use prometheus::{Encoder, IntCounter, Registry, TextEncoder};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::VecDeque, net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct AppState {
    registry: Arc<Registry>,
    ticks_received: IntCounter,
    orderbooks_received: IntCounter,
    features_published: IntCounter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FeatureSnapshot {
    symbol: String,
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

#[derive(Debug, Clone, Default)]
struct OrderBookFeatures {
    spread: f64,
    imbalance: f64,
    microprice: f64,
}

#[derive(Debug)]
struct RollingState {
    prices: VecDeque<f64>,
    max_len: usize,
}

impl RollingState {
    fn new(max_len: usize) -> Self {
        Self {
            prices: VecDeque::with_capacity(max_len),
            max_len,
        }
    }

    fn push(&mut self, price: f64) {
        if self.prices.len() >= self.max_len {
            self.prices.pop_front();
        }

        self.prices.push_back(price);
    }

    fn len(&self) -> usize {
        self.prices.len()
    }

    fn last(&self) -> Option<f64> {
        self.prices.back().copied()
    }

    fn return_n(&self, n: usize) -> f64 {
        if self.prices.len() <= n {
            return 0.0;
        }

        let latest = *self.prices.back().unwrap();
        let past = self.prices[self.prices.len() - 1 - n];

        if past.abs() < 1e-12 {
            0.0
        } else {
            (latest - past) / past
        }
    }

    fn moving_average(&self, window: usize) -> f64 {
        if self.prices.len() < window {
            return self.last().unwrap_or(0.0);
        }

        self.prices
            .iter()
            .rev()
            .take(window)
            .sum::<f64>()
            / window as f64
    }

    fn volatility(&self, window: usize) -> f64 {
        if self.prices.len() < window + 1 {
            return 0.0;
        }

        let recent: Vec<f64> = self
            .prices
            .iter()
            .rev()
            .take(window + 1)
            .copied()
            .collect();

        let mut returns = Vec::with_capacity(window);

        for pair in recent.windows(2) {
            let newer = pair[0];
            let older = pair[1];

            if older.abs() > 1e-12 {
                returns.push((newer - older) / older);
            }
        }

        if returns.is_empty() {
            return 0.0;
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;

        let variance = returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>()
            / returns.len() as f64;

        variance.sqrt()
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

fn parse_order_book_features(json_value: &Value) -> OrderBookFeatures {
    let bids = json_value["data"]["bids"].as_array();
    let asks = json_value["data"]["asks"].as_array();

    let Some(bids) = bids else {
        return OrderBookFeatures::default();
    };

    let Some(asks) = asks else {
        return OrderBookFeatures::default();
    };

    if bids.is_empty() || asks.is_empty() {
        return OrderBookFeatures::default();
    }

    let best_bid = bids[0]["price"].as_f64().unwrap_or(0.0);
    let best_ask = asks[0]["price"].as_f64().unwrap_or(0.0);

    let bid_depth: f64 = bids
        .iter()
        .take(10)
        .map(|b| b["size"].as_f64().unwrap_or(0.0))
        .sum();

    let ask_depth: f64 = asks
        .iter()
        .take(10)
        .map(|a| a["size"].as_f64().unwrap_or(0.0))
        .sum();

    let spread = if best_ask > 0.0 && best_bid > 0.0 {
        best_ask - best_bid
    } else {
        0.0
    };

    let imbalance = if bid_depth + ask_depth > 0.0 {
        (bid_depth - ask_depth) / (bid_depth + ask_depth)
    } else {
        0.0
    };

    let microprice = if bid_depth + ask_depth > 0.0 {
        (best_ask * bid_depth + best_bid * ask_depth) / (bid_depth + ask_depth)
    } else {
        0.0
    };

    OrderBookFeatures {
        spread,
        imbalance,
        microprice,
    }
}

fn classify_regime(trend_strength: f64, volatility_20: f64, orderbook_imbalance: f64) -> String {
    if volatility_20 > 0.01 {
        "HIGH_VOL".to_string()
    } else if trend_strength.abs() > 0.001 && orderbook_imbalance.abs() > 0.10 {
        "TRENDING_CONFIRMED".to_string()
    } else if trend_strength.abs() > 0.001 {
        "TRENDING".to_string()
    } else {
        "RANGE_BOUND".to_string()
    }
}

async fn publish_features(
    redis_client: &redis::Client,
    state: &AppState,
    features: FeatureSnapshot,
) {
    let Ok(feature_text) = serde_json::to_string(&features) else {
        return;
    };

    if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
        let _: redis::RedisResult<()> = conn.publish("features:latest", feature_text).await;
    }

    state.features_published.inc();
}

async fn run_feature_engine(state: AppState) {
    println!("Connecting feature-engine to Redis...");

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string());

    let redis_client = redis::Client::open(redis_url).expect("Failed to create Redis client");

    let mut pubsub = redis_client
        .get_async_pubsub()
        .await
        .expect("Failed to connect to Redis pubsub");

    pubsub
        .subscribe("market-data:ticks")
        .await
        .expect("Failed to subscribe market-data:ticks");

    pubsub
        .subscribe("market-data:orderbook")
        .await
        .expect("Failed to subscribe market-data:orderbook");

    println!("feature-engine subscribed to market-data:ticks and market-data:orderbook.");

    let mut stream = pubsub.on_message();

    let mut rolling = RollingState::new(200);
    let mut orderbook = OrderBookFeatures::default();

    while let Some(message) = stream.next().await {
        let channel = message.get_channel_name().to_string();

        let Ok(payload): Result<String, _> = message.get_payload() else {
            continue;
        };

        let Ok(json_value) = serde_json::from_str::<Value>(&payload) else {
            continue;
        };

        if channel == "market-data:orderbook" {
            state.orderbooks_received.inc();
            orderbook = parse_order_book_features(&json_value);
            continue;
        }

        if channel != "market-data:ticks" {
            continue;
        }

        state.ticks_received.inc();

        let data = &json_value["data"];

        let symbol = data["symbol"].as_str().unwrap_or("BTC-USD").to_string();
        let price = data["price"].as_f64().unwrap_or(0.0);
        let timestamp = data["timestamp"].as_str().unwrap_or("").to_string();

        if price <= 0.0 {
            continue;
        }

        rolling.push(price);

        if rolling.len() < 30 {
            continue;
        }

        let short_ma = rolling.moving_average(5);
        let long_ma = rolling.moving_average(20);
        let trend_strength = if long_ma.abs() > 1e-12 {
            (short_ma - long_ma) / long_ma
        } else {
            0.0
        };

        let volatility_20 = rolling.volatility(20);
        let return_1 = rolling.return_n(1);
        let return_5 = rolling.return_n(5);

        let regime = classify_regime(
            trend_strength,
            volatility_20,
            orderbook.imbalance,
        );

        let features = FeatureSnapshot {
            symbol,
            price,
            return_1,
            return_5,
            volatility_20,
            short_ma,
            long_ma,
            trend_strength,
            spread: orderbook.spread,
            orderbook_imbalance: orderbook.imbalance,
            microprice: orderbook.microprice,
            regime,
            timestamp,
        };

        println!("{}", serde_json::to_string(&features).unwrap());

        publish_features(&redis_client, &state, features).await;
    }
}

#[tokio::main]
async fn main() {
    let registry = Registry::new();

    let ticks_received = IntCounter::new(
        "feature_engine_ticks_received_total",
        "Total tick messages received by feature-engine",
    )
    .unwrap();

    let orderbooks_received = IntCounter::new(
        "feature_engine_orderbooks_received_total",
        "Total order book messages received by feature-engine",
    )
    .unwrap();

    let features_published = IntCounter::new(
        "feature_engine_features_published_total",
        "Total feature snapshots published",
    )
    .unwrap();

    registry
        .register(Box::new(ticks_received.clone()))
        .unwrap();

    registry
        .register(Box::new(orderbooks_received.clone()))
        .unwrap();

    registry
        .register(Box::new(features_published.clone()))
        .unwrap();

    let state = AppState {
        registry: Arc::new(registry),
        ticks_received,
        orderbooks_received,
        features_published,
    };

    let engine_state = state.clone();

    tokio::spawn(async move {
        run_feature_engine(engine_state).await;
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/metrics", get(metrics_endpoint))
        .layer(cors)
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "9601".to_string())
        .parse()
        .expect("PORT must be a number");

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    println!("Feature Engine running on http://127.0.0.1:{}/metrics", port);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind feature-engine");

    axum::serve(listener, app)
        .await
        .expect("feature-engine failed");
}