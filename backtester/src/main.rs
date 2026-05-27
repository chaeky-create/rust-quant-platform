use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug, Clone)]
struct Bar {
    time: usize,
    price: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct BacktestReport {
    final_equity: f64,
    total_return_pct: f64,
    max_drawdown_pct: f64,
    sharpe_ratio: f64,
    trades: usize,
    best_short_ma: usize,
    best_long_ma: usize,
    equity_curve: Vec<EquityPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EquityPoint {
    step: usize,
    equity: f64,
}

#[derive(Debug)]
struct BacktestState {
    cash: f64,
    position: f64,
    entry_price: f64,
    equity_curve: Vec<f64>,
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

    fn buy(&mut self, price: f64) {
        if self.position <= 0.0 {
            let fill_price = apply_slippage(price, "BUY");
            self.cash -= commission(fill_price, 1.0);
            self.position = 1.0;
            self.entry_price = fill_price;
            self.trades += 1;
        }
    }

    fn sell(&mut self, price: f64) {
        if self.position >= 0.0 {
            let fill_price = apply_slippage(price, "SELL");
            self.cash -= commission(fill_price, 1.0);
            self.position = -1.0;
            self.entry_price = fill_price;
            self.trades += 1;
        }
    }

    fn mark_to_market(&mut self, price: f64) {
        let equity = self.cash + self.position * (price - self.entry_price);
        self.equity_curve.push(equity);
    }
}

fn apply_slippage(price: f64, side: &str) -> f64 {
    let bps = 5.0 / 10_000.0;

    match side {
        "BUY" => price * (1.0 + bps),
        "SELL" => price * (1.0 - bps),
        _ => price,
    }
}

fn commission(price: f64, qty: f64) -> f64 {
    price * qty.abs() * 0.0004
}

fn moving_average(prices: &[f64]) -> f64 {
    prices.iter().sum::<f64>() / prices.len() as f64
}

fn max_drawdown(equity: &[f64]) -> f64 {
    let mut peak = equity[0];
    let mut max_dd = 0.0;

    for value in equity {
        if *value > peak {
            peak = *value;
        }

        let dd = (peak - value) / peak;

        if dd > max_dd {
            max_dd = dd;
        }
    }

    max_dd
}

fn sharpe_ratio(equity: &[f64]) -> f64 {
    let returns: Vec<f64> = equity
        .windows(2)
        .map(|w| (w[1] - w[0]) / w[0])
        .collect();

    if returns.is_empty() {
        return 0.0;
    }

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;

    let variance =
        returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
            / returns.len() as f64;

    let std = variance.sqrt();

    if std == 0.0 {
        0.0
    } else {
        (mean / std) * 252.0_f64.sqrt()
    }
}

fn load_csv(path: &str) -> Vec<Bar> {
    let mut rdr = csv::Reader::from_path(path).expect("Failed to open CSV");
    let mut bars = Vec::new();

    for result in rdr.records() {
        let record = result.expect("Failed to read row");

        bars.push(Bar {
            time: record[0].parse::<usize>().unwrap_or(0),
            price: record[1].parse::<f64>().unwrap_or(0.0),
        });
    }

    bars
}

fn run_backtest(
    bars: &[Bar],
    short_window: usize,
    long_window: usize,
) -> (f64, f64, f64, usize, Vec<f64>) {
    let mut state = BacktestState::new();
    let mut prices = Vec::new();

    for bar in bars {
        prices.push(bar.price);

        if prices.len() < long_window {
            state.mark_to_market(bar.price);
            continue;
        }

        let short_ma = moving_average(&prices[prices.len() - short_window..]);
        let long_ma = moving_average(&prices[prices.len() - long_window..]);

        if short_ma > long_ma {
            state.buy(bar.price);
        } else if short_ma < long_ma {
            state.sell(bar.price);
        }

        state.mark_to_market(bar.price);
    }

    let final_equity = *state.equity_curve.last().unwrap_or(&100_000.0);
    let total_return = (final_equity - 100_000.0) / 100_000.0;
    let max_dd = max_drawdown(&state.equity_curve);
    let sharpe = sharpe_ratio(&state.equity_curve);

    (
        total_return,
        sharpe,
        max_dd,
        state.trades,
        state.equity_curve,
    )
}

fn optimize_backtest() -> BacktestReport {
    let bars = load_csv("data/btc.csv");

    let mut best_sharpe = -999.0;
    let mut best_short = 0;
    let mut best_long = 0;
    let mut best_return = 0.0;
    let mut best_dd = 0.0;
    let mut best_trades = 0;
    let mut best_equity_curve = Vec::new();

    for short in 5..30 {
        for long in 20..100 {
            if short >= long {
                continue;
            }

            let (ret, sharpe, dd, trades, equity_curve) =
                run_backtest(&bars, short, long);

            if sharpe > best_sharpe {
                best_sharpe = sharpe;
                best_short = short;
                best_long = long;
                best_return = ret;
                best_dd = dd;
                best_trades = trades;
                best_equity_curve = equity_curve;
            }
        }
    }

    let final_equity = *best_equity_curve.last().unwrap_or(&100_000.0);

    let equity_curve = best_equity_curve
        .iter()
        .enumerate()
        .map(|(i, equity)| EquityPoint {
            step: i,
            equity: *equity,
        })
        .collect();

    BacktestReport {
        final_equity,
        total_return_pct: best_return * 100.0,
        max_drawdown_pct: best_dd * 100.0,
        sharpe_ratio: best_sharpe,
        trades: best_trades,
        best_short_ma: best_short,
        best_long_ma: best_long,
        equity_curve,
    }
}

async fn backtest_endpoint() -> Json<BacktestReport> {
    Json(optimize_backtest())
}

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/backtest", get(backtest_endpoint))
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], 9401));

    println!("Backtester API running on http://127.0.0.1:9401/backtest");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind backtester API");

    axum::serve(listener, app)
        .await
        .expect("Backtester API failed");
}