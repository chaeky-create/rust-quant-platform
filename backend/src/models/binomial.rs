use crate::OptionInput;

pub fn binomial_call(input: OptionInput, steps: usize) -> f64 {
    let dt = input.maturity / steps as f64;

    let up = (input.volatility * dt.sqrt()).exp();
    let down = 1.0 / up;

    let discount = (-input.rate * dt).exp();
    let p = ((input.rate * dt).exp() - down) / (up - down);

    let mut prices = vec![0.0; steps + 1];

    for i in 0..=steps {
        let stock_price = input.spot
            * up.powi((steps - i) as i32)
            * down.powi(i as i32);

        prices[i] = (stock_price - input.strike).max(0.0);
    }

    for step in (0..steps).rev() {
        for i in 0..=step {
            prices[i] = discount * (p * prices[i] + (1.0 - p) * prices[i + 1]);
        }
    }

    prices[0]
}