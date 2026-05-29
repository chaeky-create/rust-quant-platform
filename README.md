# Rust Quant Platform

A research-oriented quantitative trading platform built in Rust, focused on systematic strategy research, backtesting, risk control, robustness validation, and portfolio construction.

This repository currently includes:

- BTC single-asset defensive trend-following backtester
- Parameter grid optimization
- Parameter cube robustness analysis
- Walk-forward validation
- Cross-asset tactical allocation research
- RADM: Regime-Adaptive Defensive Momentum strategy
- Ablation testing
- Transaction-cost stress testing
- Rolling walk-forward validation
- Early infrastructure for multi-asset data ingestion and low-latency Rust engineering

> This repository is for quantitative research and engineering practice. It is not financial advice and should not be used for live trading without further validation, transaction cost modeling, operational risk controls, and independent review.

---

## Project Motivation

The goal of this project is to build a research-grade quantitative trading platform that connects strategy research, backtesting, risk management, robustness testing, portfolio construction, and low-latency Rust engineering.

The project began with a single-asset BTC trend-following baseline. That baseline showed strong downside control but weak upside participation. To address that limitation, the project was extended into a cross-asset tactical allocation strategy that rotates across equities, bonds, gold, commodities, USD, and BTC.

The current main research strategy is **RADM: Regime-Adaptive Defensive Momentum**.

---

## Repository Structure

```text
rust-quant-platform/
├── backtester/
│   ├── data/
│   │   ├── btc.csv
│   │   ├── spy.csv
│   │   ├── qqq.csv
│   │   ├── iwm.csv
│   │   ├── tlt.csv
│   │   ├── gld.csv
│   │   ├── dbc.csv
│   │   └── uup.csv
│   ├── scripts/
│   │   ├── download_etf_data.py
│   │   └── download_yfinance_data.py
│   ├── src/bin/
│   │   ├── research_backtest.rs
│   │   ├── cross_asset_momentum.rs
│   │   └── regime_cross_asset_strategy.rs
│   ├── backtest_report.json
│   ├── walk_forward_report.json
│   └── equity_curve.csv
├── execution-engine/
├── feature-engine/
├── risk-engine/
├── strategy-engine/
├── low-latency-engine/
├── market-data/
├── shared-types/
├── storage-engine/
├── infras/
└── frontend/
```

---

## Main Strategy: RADM

**RADM** stands for **Regime-Adaptive Defensive Momentum**.

RADM is a cross-asset tactical allocation strategy implemented in Rust. It combines volatility-adjusted momentum ranking, SPY-based market regime detection, monthly rebalancing, and a portfolio-level drawdown governor.

The strategy is designed to solve a common weakness of single-asset trend-following systems: strong downside control but weak upside participation. Instead of trading only one asset, RADM rotates capital across multiple liquid assets depending on market regime and relative momentum.

---

## RADM Strategy Logic

### Universe

RADM uses the following asset universe:

| Symbol | Asset |
|---|---|
| SPY | U.S. large-cap equities |
| QQQ | Nasdaq / growth equities |
| IWM | U.S. small-cap equities |
| TLT | Long-duration U.S. Treasuries |
| GLD | Gold |
| DBC | Broad commodities |
| UUP | U.S. dollar index ETF |
| BTC | Bitcoin |

### Signal

Each asset is ranked using volatility-adjusted multi-horizon momentum.

Conceptually:

```text
momentum_score =
    weighted 3-month return
  + weighted 6-month return
  + weighted 12-month return

risk_adjusted_score =
    momentum_score / realized_volatility
```

This favors assets that are rising efficiently, not simply assets that are volatile.

### Regime Filter

The strategy uses SPY as a market regime benchmark.

The regime filter separates market behavior into broad environments such as:

- Bull regime
- Defensive regime
- Crash regime

The strategy becomes more aggressive during stronger equity regimes and more defensive when market conditions deteriorate.

### Drawdown Governor

RADM includes a portfolio-level drawdown governor.

When portfolio drawdown increases, the strategy automatically reduces exposure. This is designed to prevent the system from continuing to take full risk during unstable or deteriorating market conditions.

### Rebalancing

The portfolio is rebalanced monthly.

This reduces unnecessary turnover while still allowing the strategy to respond to changing market regimes and cross-asset momentum leadership.

---

## Baseline: BTC Defensive Trend-Following

