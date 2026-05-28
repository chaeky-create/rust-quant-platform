use std::collections::HashMap;

#[derive(Debug, Clone)]
struct Bar {
    time: usize,
    price: f64,
}

#[derive(Debug, Clone)]
struct AssetSeries {
    symbol: String,
    bars: Vec<Bar>,
}

#[derive(Debug, Clone)]
struct AssetSignal {
    symbol: String,
    momentum: f64,
    volatility: f64,
    trend_ok: bool,
    selected: bool,
    raw_weight: f64,
    final_weight: f64,
}

#[derive(Debug, Clone)]
struct PortfolioState {
    cash: f64,
    holdings: HashMap<String, f64>,
    equity_curve: Vec<f64>,
    trades: usize,
}

impl PortfolioState {
    fn new() -> Self {
        Self {
            cash: 100_000.0,
            holdings: HashMap::new(),
            equity_curve: Vec::new(),
            trades: 0,
        }
    }

    fn equity(&self, prices: &HashMap<String, f64>) -> f64 {
        let holdings_value = self
            .holdings
            .iter()
            .map(|(symbol, qty)| qty * prices.get(symbol).unwrap_or(&0.0))
            .sum::<f64>();

        self.cash + holdings_value
    }

    fn rebalance(&mut self, target_weights: &HashMap<String, f64>, prices: &HashMap<String, f64>) {
        let equity = self.equity(prices);

        let current_symbols: Vec<String> = self.holdings.keys().cloned().collect();

        for symbol in current_symbols {
            if !target_weights.contains_key(&symbol) {
                if let Some(price) = prices.get(&symbol) {
                    let qty = self.holdings.remove(&symbol).unwrap_or(0.0);
                    self.cash += qty * price;
                    self.trades += 1;
                }
            }
        }

        for (symbol, weight) in target_weights {
            let Some(price) = prices.get(symbol) else {
                continue;
            };

            if *price <= 0.0 {
                continue;
            }

            let target_value = equity * weight;
            let current_qty = *self.holdings.get(symbol).unwrap_or(&0.0);
            let current_value = current_qty * price;
            let diff_value = target_value - current_value;

            if diff_value.abs() < equity * 0.001 {
                continue;
            }

            let diff_qty = diff_value / price;
            self.cash -= diff_qty * price;
            self.holdings.insert(symbol.clone(), current_qty + diff_qty);
            self.trades += 1;
        }
    }

    fn mark_to_market(&mut self, prices: &HashMap<String, f64>) {
        let equity = self.equity(prices);
        self.equity_curve.push(equity);
    }
}

#[derive(Debug, Clone)]
struct PortfolioReport {
    name: String,
    final_equity: f64,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    sharpe_ratio: f64,
    trades: usize,
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    values.iter().sum::<f64>() / values.len() as f64
}

fn stddev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }

    let avg = mean(values);
    let variance = values
        .iter()
        .map(|x| {
            let d = x - avg;
            d * d
        })
        .sum::<f64>()
        / (values.len() - 1) as f64;

    variance.sqrt()
}

fn pct_change(current: f64, previous: f64) -> f64 {
    if previous.abs() < 1e-12 {
        0.0
    } else {
        (current - previous) / previous
    }
}

fn rolling_momentum(prices: &[f64], window: usize) -> f64 {
    if prices.len() <= window {
        return 0.0;
    }

    pct_change(prices[prices.len() - 1], prices[prices.len() - 1 - window])
}

fn rolling_volatility(prices: &[f64], window: usize) -> f64 {
    if prices.len() <= window + 1 {
        return 0.0;
    }

    let start = prices.len() - window;
    let returns: Vec<f64> = (start..prices.len())
        .map(|i| pct_change(prices[i], prices[i - 1]))
        .collect();

    stddev(&returns)
}

fn moving_average(prices: &[f64], window: usize) -> f64 {
    if prices.len() < window {
        return 0.0;
    }

    mean(&prices[prices.len() - window..])
}

