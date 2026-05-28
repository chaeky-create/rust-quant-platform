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

#[derive(Debug, Clone, Serialize)]
struct StrategyReport {
    name: String,
    final_equity: f64,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    sharpe_ratio: f64,
    calmar_ratio: f64,
    trades: usize,
}

#[derive(Debug, Clone)]
struct ParameterCubeReport {
    best_score: f64,
    neighbor_count: usize,
    avg_neighbor_score: f64,
    median_neighbor_score: f64,
    min_neighbor_score: f64,
    max_neighbor_score: f64,
    robustness_ratio: f64,
    score_gap: f64,
}

#[derive(Debug, Clone)]
struct CandidateResult {
    config: StrategyConfig,
    train_report: StrategyReport,
    score: f64,
}

#[derive(Debug, Clone, Serialize)]
struct WalkForwardResult {
    window: usize,
    train_start: usize,
    train_end: usize,
    test_start: usize,
    test_end: usize,

    strategy_return_pct: f64,
    strategy_max_drawdown_pct: f64,
    strategy_sharpe_ratio: f64,
    strategy_calmar_ratio: f64,
    strategy_trades: usize,

    benchmark_return_pct: f64,
    benchmark_max_drawdown_pct: f64,
    benchmark_sharpe_ratio: f64,

    excess_return_pct: f64,
    drawdown_reduction_pct: f64,

    short_window: usize,
    long_window: usize,
    max_volatility: f64,
    min_momentum: f64,
    min_trend_strength: f64,
    stop_loss_pct: f64,
    take_profit_pct: f64,
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

    let recovery_market =
        features.price > features.long_ma
        && features.return_5 > 0.0
        && features.volatility_20 <= config.max_volatility * 1.5;

