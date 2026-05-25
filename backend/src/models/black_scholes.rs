use statrs::distribution::{ContinuousCDF, Normal};
use crate::OptionInput;

pub fn d1(input: OptionInput) -> f64 {
    ((input.spot / input.strike).ln()
        + (input.rate + 0.5 * input.volatility.powi(2)) * input.maturity)
        / (input.volatility * input.maturity.sqrt())
}

pub fn black_scholes_call(input: OptionInput) -> f64 {
    let normal = Normal::new(0.0, 1.0).unwrap();

    let d1_value = d1(input);
    let d2 = d1_value - input.volatility * input.maturity.sqrt();

    input.spot * normal.cdf(d1_value)
        - input.strike * (-input.rate * input.maturity).exp() * normal.cdf(d2)
}