fn summarize(name: &str, state: &PortfolioState) -> PortfolioReport {
    let final_equity = *state.equity_curve.last().unwrap_or(&100_000.0);
    let total_return_pct = (final_equity / 100_000.0 - 1.0) * 100.0;

    let mut peak = 100_000.0;
    let mut max_drawdown_pct = 0.0;

    for equity in &state.equity_curve {
        if *equity > peak {
            peak = *equity;
        }

        let drawdown = if peak > 0.0 {
            (peak - equity) / peak * 100.0
        } else {
            0.0
        };

        if drawdown > max_drawdown_pct {
            max_drawdown_pct = drawdown;
        }
    }

    let returns: Vec<f64> = state
        .equity_curve
        .windows(2)
        .map(|w| pct_change(w[1], w[0]))
        .collect();

    let avg = mean(&returns);
    let sd = stddev(&returns);
    let sharpe_ratio = if sd > 0.0 {
        avg / sd * 252.0_f64.sqrt()
    } else {
        0.0
    };

    PortfolioReport {
        name: name.to_string(),
        final_equity,
        total_return_pct,
        max_drawdown_pct,
        sharpe_ratio,
        trades: state.trades,
    }
}

fn generate_synthetic_asset(symbol: &str, drift: f64, vol: f64, phase: f64, n: usize) -> AssetSeries {
    let mut bars = Vec::new();
    let mut price = 100.0;

    for t in 0..n {
        let cycle = ((t as f64 + phase) / 35.0).sin() * vol;
        let shock = ((t as f64 * 12.9898 + phase).sin() * 43758.5453).fract() * vol * 0.5;
        let daily_return = drift + cycle + shock;

        price *= 1.0 + daily_return;
        price = price.max(1.0);

        bars.push(Bar { time: t, price });
    }

    AssetSeries {
        symbol: symbol.to_string(),
        bars,
    }
}

fn generate_universe() -> Vec<AssetSeries> {
    vec![
        generate_synthetic_asset("BTC", 0.00045, 0.018, 1.0, 900),
        generate_synthetic_asset("ETH", 0.00050, 0.022, 2.0, 900),
        generate_synthetic_asset("SPY", 0.00025, 0.008, 3.0, 900),
        generate_synthetic_asset("QQQ", 0.00030, 0.010, 4.0, 900),
        generate_synthetic_asset("IWM", 0.00018, 0.011, 5.0, 900),
        generate_synthetic_asset("GLD", 0.00012, 0.007, 6.0, 900),
        generate_synthetic_asset("TLT", 0.00008, 0.009, 7.0, 900),
        generate_synthetic_asset("DBC", 0.00015, 0.012, 8.0, 900),
        generate_synthetic_asset("UUP", 0.00005, 0.006, 9.0, 900),
        generate_synthetic_asset("VNQ", 0.00016, 0.010, 10.0, 900),
    ]
}

fn prices_at(universe: &[AssetSeries], index: usize) -> HashMap<String, f64> {
    let mut prices = HashMap::new();

    for asset in universe {
        if let Some(bar) = asset.bars.get(index) {
            prices.insert(asset.symbol.clone(), bar.price);
        }
    }

    prices
}