    if not_crashing && low_vol && uptrend && positive_momentum {
        "LONG"
    } else if not_crashing && bull_market && recovery_market {
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

    let vol_scale = (target_vol / features.volatility_20).clamp(0.35, 2.5);

    let strong_bull =
        features.price > features.long_term_ma
        && features.short_ma > features.long_ma
        && features.return_5 > config.min_momentum * 2.0
        && features.volatility_20 <= config.max_volatility;

    let risk_on_multiplier = if strong_bull {
        1.5
    } else {
        1.0
    };

    config.base_position_size * vol_scale * risk_on_multiplier
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

fn score_report(report: &StrategyReport) -> f64 {
    let risk_penalty = if report.max_drawdown_pct > 8.0 {
        (report.max_drawdown_pct - 8.0) * 2.0
    } else {
        0.0
    };

    let trade_penalty = if report.trades < 20 {
        10.0
    } else {
        0.0
    };

    let negative_return_penalty = if report.total_return_pct < 0.0 {
        report.total_return_pct.abs() * 2.0
    } else {
        0.0
    };

    report.total_return_pct * 0.20
        + report.sharpe_ratio * 3.0
        + report.calmar_ratio * 1.0
        - report.max_drawdown_pct * 0.15
        - risk_penalty
        - trade_penalty
        - negative_return_penalty
}

fn score_against_benchmark(strategy: &StrategyReport, benchmark: &StrategyReport) -> f64 {
    let excess_return = strategy.total_return_pct - benchmark.total_return_pct;
    let drawdown_reduction = benchmark.max_drawdown_pct - strategy.max_drawdown_pct;

    let risk_penalty = if strategy.max_drawdown_pct > 8.0 {
        (strategy.max_drawdown_pct - 8.0) * 2.0
    } else {
        0.0
    };

    let negative_return_penalty = if strategy.total_return_pct < 0.0 {
        strategy.total_return_pct.abs() * 2.0
    } else {
        0.0
    };

    let low_trade_penalty = if strategy.trades < 8 {
        8.0
    } else {
        0.0
    };

    strategy.total_return_pct * 0.25
        + excess_return * 0.20
        + strategy.sharpe_ratio * 3.0
        + strategy.calmar_ratio * 0.8
        + drawdown_reduction * 0.12
        - strategy.max_drawdown_pct * 0.10
        - risk_penalty
        - negative_return_penalty
        - low_trade_penalty
}

fn parameter_cube_analysis(
    train: &[Bar],
    best_config: &StrategyConfig,
    best_score: f64,
) -> ParameterCubeReport {
    let benchmark_state = run_buy_and_hold(train);
    let benchmark_report = summarize("cube_benchmark", &benchmark_state);

    let short_values = unique_sorted_usize(vec![
        best_config.short_window.saturating_sub(2).max(2),
        best_config.short_window,
        best_config.short_window + 2,
    ]);

    let long_values = unique_sorted_usize(vec![
        best_config.long_window.saturating_sub(20).max(best_config.short_window + 1),
        best_config.long_window,
        best_config.long_window + 20,
    ]);

    let max_vol_values = unique_sorted_f64(vec![
        (best_config.max_volatility * 0.75).max(0.001),
        best_config.max_volatility,
        best_config.max_volatility * 1.25,
    ]);

    let momentum_values = unique_sorted_f64(vec![
        (best_config.min_momentum * 0.5).max(0.00005),
        best_config.min_momentum,
        best_config.min_momentum * 2.0,
    ]);

    let trend_values = unique_sorted_f64(vec![
        (best_config.min_trend_strength * 0.5).max(0.00005),
        best_config.min_trend_strength,
        best_config.min_trend_strength * 2.0,
    ]);

    let stop_values = unique_sorted_f64(vec![
        (best_config.stop_loss_pct * 0.5).max(0.005),
        best_config.stop_loss_pct,
        best_config.stop_loss_pct * 1.5,
    ]);

    let take_profit_values = unique_sorted_f64(vec![
        (best_config.take_profit_pct * 0.75).max(0.02),
        best_config.take_profit_pct,
        best_config.take_profit_pct * 1.25,
    ]);

    let size_values = unique_sorted_f64(vec![
        (best_config.base_position_size - 0.25).max(0.5),
        best_config.base_position_size,
        best_config.base_position_size + 0.25,
    ]);

    let mut scores: Vec<f64> = Vec::new();

    for short_window in short_values {
        for long_window in &long_values {
            if short_window >= *long_window {
                continue;
            }

            for max_volatility in &max_vol_values {
                for min_momentum in &momentum_values {
                    for min_trend_strength in &trend_values {
                        for stop_loss_pct in &stop_values {
                            for take_profit_pct in &take_profit_values {
                                for base_position_size in &size_values {
                                    let config = StrategyConfig {
                                        short_window,
                                        long_window: *long_window,
                                        volatility_window: best_config.volatility_window,
                                        max_volatility: *max_volatility,
                                        min_momentum: *min_momentum,
                                        min_trend_strength: *min_trend_strength,
                                        stop_loss_pct: *stop_loss_pct,
                                        take_profit_pct: *take_profit_pct,
                                        base_position_size: *base_position_size,
                                    };

                                    let state = run_strategy(train, &config);
                                    let report = summarize("cube_neighbor", &state);

                                    if report.trades < 8 || report.max_drawdown_pct > 12.0 {
                                        continue;
                                    }

                                    let score =
                                        score_against_benchmark(&report, &benchmark_report);
                                    scores.push(score);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if scores.is_empty() {
        return ParameterCubeReport {
            best_score,
            neighbor_count: 0,
            avg_neighbor_score: 0.0,
            median_neighbor_score: 0.0,
            min_neighbor_score: 0.0,
            max_neighbor_score: 0.0,
            robustness_ratio: 0.0,
            score_gap: best_score,
        };
    }

    scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let neighbor_count = scores.len();
    let min_neighbor_score = scores[0];
    let max_neighbor_score = scores[neighbor_count - 1];
    let avg_neighbor_score = scores.iter().sum::<f64>() / neighbor_count as f64;

    let median_neighbor_score = if neighbor_count % 2 == 0 {
        let a = scores[neighbor_count / 2 - 1];
        let b = scores[neighbor_count / 2];
        (a + b) / 2.0
    } else {
        scores[neighbor_count / 2]
    };

    let robustness_ratio = if best_score.abs() > 1e-9 {
        median_neighbor_score / best_score
    } else {
        0.0
    };

    let score_gap = best_score - median_neighbor_score;

    ParameterCubeReport {
        best_score,
        neighbor_count,
        avg_neighbor_score,
        median_neighbor_score,
        min_neighbor_score,
        max_neighbor_score,
        robustness_ratio,
        score_gap,
    }
}

fn unique_sorted_usize(values: Vec<usize>) -> Vec<usize> {
    let mut values = values;
    values.sort();
    values.dedup();
    values
}

fn unique_sorted_f64(values: Vec<f64>) -> Vec<f64> {
    let mut values = values;
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    values.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    values
}

fn optimize_on_train(train: &[Bar]) -> Option<(StrategyConfig, BacktestState, StrategyReport, f64)> {
    let mut best_config: Option<StrategyConfig> = None;
    let mut best_state: Option<BacktestState> = None;
    let mut best_report: Option<StrategyReport> = None;
    let mut best_score = f64::MIN;
    let benchmark_state = run_buy_and_hold(train);
    let benchmark_report = summarize("train_benchmark", &benchmark_state);

    for short_window in [8, 10, 12] {
    for long_window in [60, 80, 100] {
        if short_window >= long_window {
                continue;
            }

            for max_volatility in [0.005, 0.01, 0.02, 0.03, 0.04] {
                for min_momentum in [0.0003, 0.0005, 0.001, 0.002] {
                    for min_trend_strength in [0.0003, 0.0005, 0.001, 0.002] {
                        for stop_loss_pct in [0.015, 0.02, 0.03] {
                            for take_profit_pct in [0.06, 0.08, 0.10] {
                                for base_position_size in [1.0, 1.25, 1.5] {
                                    let config = StrategyConfig {
                                        short_window,
                                        long_window,
                                        volatility_window: 20,
                                        max_volatility,
                                        min_momentum,
                                        min_trend_strength,
                                        stop_loss_pct,
                                        take_profit_pct,
                                        base_position_size,
                                    };

                                    let state = run_strategy(train, &config);
                                    let report = summarize("train", &state);
                                    let score = score_against_benchmark(&report, &benchmark_report);

                                    if report.trades >= 8
                                        && report.max_drawdown_pct <= 12.0
                                        && score > best_score
                                    {
                                        best_score = score;
                                        best_config = Some(config);
                                        best_state = Some(state);
                                        best_report = Some(report);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Some((best_config?, best_state?, best_report?, best_score))
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

    let (best_config, best_train_state, best_train_report, best_train_score) =
    optimize_on_train(train).expect("No valid config found");

println!();
println!("=== BEST TRAIN RESULT ===");
println!("score: {:.4}", best_train_score);
println!("train_return: {:.4}%", best_train_report.total_return_pct);
println!("train_max_drawdown: {:.4}%", best_train_report.max_drawdown_pct);
println!("train_sharpe: {:.4}", best_train_report.sharpe_ratio);
println!("train_calmar: {:.4}", best_train_report.calmar_ratio);
println!("train_trades: {}", best_train_report.trades);

let cube_report = parameter_cube_analysis(train, &best_config, best_train_score);

println!();
println!("=== PARAMETER CUBE ROBUSTNESS ===");
println!("neighbor_count: {}", cube_report.neighbor_count);
println!("best_score: {:.4}", cube_report.best_score);
println!("avg_neighbor_score: {:.4}", cube_report.avg_neighbor_score);
println!("median_neighbor_score: {:.4}", cube_report.median_neighbor_score);
println!("min_neighbor_score: {:.4}", cube_report.min_neighbor_score);
println!("max_neighbor_score: {:.4}", cube_report.max_neighbor_score);
println!("robustness_ratio: {:.4}", cube_report.robustness_ratio);
println!("score_gap: {:.4}", cube_report.score_gap);

if cube_report.robustness_ratio >= 0.70 {
    println!("robustness_label: ROBUST");
} else if cube_report.robustness_ratio >= 0.40 {
    println!("robustness_label: MODERATE");
} else {
    println!("robustness_label: OVERFIT_RISK");
}
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



    let train_window = 700;
    let test_window = 180;
    let step = 180;

    let mut walk_forward_results: Vec<WalkForwardResult> = Vec::new();

    let mut start = 0;
    let mut window_id = 1;

    while start + train_window + test_window <= bars.len() {
        let train_start = start;
        let train_end = start + train_window;
        let test_start = train_end;
        let test_end = train_end + test_window;

        let wf_train = &bars[train_start..train_end];
        let wf_test = &bars[test_start..test_end];

        if let Some((wf_config, _, _, _)) = optimize_on_train(wf_train) {
            let wf_strategy_state = run_strategy(wf_test, &wf_config);


            let wf_strategy_report = summarize("walk_forward_strategy", &wf_strategy_state);

            let wf_benchmark_state = run_buy_and_hold(wf_test);
            let wf_benchmark_report = summarize("walk_forward_benchmark", &wf_benchmark_state);

            let excess_return_pct =
                wf_strategy_report.total_return_pct - wf_benchmark_report.total_return_pct;

            let drawdown_reduction_pct =
                wf_benchmark_report.max_drawdown_pct - wf_strategy_report.max_drawdown_pct;

            walk_forward_results.push(WalkForwardResult {
                window: window_id,
                train_start,
                train_end,
                test_start,
                test_end,

                strategy_return_pct: wf_strategy_report.total_return_pct,
                strategy_max_drawdown_pct: wf_strategy_report.max_drawdown_pct,
                strategy_sharpe_ratio: wf_strategy_report.sharpe_ratio,
                strategy_calmar_ratio: wf_strategy_report.calmar_ratio,
                strategy_trades: wf_strategy_report.trades,

                benchmark_return_pct: wf_benchmark_report.total_return_pct,
                benchmark_max_drawdown_pct: wf_benchmark_report.max_drawdown_pct,
                benchmark_sharpe_ratio: wf_benchmark_report.sharpe_ratio,

                excess_return_pct,
                drawdown_reduction_pct,

                short_window: wf_config.short_window,
                long_window: wf_config.long_window,
                max_volatility: wf_config.max_volatility,
                min_momentum: wf_config.min_momentum,
                min_trend_strength: wf_config.min_trend_strength,
                stop_loss_pct: wf_config.stop_loss_pct,
                take_profit_pct: wf_config.take_profit_pct,
            });
                    }

        start += step;
        window_id += 1;
    }

    println!();
    println!("=== WALK-FORWARD VALIDATION ===");

    for result in &walk_forward_results {
        println!(
            "window={} strategy_return={:.4}% benchmark_return={:.4}% excess={:.4}% strategy_dd={:.4}% benchmark_dd={:.4}% dd_reduction={:.4}% sharpe={:.4} trades={} | short={} long={} vol={} mom={} trend={} sl={} tp={}",
            result.window,
            result.strategy_return_pct,
            result.benchmark_return_pct,
            result.excess_return_pct,
            result.strategy_max_drawdown_pct,
            result.benchmark_max_drawdown_pct,
            result.drawdown_reduction_pct,
            result.strategy_sharpe_ratio,
            result.strategy_trades,
            result.short_window,
            result.long_window,
            result.max_volatility,
            result.min_momentum,
            result.min_trend_strength,
            result.stop_loss_pct,
            result.take_profit_pct,
        );
    }

    if !walk_forward_results.is_empty() {
        let n = walk_forward_results.len() as f64;

        let avg_strategy_return = walk_forward_results
            .iter()
            .map(|r| r.strategy_return_pct)
            .sum::<f64>()
            / n;

        let avg_benchmark_return = walk_forward_results
            .iter()
            .map(|r| r.benchmark_return_pct)
            .sum::<f64>()
            / n;

        let avg_excess_return = walk_forward_results
            .iter()
            .map(|r| r.excess_return_pct)
            .sum::<f64>()
            / n;

        let avg_strategy_dd = walk_forward_results
            .iter()
            .map(|r| r.strategy_max_drawdown_pct)
            .sum::<f64>()
            / n;

        let avg_benchmark_dd = walk_forward_results
            .iter()
            .map(|r| r.benchmark_max_drawdown_pct)
            .sum::<f64>()
            / n;

        let avg_drawdown_reduction = walk_forward_results
            .iter()
            .map(|r| r.drawdown_reduction_pct)
            .sum::<f64>()
            / n;

        let avg_strategy_sharpe = walk_forward_results
            .iter()
            .map(|r| r.strategy_sharpe_ratio)
            .sum::<f64>()
            / n;

        let positive_strategy_windows = walk_forward_results
            .iter()
            .filter(|r| r.strategy_return_pct > 0.0)
            .count();

        let outperform_windows = walk_forward_results
            .iter()
            .filter(|r| r.excess_return_pct > 0.0)
            .count();

        println!();
        println!("=== WALK-FORWARD SUMMARY ===");
        println!("Windows: {}", walk_forward_results.len());
        println!("Average Strategy Return: {:.4}%", avg_strategy_return);
        println!("Average Benchmark Return: {:.4}%", avg_benchmark_return);
        println!("Average Excess Return: {:.4}%", avg_excess_return);
        println!("Average Strategy Max Drawdown: {:.4}%", avg_strategy_dd);
        println!("Average Benchmark Max Drawdown: {:.4}%", avg_benchmark_dd);
        println!(
            "Average Drawdown Reduction: {:.4}%",
            avg_drawdown_reduction
        );
        println!("Average Strategy Sharpe: {:.4}", avg_strategy_sharpe);
        println!(
            "Positive Strategy Windows: {}/{}",
            positive_strategy_windows,
            walk_forward_results.len()
        );
        println!(
            "Outperform Benchmark Windows: {}/{}",
            outperform_windows,
            walk_forward_results.len()
        );

        let wf_json = serde_json::to_string_pretty(&walk_forward_results).unwrap();
        fs::write("walk_forward_report.json", wf_json)
            .expect("Failed to write walk_forward_report.json");

        println!("Saved walk-forward report to walk_forward_report.json");
    }

    println!();
    println!("Saved report to backtest_report.json");
    println!("Saved test equity curve to equity_curve.csv");
}