use crate::OptionInput;
use crate::models::black_scholes::d1;

pub fn vega(input: OptionInput) -> f64 {
    let d1_value = d1(input);
    let pdf = (-0.5 * d1_value * d1_value).exp()
        / (2.0 * std::f64::consts::PI).sqrt();

    input.spot * pdf * input.maturity.sqrt()
}