fn run_cross_asset_momentum(universe: &[AssetSeries]) -> PortfolioState {
    let mut state = PortfolioState::new();

    let momentum_window = 60;
    let trend_window = 120;
    let volatility_window = 20;
    let max_assets = 3;
    let min_momentum = 0.02;
    let max_daily_vol = 0.035;
    let target_annual_vol = 0.15;
    let leverage_cap = 2.0;

    let n = universe.iter().map(|a| a.bars.len()).min().unwrap_or(0);

    for i in 0..n {
        let current_prices = prices_at(universe, i);

        if i < trend_window + 2 {
            state.mark_to_market(&current_prices);
            continue;
        }

        let mut signals: Vec<AssetSignal> = Vec::new();

        for asset in universe {
            let prices: Vec<f64> = asset.bars[..=i].iter().map(|b| b.price).collect();

            let momentum = rolling_momentum(&prices, momentum_window);
            let volatility = rolling_volatility(&prices, volatility_window);
            let ma = moving_average(&prices, trend_window);
            let current_price = *prices.last().unwrap_or(&0.0);

            let trend_ok = current_price > ma
                && momentum > min_momentum
                && volatility > 0.0
                && volatility < max_daily_vol;

            signals.push(AssetSignal {
                symbol: asset.symbol.clone(),
                momentum,
                volatility,
                trend_ok,
                selected: false,
                raw_weight: 0.0,
                final_weight: 0.0,
            });
        }

        signals.sort_by(|a, b| {
            b.momentum
                .partial_cmp(&a.momentum)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut selected: Vec<AssetSignal> = signals
            .into_iter()
            .filter(|s| s.trend_ok)
            .take(max_assets)
            .collect();

        let inv_vol_sum = selected
            .iter()
            .map(|s| 1.0 / s.volatility.max(1e-9))
            .sum::<f64>();

        let mut target_weights = HashMap::new();

        if inv_vol_sum > 0.0 && !selected.is_empty() {
            for signal in selected.iter_mut() {
                signal.selected = true;
                signal.raw_weight = (1.0 / signal.volatility.max(1e-9)) / inv_vol_sum;
            }

            let estimated_portfolio_daily_vol = selected
                .iter()
                .map(|s| s.raw_weight * s.volatility)
                .sum::<f64>();

            let target_daily_vol = target_annual_vol / 252.0_f64.sqrt();

            let leverage = if estimated_portfolio_daily_vol > 0.0 {
                (target_daily_vol / estimated_portfolio_daily_vol).min(leverage_cap)
            } else {
                0.0
            };

            for signal in selected {
                let final_weight = signal.raw_weight * leverage;
                target_weights.insert(signal.symbol, final_weight);
            }
        }

        state.rebalance(&target_weights, &current_prices);
        state.mark_to_market(&current_prices);
    }

    state
}

fn run_equal_weight_buy_and_hold(universe: &[AssetSeries]) -> PortfolioState {
    let mut state = PortfolioState::new();
    let n = universe.iter().map(|a| a.bars.len()).min().unwrap_or(0);

    if n == 0 {
        return state;
    }

    let first_prices = prices_at(universe, 0);
    let weight = 1.0 / universe.len() as f64;
    let target_weights: HashMap<String, f64> = universe
        .iter()
        .map(|a| (a.symbol.clone(), weight))
        .collect();

    state.rebalance(&target_weights, &first_prices);

    for i in 0..n {
        let prices = prices_at(universe, i);
        state.mark_to_market(&prices);
    }

    state
}

fn main() {
    let universe = generate_universe();

    let strategy_state = run_cross_asset_momentum(&universe);
    let benchmark_state = run_equal_weight_buy_and_hold(&universe);

    let strategy_report = summarize("cross_asset_relative_momentum", &strategy_state);
    let benchmark_report = summarize("equal_weight_buy_and_hold", &benchmark_state);

    println!("=== CROSS-ASSET MOMENTUM REPORT ===");
    println!(
        "{} | final_equity={:.2} return={:.3}% max_dd={:.3}% sharpe={:.4} trades={}",
        strategy_report.name,
        strategy_report.final_equity,
        strategy_report.total_return_pct,
        strategy_report.max_drawdown_pct,
        strategy_report.sharpe_ratio,
        strategy_report.trades
    );

    println!(
        "{} | final_equity={:.2} return={:.3}% max_dd={:.3}% sharpe={:.4} trades={}",
        benchmark_report.name,
        benchmark_report.final_equity,
        benchmark_report.total_return_pct,
        benchmark_report.max_drawdown_pct,
        benchmark_report.sharpe_ratio,
        benchmark_report.trades
    );

    println!();
    println!("=== INTERPRETATION ===");
    println!(
        "excess_return={:.3}%",
        strategy_report.total_return_pct - benchmark_report.total_return_pct
    );
    println!(
        "drawdown_reduction={:.3}%",
        benchmark_report.max_drawdown_pct - strategy_report.max_drawdown_pct
    );
}