use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone)]
struct Bar {
    price: f64,
}

#[derive(Debug, Clone)]
struct AssetSeries {
    symbol: String,
    bars: Vec<Bar>,
}

#[derive(Debug, Clone)]
struct PortfolioState {
    cash: f64,
    positions: HashMap<String, f64>,
    equity_curve: Vec<f64>,
    peak_equity: f64,
    trades: usize,
}

#[derive(Debug, Clone)]
struct StrategyReport {
    name: String,
    final_equity: f64,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    sharpe: f64,
    trades: usize,
}

#[derive(Debug, Clone)]
struct Holding {
    symbol: String,
    weight: f64,
}

const INITIAL_CAPITAL: f64 = 10_000.0;

fn load_csv(symbol: &str, path: &str) -> Option<AssetSeries> {
    let content = fs::read_to_string(path).ok()?;
    let mut bars = Vec::new();

    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.trim().split(',').collect();
        if parts.len() < 2 {
            continue;
        }

        let price = parts[1].parse::<f64>().ok()?;
        if price.is_finite() && price > 0.0 {
            bars.push(Bar { price });
        }
    }

    if bars.len() < 260 {
        return None;
    }

    Some(AssetSeries {
        symbol: symbol.to_string(),
        bars,
    })
}

fn pct_return(prices: &[f64], end: usize, lookback: usize) -> f64 {
    if end < lookback {
        return 0.0;
    }

    let current = prices[end];
    let past = prices[end - lookback];

    if past <= 0.0 {
        return 0.0;
    }

    (current - past) / past
}

fn moving_average(prices: &[f64], end: usize, window: usize) -> f64 {
    if end + 1 < window {
        return prices[end];
    }

    let start = end + 1 - window;
    let sum: f64 = prices[start..=end].iter().sum();
    sum / window as f64
}

fn realized_volatility(prices: &[f64], end: usize, window: usize) -> f64 {
    if end < window {
        return 0.0;
    }

    let start = end + 1 - window;
    let mut returns = Vec::new();

    for i in (start + 1)..=end {
        let prev = prices[i - 1];
        let curr = prices[i];

        if prev > 0.0 {
            returns.push((curr - prev) / prev);
        }
    }

    if returns.len() < 2 {
        return 0.0;
    }

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let var = returns
        .iter()
        .map(|r| {
            let d = r - mean;
            d * d
        })
        .sum::<f64>()
        / (returns.len() as f64 - 1.0);

    var.sqrt()
}

fn momentum_score(prices: &[f64], end: usize) -> f64 {
    let r63 = pct_return(prices, end, 63);
    let r126 = pct_return(prices, end, 126);
    let r252 = pct_return(prices, end, 252);
    let vol20 = realized_volatility(prices, end, 20).max(1e-6);

    let raw_momentum = 0.45 * r63 + 0.35 * r126 + 0.20 * r252;

    raw_momentum / vol20
}

