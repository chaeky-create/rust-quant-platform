use crate::OptionInput;
use crate::models::black_scholes::black_scholes_call;
use crate::greeks::vega::vega;

pub fn implied_volatility(market_price: f64, mut input: OptionInput) -> f64 {
    let tolerance = 1e-6;
    let max_iterations = 100;

    let mut sigma = 0.2;

    for _ in 0..max_iterations {
        input.volatility = sigma;

        let price = black_scholes_call(input);
        let vega_value = vega(input);

        let diff = price - market_price;

        if diff.abs() < tolerance {
            return sigma;
        }

        sigma -= diff / vega_value;
    }

    sigma
}