The project first implemented a BTC single-asset trend-following baseline. The purpose of this baseline was to test whether simple trend and volatility filters could reduce drawdown relative to buy-and-hold.

### BTC Baseline Out-of-Sample Test

| Strategy | Total Return | Max Drawdown | Sharpe | Trades |
|---|---:|---:|---:|---:|
| BTC Buy-and-Hold | -23.06% | 49.16% | -0.1536 | 2 |
| Optimized BTC Strategy | 1.94% | 3.21% | 0.2690 | 74 |

The BTC baseline showed strong downside control but weak upside capture. This motivated the move from single-asset trend-following to cross-asset tactical allocation.

---

## RADM Full-Period Backtest Results

| Strategy | Total Return | Max Drawdown | Sharpe | Trades |
|---|---:|---:|---:|---:|
| RADM Strategy | 120.86% | 16.44% | 0.9765 | 213 |
| Equal-Weight Portfolio | 100.78% | 15.85% | 1.2190 | 592 |
| SPY Buy-and-Hold | 137.80% | 33.72% | 0.8719 | 1 |

### Interpretation

RADM did not exceed SPY's raw total return, but it reduced maximum drawdown from 33.72% to 16.44% while achieving a higher Sharpe ratio than SPY.

It also outperformed the equal-weight cross-asset portfolio in total return with substantially fewer trades.

This makes RADM better interpreted as a **risk-controlled tactical allocation strategy**, not as a pure equity benchmark-beating strategy.

---

## Research Gate

RADM passed the research gate by satisfying 4 out of 5 criteria:

| Criterion | Result |
|---|---|
| Return > SPY | False |
| Return > Equal-Weight Portfolio | True |
| Drawdown < SPY | True |
| Drawdown <= Equal-Weight + 2% | True |
| Sharpe > SPY | True |

The strategy passed because it improved return versus equal-weight, reduced drawdown versus SPY, kept drawdown close to equal-weight, and achieved higher Sharpe than SPY.

---

## Ablation Study

To test whether RADM's performance came from the actual strategy logic rather than simple diversification, the project includes an ablation study.

| Variant | Total Return | Max Drawdown | Sharpe | Trades |
|---|---:|---:|---:|---:|
| RADM Full | 120.86% | 16.44% | 0.9765 | 213 |
| No Regime Filter | 90.03% | 15.43% | 0.7727 | 219 |
| No Drawdown Governor | 130.81% | 18.31% | 0.9730 | 213 |
| No Momentum Rotation | 110.10% | 14.74% | 1.2160 | 592 |

### Ablation Interpretation

The ablation study shows:

- The regime filter materially improves total return.
- Removing the regime filter reduces return from 120.86% to 90.03%.
- Removing the drawdown governor increases return to 130.81%, but also increases max drawdown to 18.31%.
- Momentum rotation improves total return versus the non-rotating version.
- The no-momentum-rotation variant produced the highest Sharpe ratio, showing that simple diversification remains a strong baseline.

This is an important limitation: RADM improves total return, but equal-weight-style diversification remains very competitive on risk-adjusted performance.

---

## Transaction Cost Stress Test

RADM was tested under transaction-cost assumptions.

| Scenario | Total Return | Max Drawdown | Sharpe | Trades |
|---|---:|---:|---:|---:|
| 0 bps | 120.86% | 16.44% | 0.9765 | 213 |
| 5 bps | 112.33% | 16.60% | 0.9320 | 213 |
| 10 bps | 103.81% | 16.89% | 0.8865 | 213 |

### Cost Stress Interpretation

RADM remains profitable under moderate transaction-cost assumptions, although performance declines as trading costs rise.

The strategy's total return decreases from 120.86% to 103.81% under the 10 bps stress scenario, while still maintaining a lower drawdown profile than SPY buy-and-hold.

---

## Rolling Walk-Forward Validation

To reduce reliance on a single full-period backtest, RADM was evaluated using rolling walk-forward windows.

| Metric | Result |
|---|---:|
| Windows | 6 |
| Average RADM Return | 14.08% |
| Average Equal-Weight Return | 12.90% |
| Average SPY Return | 17.31% |
| Average RADM Max Drawdown | 10.35% |
| Average SPY Max Drawdown | 12.77% |
| Average RADM Sharpe | 0.9448 |
| Positive RADM Windows | 6/6 |
| RADM Beats Equal-Weight Return Windows | 3/6 |
| RADM Beats SPY Return Windows | 2/6 |
| RADM Lower Drawdown Than SPY Windows | 4/6 |

