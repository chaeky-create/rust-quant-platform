use serde::Serialize;
use std::fs;

#[derive(Debug, Clone)]
struct Bar {
    price: f64,
}

#[derive(Debug, Clone)]
struct FeatureSnapshot {
    price: f64,
    return_1: f64,
    return_5: f64,
    volatility_20: f64,
    short_ma: f64,
    long_ma: f64,
    long_term_ma: f64,
    trend_strength: f64,
    regime: String,
}

#[derive(Debug, Clone)]
struct StrategyConfig {
    short_window: usize,
    long_window: usize,
    volatility_window: usize,
    max_volatility: f64,
    min_momentum: f64,
    min_trend_strength: f64,
    stop_loss_pct: f64,
    take_profit_pct: f64,
    base_position_size: f64,
}

#[derive(Debug, Clone)]
struct BacktestState {
    cash: f64,
    position: f64,
    entry_price: f64,
    equity_curve: Vec<f64>,
    trades: usize,
}

#[derive(Debug, Serialize)]
struct StrategyReport {
    name: String,
    final_equity: f64,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    sharpe_ratio: f64,
    calmar_ratio: f64,
    trades: usize,
}

impl BacktestState {
    fn new() -> Self {
        Self {
            cash: 100_000.0,
            position: 0.0,
            entry_price: 0.0,
            equity_curve: Vec::new(),
            trades: 0,
        }
    }

    fn buy(&mut self, price: f64, qty: f64) {
        if self.position <= 0.0 {
            let fill_price = apply_slippage(price, "BUY");

            if self.position < 0.0 {
                self.cash += self.position.abs() * (self.entry_price - fill_price);
            }

            self.cash -= commission(fill_price, qty);
            self.position = qty;
            self.entry_price = fill_price;
            self.trades += 1;
        }
    }

    fn sell(&mut self, price: f64, qty: f64) {
        if self.position >= 0.0 {
            let fill_price = apply_slippage(price, "SELL");

            if self.position > 0.0 {
                self.cash += self.position * (fill_price - self.entry_price);
            }

            self.cash -= commission(fill_price, qty);
            self.position = -qty;
            self.entry_price = fill_price;
            self.trades += 1;
        }
    }

    fn flatten(&mut self, price: f64) {
        if self.position > 0.0 {
            let fill_price = apply_slippage(price, "SELL");
            self.cash += self.position * (fill_price - self.entry_price);
            self.cash -= commission(fill_price, self.position.abs());
        } else if self.position < 0.0 {
            let fill_price = apply_slippage(price, "BUY");
            self.cash += self.position.abs() * (self.entry_price - fill_price);
            self.cash -= commission(fill_price, self.position.abs());
        }

        if self.position.abs() > 0.0 {
            self.trades += 1;
        }

        self.position = 0.0;
        self.entry_price = 0.0;
    }

    fn mark_to_market(&mut self, price: f64) {
        let unrealized = if self.position > 0.0 {
            self.position * (price - self.entry_price)
        } else if self.position < 0.0 {
            self.position.abs() * (self.entry_price - price)
        } else {
            0.0
        };

        self.equity_curve.push(self.cash + unrealized);
    }
}

fn apply_slippage(price: f64, side: &str) -> f64 {
    let slippage_bps = 5.0 / 10_000.0;

    match side {
        "BUY" => price * (1.0 + slippage_bps),
        "SELL" => price * (1.0 - slippage_bps),
        _ => price,
    }
}

fn commission(price: f64, qty: f64) -> f64 {
    price * qty.abs() * 0.0004
}

fn moving_average(prices: &[f64]) -> f64 {
    if prices.is_empty() {
        return 0.0;
    }

    prices.iter().sum::<f64>() / prices.len() as f64
}

fn volatility(prices: &[f64]) -> f64 {
    if prices.len() < 2 {
        return 0.0;
    }

    let returns: Vec<f64> = prices
        .windows(2)
        .filter_map(|w| {
            if w[0].abs() > 1e-12 {
                Some((w[1] - w[0]) / w[0])
            } else {
                None
            }
        })
        .collect();

    if returns.is_empty() {
        return 0.0;
    }

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;

    let variance = returns
        .iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>()
        / returns.len() as f64;

    variance.sqrt()
}

