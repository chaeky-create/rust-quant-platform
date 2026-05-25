use crate::OptionInput;

pub fn american_put(input: OptionInput, steps: usize) -> f64 {
    let dt = input.maturity / steps as f64;

    let up = (input.volatility * dt.sqrt()).exp();
    let down = 1.0 / up;

    let discount = (-input.rate * dt).exp();

    let p = ((input.rate * dt).exp() - down) / (up - down);

    let mut prices = vec![0.0; steps + 1];

    // Terminal payoff
    for i in 0..=steps {
        let stock_price = input.spot
            * up.powi((steps - i) as i32)
            * down.powi(i as i32);

        prices[i] = (input.strike - stock_price).max(0.0);
    }

    // Backward induction with early exercise
    for step in (0..steps).rev() {
        for i in 0..=step {
            let stock_price = input.spot
                * up.powi((step - i) as i32)
                * down.powi(i as i32);

            let continuation =
                discount * (p * prices[i] + (1.0 - p) * prices[i + 1]);

            let exercise =
                (input.strike - stock_price).max(0.0);

            prices[i] = continuation.max(exercise);
        }
    }

    prices[0]
}