fn max_drawdown(equity: &[f64]) -> f64 {
    if equity.is_empty() {
        return 0.0;
    }

    let mut peak = equity[0];
    let mut max_dd = 0.0;

    for &value in equity {
        if value > peak {
            peak = value;
        }

        if peak > 0.0 {
            let dd = (peak - value) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }

    max_dd * 100.0
}

fn sharpe_ratio(equity: &[f64]) -> f64 {
    if equity.len() < 3 {
        return 0.0;
    }

    let mut returns = Vec::new();

    for i in 1..equity.len() {
        if equity[i - 1] > 0.0 {
            returns.push((equity[i] - equity[i - 1]) / equity[i - 1]);
        }
    }

    if returns.len() < 2 {
        return 0.0;
    }

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let var = returns
        .iter()
        .map(|r| {
            let d = r - mean;
            d * d
        })
        .sum::<f64>()
        / (returns.len() as f64 - 1.0);

    let std = var.sqrt();

    if std <= 1e-12 {
        return 0.0;
    }

    mean / std * 252.0_f64.sqrt()
}

fn summarize(name: &str, state: &PortfolioState) -> StrategyReport {
    let final_equity = *state.equity_curve.last().unwrap_or(&INITIAL_CAPITAL);
    let total_return_pct = (final_equity / INITIAL_CAPITAL - 1.0) * 100.0;
    let max_drawdown_pct = max_drawdown(&state.equity_curve);
    let sharpe = sharpe_ratio(&state.equity_curve);

    StrategyReport {
        name: name.to_string(),
        final_equity,
        total_return_pct,
        max_drawdown_pct,
        sharpe,
        trades: state.trades,
    }
}

fn print_report(report: &StrategyReport) {
    println!();
    println!("{}", report.name);
    println!("Final Equity: {:.2}", report.final_equity);
    println!("Total Return: {:.4}%", report.total_return_pct);
    println!("Max Drawdown: {:.4}%", report.max_drawdown_pct);
    println!("Sharpe Ratio: {:.4}", report.sharpe);
    println!("Trades: {}", report.trades);
}

fn aligned_len(assets: &[AssetSeries]) -> usize {
    assets.iter().map(|a| a.bars.len()).min().unwrap_or(0)
}

fn latest_prices(assets: &[AssetSeries], idx: usize) -> HashMap<String, f64> {
    let mut prices = HashMap::new();

    for asset in assets {
        prices.insert(asset.symbol.clone(), asset.bars[idx].price);
    }

    prices
}

fn portfolio_equity(
    cash: f64,
    positions: &HashMap<String, f64>,
    prices: &HashMap<String, f64>,
) -> f64 {
    let position_value: f64 = positions
        .iter()
        .map(|(symbol, qty)| qty * prices.get(symbol).copied().unwrap_or(0.0))
        .sum();

    cash + position_value
}

fn rebalance(
    state: &mut PortfolioState,
    holdings: &[Holding],
    prices: &HashMap<String, f64>,
    total_equity: f64,
) {
    state.cash = total_equity;
    state.positions.clear();

    for holding in holdings {
        let Some(price) = prices.get(&holding.symbol) else {
            continue;
        };

        if *price <= 0.0 {
            continue;
        }

        let target_value = total_equity * holding.weight;
        let qty = target_value / price;

        state.cash -= target_value;
        state.positions.insert(holding.symbol.clone(), qty);
        state.trades += 1;
    }
}

fn determine_regime(spy_prices: &[f64], idx: usize) -> String {
    let spy_price = spy_prices[idx];
    let spy_ma200 = moving_average(spy_prices, idx, 200);
    let spy_r63 = pct_return(spy_prices, idx, 63);

    if spy_price > spy_ma200 && spy_r63 > -0.05 {
        "BULL".to_string()
    } else if spy_price < spy_ma200 && spy_r63 < -0.10 {
        "CRASH".to_string()
    } else {
        "DEFENSIVE".to_string()
    }
}

fn drawdown_exposure(current_equity: f64, peak_equity: f64, regime: &str) -> f64 {
    if peak_equity <= 0.0 {
        return 1.0;
    }

    let dd = (peak_equity - current_equity) / peak_equity;

    match regime {
        "BULL" => {
            if dd < 0.08 {
                1.20
            } else if dd < 0.15 {
                0.85
            } else {
                0.50
            }
        }
        "DEFENSIVE" => {
            if dd < 0.06 {
                0.85
            } else if dd < 0.12 {
                0.55
            } else {
                0.25
            }
        }
        "CRASH" => {
            if dd < 0.05 {
                0.45
            } else {
                0.20
            }
        }
        _ => 0.75,
    }
}

fn select_holdings(
    assets: &[AssetSeries],
    idx: usize,
    regime: &str,
    exposure: f64,
) -> Vec<Holding> {
    let defensive = ["TLT", "GLD", "UUP"];
    let aggressive = ["SPY", "QQQ", "IWM", "DBC", "GLD", "BTC"];

    let mut scores: Vec<(String, f64)> = Vec::new();

    for asset in assets {
        let prices: Vec<f64> = asset.bars.iter().map(|b| b.price).collect();

        let allowed = match regime {
            "BULL" => aggressive.contains(&asset.symbol.as_str())
                || defensive.contains(&asset.symbol.as_str()),
            "CRASH" => defensive.contains(&asset.symbol.as_str()),
            _ => defensive.contains(&asset.symbol.as_str())
                || asset.symbol == "SPY"
                || asset.symbol == "QQQ",
        };

        if !allowed {
            continue;
        }

        let score = momentum_score(&prices, idx);
        scores.push((asset.symbol.clone(), score));
    }

    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let top_n = match regime {
        "BULL" => 3,
        "CRASH" => 2,
        _ => 2,
    };

    let selected: Vec<(String, f64)> = scores
        .into_iter()
        .filter(|(_, score)| *score > 0.0)
        .take(top_n)
        .collect();

    if selected.is_empty() {
        return Vec::new();
    }

    let weight = exposure / selected.len() as f64;

    selected
        .into_iter()
        .map(|(symbol, _)| Holding { symbol, weight })
        .collect()
}

fn run_radm_strategy(assets: &[AssetSeries]) -> PortfolioState {
    let len = aligned_len(assets);

    let mut state = PortfolioState {
        cash: INITIAL_CAPITAL,
        positions: HashMap::new(),
        equity_curve: Vec::new(),
        peak_equity: INITIAL_CAPITAL,
        trades: 0,
    };

    let spy = assets
        .iter()
        .find(|a| a.symbol == "SPY")
        .expect("SPY is required as regime benchmark");

    let spy_prices: Vec<f64> = spy.bars.iter().map(|b| b.price).collect();

    for idx in 252..len {
        let prices = latest_prices(assets, idx);
        let current_equity = portfolio_equity(state.cash, &state.positions, &prices);

        if current_equity > state.peak_equity {
            state.peak_equity = current_equity;
        }

        let is_rebalance_day = idx == 252 || idx % 21 == 0;

        if is_rebalance_day {
            let regime = determine_regime(&spy_prices, idx);
            let exposure = drawdown_exposure(current_equity, state.peak_equity, &regime);
            let holdings = select_holdings(assets, idx, &regime, exposure);

            rebalance(&mut state, &holdings, &prices, current_equity);
        }

        let updated_equity = portfolio_equity(state.cash, &state.positions, &prices);
        state.equity_curve.push(updated_equity);
    }

    state
}

fn run_equal_weight(assets: &[AssetSeries]) -> PortfolioState {
    let len = aligned_len(assets);

    let mut state = PortfolioState {
        cash: INITIAL_CAPITAL,
        positions: HashMap::new(),
        equity_curve: Vec::new(),
        peak_equity: INITIAL_CAPITAL,
        trades: 0,
    };

    for idx in 252..len {
        let prices = latest_prices(assets, idx);
        let current_equity = portfolio_equity(state.cash, &state.positions, &prices);

        let is_rebalance_day = idx == 252 || idx % 21 == 0;

        if is_rebalance_day {
            let weight = 1.0 / assets.len() as f64;
            let holdings: Vec<Holding> = assets
                .iter()
                .map(|a| Holding {
                    symbol: a.symbol.clone(),
                    weight,
                })
                .collect();

            rebalance(&mut state, &holdings, &prices, current_equity);
        }

        let updated_equity = portfolio_equity(state.cash, &state.positions, &prices);
        state.equity_curve.push(updated_equity);
    }

    state
}

fn run_spy_buy_hold(assets: &[AssetSeries]) -> PortfolioState {
    let len = aligned_len(assets);
    let spy = assets
        .iter()
        .find(|a| a.symbol == "SPY")
        .expect("SPY is required");

    let mut state = PortfolioState {
        cash: 0.0,
        positions: HashMap::new(),
        equity_curve: Vec::new(),
        peak_equity: INITIAL_CAPITAL,
        trades: 1,
    };

    let start_price = spy.bars[252].price;
    let qty = INITIAL_CAPITAL / start_price;
    state.positions.insert("SPY".to_string(), qty);

    for idx in 252..len {
        let price = spy.bars[idx].price;
        let mut prices = HashMap::new();
        prices.insert("SPY".to_string(), price);

        let equity = portfolio_equity(state.cash, &state.positions, &prices);
        state.equity_curve.push(equity);
    }

    state
}

fn load_universe() -> Vec<AssetSeries> {
    let specs = [
        ("SPY", "data/spy.csv"),
        ("QQQ", "data/qqq.csv"),
        ("IWM", "data/iwm.csv"),
        ("TLT", "data/tlt.csv"),
        ("GLD", "data/gld.csv"),
        ("DBC", "data/dbc.csv"),
        ("UUP", "data/uup.csv"),
        ("BTC", "data/btc.csv"),
    ];

    let mut assets = Vec::new();

    for (symbol, path) in specs {
        match load_csv(symbol, path) {
            Some(asset) => {
                println!("Loaded {} rows for {}", asset.bars.len(), symbol);
                assets.push(asset);
            }
            None => {
                println!("Skipping {} because {} is missing or too short", symbol, path);
            }
        }
    }

    assets
}

fn main() {
    println!("=== RADM: REGIME-ADAPTIVE DEFENSIVE MOMENTUM ===");

    let assets = load_universe();

    if assets.len() < 4 {
        println!("ERROR: Need at least 4 valid asset CSV files in backtester/data/");
        println!("Expected examples: data/spy.csv, data/qqq.csv, data/tlt.csv, data/gld.csv");
        return;
    }

    if !assets.iter().any(|a| a.symbol == "SPY") {
        println!("ERROR: SPY is required for market regime detection.");
        return;
    }

    let strategy_state = run_radm_strategy(&assets);
    let equal_weight_state = run_equal_weight(&assets);
    let spy_state = run_spy_buy_hold(&assets);

    let strategy_report = summarize("RADM_strategy", &strategy_state);
    let equal_weight_report = summarize("equal_weight_portfolio", &equal_weight_state);
    let spy_report = summarize("SPY_buy_and_hold", &spy_state);

    println!();
    println!("=== PERFORMANCE REPORT ===");
    print_report(&strategy_report);
    print_report(&equal_weight_report);
    print_report(&spy_report);

    println!();
    println!("=== STRATEGY INTERPRETATION ===");
    println!(
        "RADM combines volatility-adjusted momentum, SPY-based regime detection, monthly rebalancing, and a portfolio-level drawdown governor."
    );
    println!(
        "Goal: capture upside through cross-asset rotation while reducing drawdown during defensive and crash regimes."
    );

    println!();
    println!("=== RESEARCH GATE ===");

    let beats_spy_return = strategy_report.total_return_pct > spy_report.total_return_pct;
    let beats_equal_return = strategy_report.total_return_pct > equal_weight_report.total_return_pct;
    let lower_dd_than_spy = strategy_report.max_drawdown_pct < spy_report.max_drawdown_pct;
    let dd_close_to_equal =
    strategy_report.max_drawdown_pct <= equal_weight_report.max_drawdown_pct + 2.0;
    let sharpe_above_spy = strategy_report.sharpe > spy_report.sharpe;

    println!(
        "return > SPY: {} ({:.2}% vs {:.2}%)",
        beats_spy_return, strategy_report.total_return_pct, spy_report.total_return_pct
    );
    println!(
        "return > equal_weight: {} ({:.2}% vs {:.2}%)",
        beats_equal_return, strategy_report.total_return_pct, equal_weight_report.total_return_pct
    );
    println!(
        "drawdown < SPY: {} ({:.2}% vs {:.2}%)",
        lower_dd_than_spy, strategy_report.max_drawdown_pct, spy_report.max_drawdown_pct
    );
    println!(
        "drawdown < equal_weight: {} ({:.2}% vs {:.2}%)",
        dd_close_to_equal,
        strategy_report.max_drawdown_pct,
        equal_weight_report.max_drawdown_pct
    );
    println!(
        "sharpe > SPY: {} ({:.4} vs {:.4})",
        sharpe_above_spy, strategy_report.sharpe, spy_report.sharpe
    );

    let pass_count = [
    beats_spy_return,
    beats_equal_return,
    lower_dd_than_spy,
    dd_close_to_equal,
    sharpe_above_spy,
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    if pass_count >= 4 {
        println!("RESEARCH_GATE: PASS");
    } else {
        println!("RESEARCH_GATE: REVIEW");
    }
}