use rand::RngExt;
use statrs::distribution::{ContinuousCDF, Normal};
use rayon::prelude::*;
use crate::OptionInput;

pub fn monte_carlo_call(input: OptionInput, simulations: usize) -> f64 {
    let normal = Normal::new(0.0, 1.0).unwrap();
    let mut rng = rand::rng();

    let mut payoff_sum = 0.0;

    for _ in 0..simulations {
        let u: f64 = rng.random();
        let z = normal.inverse_cdf(u);

        let terminal_price = input.spot
            * ((input.rate - 0.5 * input.volatility.powi(2)) * input.maturity
                + input.volatility * input.maturity.sqrt() * z)
                .exp();

        payoff_sum += (terminal_price - input.strike).max(0.0);
    }

    (-input.rate * input.maturity).exp() * payoff_sum / simulations as f64
}

pub fn parallel_monte_carlo_call(input: OptionInput, simulations: usize) -> f64 {
    let normal = Normal::new(0.0, 1.0).unwrap();

    let payoff_sum: f64 = (0..simulations)
        .into_par_iter()
        .map(|_| {
            let mut rng = rand::rng();
            let u: f64 = rng.random();
            let z = normal.inverse_cdf(u);

            let terminal_price = input.spot
                * ((input.rate - 0.5 * input.volatility.powi(2)) * input.maturity
                    + input.volatility * input.maturity.sqrt() * z)
                    .exp();

            (terminal_price - input.strike).max(0.0)
        })
        .sum();

    (-input.rate * input.maturity).exp() * payoff_sum / simulations as f64
}