fn max_drawdown(equity: &[f64]) -> f64 {
    if equity.is_empty() {
        return 0.0;
    }

    let mut peak = equity[0];
    let mut max_dd = 0.0;

    for value in equity {
        if *value > peak {
            peak = *value;
        }

        if peak.abs() > 1e-12 {
            let dd = (peak - value) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }

    max_dd
}

fn sharpe_ratio(equity: &[f64]) -> f64 {
    let returns: Vec<f64> = equity
        .windows(2)
        .filter_map(|w| {
            if w[0].abs() > 1e-12 {
                Some((w[1] - w[0]) / w[0])
            } else {
                None
            }
        })
        .collect();

    if returns.is_empty() {
        return 0.0;
    }

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;

    let variance =
        returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;

    let std = variance.sqrt();

    if std <= 1e-12 {
        0.0
    } else {
        (mean / std) * 252.0_f64.sqrt()
    }
}

fn calmar_ratio(total_return: f64, max_dd: f64) -> f64 {
    if max_dd.abs() < 1e-12 {
        0.0
    } else {
        total_return / max_dd
    }
}

fn load_csv(path: &str) -> Vec<Bar> {
    let mut rdr = csv::Reader::from_path(path).expect("Failed to open CSV");

    let mut bars = Vec::new();

    for result in rdr.records() {
        let record = result.expect("Failed to read row");
        let price = record[1].parse::<f64>().unwrap_or(0.0);

        if price > 0.0 {
            bars.push(Bar { price });
        }
    }

    bars
}

fn build_features(prices: &[f64], config: &StrategyConfig) -> Option<FeatureSnapshot> {
    let min_required = config
        .long_window
        .max(config.volatility_window)
        .max(100)
        .max(6);

    if prices.len() < min_required {
        return None;
    }

    let price = *prices.last()?;

    let previous_price = prices[prices.len() - 2];
    let price_5_back = prices[prices.len() - 6];

    if previous_price.abs() < 1e-12 || price_5_back.abs() < 1e-12 {
        return None;
    }

    let return_1 = (price - previous_price) / previous_price;
    let return_5 = (price - price_5_back) / price_5_back;

    let short_ma = moving_average(&prices[prices.len() - config.short_window..]);
    let long_ma = moving_average(&prices[prices.len() - config.long_window..]);
    let long_term_ma = moving_average(&prices[prices.len() - 100..]);
    let volatility_20 = volatility(&prices[prices.len() - config.volatility_window..]);

    let trend_strength = if long_ma.abs() > 1e-12 {
        (short_ma - long_ma) / long_ma
    } else {
        0.0
    };

    let regime = if volatility_20 > config.max_volatility {
        "HIGH_VOL"
    } else if trend_strength.abs() > config.min_trend_strength {
        "TRENDING"
    } else {
        "RANGE_BOUND"
    }
    .to_string();

    Some(FeatureSnapshot {
        price,
        return_1,
        return_5,
        volatility_20,
        short_ma,
        long_ma,
        long_term_ma,
        trend_strength,
        regime,
    })
}

fn decide_signal(features: &FeatureSnapshot, config: &StrategyConfig) -> &'static str {
    let low_vol = features.volatility_20 <= config.max_volatility;
    let uptrend = features.trend_strength > config.min_trend_strength;
    let positive_momentum = features.return_5 > config.min_momentum;
    let not_crashing = features.return_1 > -config.stop_loss_pct * 0.25;
    let bull_market = features.price > features.long_term_ma;

    if bull_market && low_vol && uptrend && positive_momentum && not_crashing {
        "LONG"
    } else {
        "FLAT"
    }
}

fn position_size(features: &FeatureSnapshot, config: &StrategyConfig) -> f64 {
    let target_vol = 0.005;

    if features.volatility_20 <= 1e-12 {
        return config.base_position_size;
    }

    let vol_scale = (target_vol / features.volatility_20).clamp(0.25, 2.0);

    config.base_position_size * vol_scale
}

fn run_strategy(bars: &[Bar], config: &StrategyConfig) -> BacktestState {
    let mut state = BacktestState::new();
    let mut prices: Vec<f64> = Vec::new();

    for bar in bars {
        prices.push(bar.price);

        let Some(features) = build_features(&prices, config) else {
            state.mark_to_market(bar.price);
            continue;
        };

        let unrealized_pct = if state.entry_price > 0.0 {
            if state.position > 0.0 {
                (bar.price - state.entry_price) / state.entry_price
            } else if state.position < 0.0 {
                (state.entry_price - bar.price) / state.entry_price
            } else {
                0.0
            }
        } else {
            0.0
        };

        if unrealized_pct <= -config.stop_loss_pct || unrealized_pct >= config.take_profit_pct {
            state.flatten(bar.price);
            state.mark_to_market(bar.price);
            continue;
        }

        let signal = decide_signal(&features, config);
        let qty = position_size(&features, config);

        match signal {
            "LONG" => state.buy(bar.price, qty),
            _ => state.flatten(bar.price),
        }

        state.mark_to_market(bar.price);
    }

    state
}

fn run_buy_and_hold(bars: &[Bar]) -> BacktestState {
    let mut state = BacktestState::new();

    if bars.is_empty() {
        return state;
    }

    let entry = bars[0].price;
    let qty = 1.0;

    state.position = qty;
    state.entry_price = entry;
    state.trades = 1;

    for bar in bars {
        state.mark_to_market(bar.price);
    }

    state.flatten(bars.last().unwrap().price);

    state
}

