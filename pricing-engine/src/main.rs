use axum::{
    extract::{Query, State},
    response::Response,
    routing::get,
    Json, Router,
};
use prometheus::{Encoder, IntCounter, Registry, TextEncoder};
use serde::{Deserialize, Serialize};
use statrs::distribution::{ContinuousCDF, Normal};
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug, Deserialize)]
struct PricingQuery {
    spot: f64,
    strike: f64,
    rate: f64,
    volatility: f64,
    maturity: f64,
}

#[derive(Debug, Serialize)]
struct PriceResponse {
    price: f64,
}

#[derive(Debug, Serialize)]
struct GreeksResponse {
    price: f64,
    delta: f64,
    gamma: f64,
    vega: f64,
    theta: f64,
    rho: f64,
}

#[derive(Debug, Serialize)]
struct SurfacePoint {
    strike: f64,
    volatility: f64,
    price: f64,
    delta: f64,
    gamma: f64,
    vega: f64,
}

#[derive(Debug, Serialize)]
struct SurfaceResponse {
    spot: f64,
    rate: f64,
    maturity: f64,
    points: Vec<SurfacePoint>,
}

fn d1(spot: f64, strike: f64, rate: f64, volatility: f64, maturity: f64) -> f64 {
    ((spot / strike).ln() + (rate + 0.5 * volatility * volatility) * maturity)
        / (volatility * maturity.sqrt())
}

fn d2(spot: f64, strike: f64, rate: f64, volatility: f64, maturity: f64) -> f64 {
    d1(spot, strike, rate, volatility, maturity) - volatility * maturity.sqrt()
}

fn call_price(spot: f64, strike: f64, rate: f64, volatility: f64, maturity: f64) -> f64 {
    let normal = Normal::new(0.0, 1.0).unwrap();
    let d1 = d1(spot, strike, rate, volatility, maturity);
    let d2 = d2(spot, strike, rate, volatility, maturity);

    spot * normal.cdf(d1) - strike * (-rate * maturity).exp() * normal.cdf(d2)
}

fn greeks(spot: f64, strike: f64, rate: f64, volatility: f64, maturity: f64) -> GreeksResponse {
    let normal = Normal::new(0.0, 1.0).unwrap();
    let d1 = d1(spot, strike, rate, volatility, maturity);
    let d2 = d2(spot, strike, rate, volatility, maturity);

    let pdf_d1 = (-0.5 * d1 * d1).exp() / (2.0 * std::f64::consts::PI).sqrt();

    let price = call_price(spot, strike, rate, volatility, maturity);
    let delta = normal.cdf(d1);
    let gamma = pdf_d1 / (spot * volatility * maturity.sqrt());
    let vega = spot * pdf_d1 * maturity.sqrt();
    let theta = -(spot * pdf_d1 * volatility) / (2.0 * maturity.sqrt())
        - rate * strike * (-rate * maturity).exp() * normal.cdf(d2);
    let rho = strike * maturity * (-rate * maturity).exp() * normal.cdf(d2);

    GreeksResponse {
        price,
        delta,
        gamma,
        vega,
        theta,
        rho,
    }
}

#[derive(Clone)]
struct AppState {
    registry: Arc<Registry>,
    price_requests: IntCounter,
    greeks_requests: IntCounter,
    surface_requests: IntCounter,
}

async fn price_endpoint(
    State(state): State<AppState>,
    Query(q): Query<PricingQuery>,
) -> Json<PriceResponse> {
    state.price_requests.inc();

    Json(PriceResponse {
        price: call_price(q.spot, q.strike, q.rate, q.volatility, q.maturity),
    })
}

async fn greeks_endpoint(
    State(state): State<AppState>,
    Query(q): Query<PricingQuery>,
) -> Json<GreeksResponse> {
    state.greeks_requests.inc();

    Json(greeks(
        q.spot,
        q.strike,
        q.rate,
        q.volatility,
        q.maturity,
    ))
}

async fn surface_endpoint(
    State(state): State<AppState>,
    Query(q): Query<PricingQuery>,
) -> Json<SurfaceResponse> {
    state.surface_requests.inc();

    let strikes = vec![
        q.spot * 0.7,
        q.spot * 0.8,
        q.spot * 0.9,
        q.spot,
        q.spot * 1.1,
        q.spot * 1.2,
        q.spot * 1.3,
    ];

    let vols = vec![0.1, 0.15, 0.2, 0.25, 0.3, 0.4, 0.5, 0.6];

    let mut points = Vec::new();

    for strike in strikes {
        for vol in &vols {
            let g = greeks(q.spot, strike, q.rate, *vol, q.maturity);

            points.push(SurfacePoint {
                strike,
                volatility: *vol,
                price: g.price,
                delta: g.delta,
                gamma: g.gamma,
                vega: g.vega,
            });
        }
    }

    Json(SurfaceResponse {
        spot: q.spot,
        rate: q.rate,
        maturity: q.maturity,
        points,
    })
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
    let registry = Registry::new();

    let price_requests = IntCounter::new(
        "pricing_engine_price_requests_total",
        "Total /price requests",
    )
    .unwrap();

    let greeks_requests = IntCounter::new(
        "pricing_engine_greeks_requests_total",
        "Total /greeks requests",
    )
    .unwrap();

    let surface_requests = IntCounter::new(
        "pricing_engine_surface_requests_total",
        "Total /surface requests",
    )
    .unwrap();

    registry.register(Box::new(price_requests.clone())).unwrap();
    registry.register(Box::new(greeks_requests.clone())).unwrap();
    registry.register(Box::new(surface_requests.clone())).unwrap();

    let state = AppState {
        registry: Arc::new(registry),
        price_requests,
        greeks_requests,
        surface_requests,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

        let app = Router::new()
        .route("/price", get(price_endpoint))
        .route("/greeks", get(greeks_endpoint))
        .route("/surface", get(surface_endpoint))
        .route("/metrics", get(metrics_endpoint))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 9501));

    println!("Pricing Engine running on http://127.0.0.1:9501");
    println!("GET /price");
    println!("GET /greeks");
    println!("GET /surface");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind pricing-engine");

    axum::serve(listener, app)
        .await
        .expect("Pricing engine failed");
}