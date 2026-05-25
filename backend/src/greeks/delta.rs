use statrs::distribution::{ContinuousCDF, Normal};
use crate::OptionInput;
use crate::models::black_scholes::d1;

pub fn delta(input: OptionInput) -> f64 {
    let normal = Normal::new(0.0, 1.0).unwrap();
    normal.cdf(d1(input))
}