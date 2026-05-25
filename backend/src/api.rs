use axum::{
    extract::Query,
    routing::get,
    Json, Router,
};

use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::calibration::implied_vol::implied_volatility;
use crate::greeks::delta::delta;
use crate::greeks::gamma::gamma;
use crate::greeks::vega::vega;
use crate::models::american::american_put;
use crate::models::black_scholes::black_scholes_call;
use crate::models::monte_carlo::parallel_monte_carlo_call;
use crate::OptionInput;

#[derive(Deserialize)]
struct PriceQuery {
    spot: f64,
    strike: f64,
    rate: f64,
    volatility: f64,
    maturity: f64,
}

#[derive(Deserialize)]
struct MonteCarloQuery {
    spot: f64,
    strike: f64,
    rate: f64,
    volatility: f64,
    maturity: f64,
    simulations: usize,
}

#[derive(Deserialize)]
struct ImpliedVolQuery {
    market_price: f64,
    spot: f64,
    strike: f64,
    rate: f64,
    maturity: f64,
}

#[derive(Serialize)]
struct PriceResponse {
    model: String,
    price: f64,
}

#[derive(Serialize)]
struct GreeksResponse {
    delta: f64,
    gamma: f64,
    vega: f64,
}

#[derive(Serialize)]
struct ImpliedVolResponse {
    implied_volatility: f64,
}

fn build_input(params: &PriceQuery) -> OptionInput {
    OptionInput {
        spot: params.spot,
        strike: params.strike,
        rate: params.rate,
        volatility: params.volatility,
        maturity: params.maturity,
    }
}

async fn price_option(Query(params): Query<PriceQuery>) -> Json<PriceResponse> {
    let input = build_input(&params);
    let price = black_scholes_call(input);

    Json(PriceResponse {
        model: "Black-Scholes".to_string(),
        price,
    })
}

async fn greeks(Query(params): Query<PriceQuery>) -> Json<GreeksResponse> {
    let input = build_input(&params);

    Json(GreeksResponse {
        delta: delta(input),
        gamma: gamma(input),
        vega: vega(input),
    })
}

async fn monte_carlo(Query(params): Query<MonteCarloQuery>) -> Json<PriceResponse> {
    let input = OptionInput {
        spot: params.spot,
        strike: params.strike,
        rate: params.rate,
        volatility: params.volatility,
        maturity: params.maturity,
    };

    let price = parallel_monte_carlo_call(input, params.simulations);

    Json(PriceResponse {
        model: "Parallel Monte Carlo".to_string(),
        price,
    })
}

async fn american_put_endpoint(Query(params): Query<PriceQuery>) -> Json<PriceResponse> {
    let input = build_input(&params);
    let price = american_put(input, 1000);

    Json(PriceResponse {
        model: "American Put Binomial Tree".to_string(),
        price,
    })
}

async fn implied_vol_endpoint(Query(params): Query<ImpliedVolQuery>) -> Json<ImpliedVolResponse> {
    let input = OptionInput {
        spot: params.spot,
        strike: params.strike,
        rate: params.rate,
        volatility: 0.20,
        maturity: params.maturity,
    };

    let iv = implied_volatility(params.market_price, input);

    Json(ImpliedVolResponse {
        implied_volatility: iv,
    })
}

pub async fn start_api() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/price", get(price_option))
        .route("/greeks", get(greeks))
        .route("/monte-carlo", get(monte_carlo))
        .route("/american-put", get(american_put_endpoint))
        .route("/implied-vol", get(implied_vol_endpoint))
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();

    println!("API running on http://127.0.0.1:8080");

    axum::serve(listener, app).await.unwrap();
}