fn summarize(name: &str, state: &BacktestState) -> StrategyReport {
    let final_equity = *state.equity_curve.last().unwrap_or(&100_000.0);
    let total_return = (final_equity - 100_000.0) / 100_000.0;
    let max_dd = max_drawdown(&state.equity_curve);
    let sharpe = sharpe_ratio(&state.equity_curve);
    let calmar = calmar_ratio(total_return, max_dd);

    StrategyReport {
        name: name.to_string(),
        final_equity,
        total_return_pct: total_return * 100.0,
        max_drawdown_pct: max_dd * 100.0,
        sharpe_ratio: sharpe,
        calmar_ratio: calmar,
        trades: state.trades,
    }
}

fn main() {
    let bars = load_csv("data/btc.csv");

    if bars.len() < 300 {
        panic!("Not enough data in data/btc.csv");
    }

    let split = (bars.len() as f64 * 0.7) as usize;
    let train = &bars[..split];
    let test = &bars[split..];

    let buy_hold_train = run_buy_and_hold(train);
    let buy_hold_test = run_buy_and_hold(test);

    let mut best_config: Option<StrategyConfig> = None;
    let mut best_train_score = f64::MIN;
    let mut best_train_state: Option<BacktestState> = None;

    for short_window in [5, 8, 10, 12, 15] {
        for long_window in [20, 30, 50, 80, 100] {
            if short_window >= long_window {
                continue;
            }

            for max_volatility in [0.002, 0.005, 0.01, 0.02] {
                for min_momentum in [0.00005, 0.0001, 0.0002, 0.0005] {
                    for min_trend_strength in [0.00005, 0.0001, 0.0002, 0.0005] {
                        for stop_loss_pct in [0.005, 0.01, 0.02, 0.04] {
                            for take_profit_pct in [0.02, 0.04, 0.08] {
                                let config = StrategyConfig {
                                    short_window,
                                    long_window,
                                    volatility_window: 20,
                                    max_volatility,
                                    min_momentum,
                                    min_trend_strength,
                                    stop_loss_pct,
                                    take_profit_pct,
                                    base_position_size: 1.0,
                                };

                                let state = run_strategy(train, &config);
                                let report = summarize("train", &state);

                                let score = report.sharpe_ratio * 2.0
                                    + report.calmar_ratio * 0.8
                                    + report.total_return_pct * 0.03
                                    - report.max_drawdown_pct * 0.08;

                                if report.trades >= 5 && score > best_train_score {
                                    best_train_score = score;
                                    best_config = Some(config.clone());
                                    best_train_state = Some(state);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let best_config = best_config.expect("No valid config found");
    let best_train_state = best_train_state.expect("No valid state found");

    let test_state = run_strategy(test, &best_config);

    let reports = vec![
        summarize("buy_and_hold_train", &buy_hold_train),
        summarize("buy_and_hold_test", &buy_hold_test),
        summarize("optimized_strategy_train", &best_train_state),
        summarize("optimized_strategy_test", &test_state),
    ];

    println!();
    println!("=== BEST CONFIG FROM TRAIN SET ===");
    println!("short_window: {}", best_config.short_window);
    println!("long_window: {}", best_config.long_window);
    println!("volatility_window: {}", best_config.volatility_window);
    println!("max_volatility: {}", best_config.max_volatility);
    println!("min_momentum: {}", best_config.min_momentum);
    println!("min_trend_strength: {}", best_config.min_trend_strength);
    println!("stop_loss_pct: {}", best_config.stop_loss_pct);
    println!("take_profit_pct: {}", best_config.take_profit_pct);
    println!("base_position_size: {}", best_config.base_position_size);

    println!();
    println!("=== PERFORMANCE REPORT ===");

    for report in &reports {
        println!();
        println!("{}", report.name);
        println!("Final Equity: {:.2}", report.final_equity);
        println!("Total Return: {:.4}%", report.total_return_pct);
        println!("Max Drawdown: {:.4}%", report.max_drawdown_pct);
        println!("Sharpe Ratio: {:.4}", report.sharpe_ratio);
        println!("Calmar Ratio: {:.4}", report.calmar_ratio);
        println!("Trades: {}", report.trades);
    }

    let json = serde_json::to_string_pretty(&reports).unwrap();
    fs::write("backtest_report.json", json).expect("Failed to write backtest_report.json");

    let mut equity_csv = String::from("step,equity\n");
    for (i, equity) in test_state.equity_curve.iter().enumerate() {
        equity_csv.push_str(&format!("{},{}\n", i, equity));
    }

    fs::write("equity_curve.csv", equity_csv).expect("Failed to write equity_curve.csv");

    println!();
    println!("Saved report to backtest_report.json");
    println!("Saved test equity curve to equity_curve.csv");
}