### Walk-Forward Interpretation

The walk-forward validation shows that RADM produced positive returns in all validation windows and achieved a higher average return than the equal-weight cross-asset portfolio.

It did not consistently outperform SPY on raw return, but it reduced average maximum drawdown relative to SPY and produced a more risk-controlled return profile.

RADM should therefore be interpreted as a **risk-controlled cross-asset allocation strategy**, not a pure return-maximizing equity strategy.

---

## Key Findings

1. A single-asset BTC defensive strategy can reduce drawdown but struggles to capture upside.
2. Cross-asset allocation improves participation across market regimes.
3. RADM outperforms equal-weight on total return in the full-period test.
4. RADM reduces SPY drawdown by more than 50% in the full-period test.
5. RADM produces positive returns across all walk-forward windows.
6. The regime filter improves return meaningfully.
7. The drawdown governor lowers risk but sacrifices some upside.
8. Transaction costs reduce performance but do not fully eliminate profitability under tested assumptions.
9. Equal-weight diversification remains a strong risk-adjusted benchmark.
10. RADM is promising as a research prototype, but not yet suitable for live trading.

---

## How to Run

### 1. Run the BTC Research Backtest

```bash
cd backtester
cargo run --release --bin research_backtest
```

### 2. Run the RADM Cross-Asset Strategy

```bash
cd backtester
cargo run --release --bin regime_cross_asset_strategy
```

---

## Data

The strategy expects CSV files in:

```text
backtester/data/
```

Expected files:

```text
spy.csv
qqq.csv
iwm.csv
tlt.csv
gld.csv
dbc.csv
uup.csv
btc.csv
```

Expected CSV format:

```csv
time,price
0,100.00
1,101.25
2,100.80
```

ETF data can be downloaded using the included Python scripts.

Example:

```bash
cd backtester
python3 scripts/download_yfinance_data.py
```

---

## Engineering Notes

This project is written primarily in Rust to practice quantitative engineering with:

- Fast compiled execution
- Strong type safety
- Explicit memory and data handling
- Modular strategy development
- Backtesting and research tooling

Current implementation is research-oriented and not yet production-grade.

Planned infrastructure components include:

- `market-data`
- `feature-engine`
- `strategy-engine`
- `risk-engine`
- `execution-engine`
- `low-latency-engine`
- `storage-engine`
- `shared-types`
- `frontend`

---

## Current Limitations

This project is still a research prototype.

Important limitations:

- No live trading integration
- No broker or exchange execution layer
- No order book simulation
- No intraday data validation
- No tax modeling
- Simplified transaction cost assumptions
- Limited asset universe
- BTC data length differs from ETF data length
- Results are sensitive to benchmark choice
- Equal-weight remains stronger on Sharpe ratio in the full-period test
- Walk-forward validation shows inconsistent SPY outperformance on raw return

---

## Future Work

Planned improvements:

- Add stronger walk-forward parameter selection
- Add expanding-window and rolling-window optimization
- Add per-asset volatility targeting
- Add BTC-specific risk controls
- Add richer transaction cost and slippage modeling
- Add turnover reporting
- Add benchmark-relative attribution
- Add drawdown attribution
- Add regime-specific performance breakdown
- Add CSV/JSON report export for RADM
- Add charts for equity curve and drawdown curve
- Refactor strategy logic into reusable modules
- Expand asset universe beyond 8 assets
- Add CI tests for strategy components
- Build frontend dashboard for backtest visualization

---

## Portfolio Summary

This project demonstrates the design and implementation of a Rust-based quantitative research platform with a custom cross-asset allocation strategy.

The main strategy, RADM, combines:

- Volatility-adjusted momentum ranking
- Market regime detection
- Monthly rebalancing
- Portfolio-level drawdown control
- Ablation testing
- Transaction-cost stress testing
- Rolling walk-forward validation

RADM achieved positive returns across all walk-forward windows, outperformed equal-weight on average walk-forward return, and reduced average drawdown relative to SPY.

---

## Disclaimer

This project is for research, education, and engineering practice only.

It is not financial advice, investment advice, or a recommendation to trade any asset. The backtests are historical simulations and may not reflect future performance. Live trading would require substantially more validation, risk management, execution modeling, and regulatory review.