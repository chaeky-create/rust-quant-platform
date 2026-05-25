use std::time::Instant;

use rust_options_pricer::api::start_api;

use rust_options_pricer::OptionInput;

use rust_options_pricer::models::american::american_put;
use rust_options_pricer::models::binomial::binomial_call;
use rust_options_pricer::models::black_scholes::black_scholes_call;
use rust_options_pricer::models::monte_carlo::{
    monte_carlo_call,
    parallel_monte_carlo_call,
};

use rust_options_pricer::greeks::delta::delta;
use rust_options_pricer::greeks::gamma::gamma;
use rust_options_pricer::greeks::vega::vega;

use rust_options_pricer::calibration::implied_vol::implied_volatility;

#[tokio::main]
async fn main() {
    let input = OptionInput {
        spot: 100.0,
        strike: 110.0,
        rate: 0.05,
        volatility: 0.20,
        maturity: 1.0,
    };

    let simulations = 1_000_000;

    // =========================
    // Pricing Models
    // =========================

    let start = Instant::now();
    let bs_price = black_scholes_call(input);
    let bs_time = start.elapsed();

    let start = Instant::now();
    let mc_price = monte_carlo_call(input, simulations);
    let mc_time = start.elapsed();

    let start = Instant::now();
    let parallel_mc_price =
        parallel_monte_carlo_call(input, simulations);
    let parallel_mc_time = start.elapsed();

    let binomial_price = binomial_call(input, 1000);

    let american_price = american_put(input, 1000);

    // =========================
    // Greeks
    // =========================

    let delta_value = delta(input);
    let gamma_value = gamma(input);
    let vega_value = vega(input);

    // =========================
    // Calibration
    // =========================

    let market_price = 6.04;

    let implied_vol =
        implied_volatility(market_price, input);

    // =========================
    // Output
    // =========================

    println!("=== Pricing Models ===");

    println!("Black-Scholes Price: {:.6}", bs_price);

    println!("Monte Carlo Price: {:.6}", mc_price);

    println!(
        "Parallel Monte Carlo Price: {:.6}",
        parallel_mc_price
    );

    println!(
        "Binomial Tree Price: {:.6}",
        binomial_price
    );

    println!(
        "American Put Price: {:.6}",
        american_price
    );

    println!("\n=== Greeks ===");

    println!("Delta: {:.6}", delta_value);

    println!("Gamma: {:.6}", gamma_value);

    println!("Vega: {:.6}", vega_value);

    println!("\n=== Calibration ===");

    println!(
        "Implied Volatility: {:.6}",
        implied_vol
    );

    println!("\n=== Performance ===");

    println!("Black-Scholes Time: {:?}", bs_time);

    println!("Monte Carlo Time: {:?}", mc_time);

    println!(
        "Parallel Monte Carlo Time: {:?}",
        parallel_mc_time
    );

    // =========================
    // Start API Server
    // =========================

    start_api().await;
}