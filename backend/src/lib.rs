pub mod models;
pub mod greeks;
pub mod calibration;
pub mod api;

#[derive(Debug, Clone, Copy)]
pub struct OptionInput {
    pub spot: f64,
    pub strike: f64,
    pub rate: f64,
    pub volatility: f64,
    pub maturity: f64,
}