# Rust Quant Platform

A research-oriented quantitative trading platform built in Rust, focused on low-latency strategy execution, backtesting, risk control, and robustness validation.

This project currently includes:

- A BTC single-asset trend-following backtester
- Parameter grid optimization
- Walk-forward validation
- Parameter cube robustness analysis
- A synthetic cross-asset relative momentum portfolio prototype
- Early infrastructure for multi-asset data ingestion

> This repository is for quantitative research and engineering practice. It is not financial advice and should not be used for live trading without further validation, transaction cost modeling, and risk controls.

---

## Project Motivation

The goal of this project is to build a research-grade quant platform that connects:

1. Strategy research  
2. Backtesting  
3. Risk management  
4. Robustness testing  
5. Portfolio construction  
6. Low-latency Rust engineering  

The current research focus is on whether simple momentum and trend-following rules can produce defensive, risk-controlled behavior compared with buy-and-hold benchmarks.

---

## Repository Structure

```text
rust-quant-platform/
├── backtester/
│   ├── data/
│   │   └── btc.csv
│   ├── scripts/
│   │   └── download_etf_data.py
│   └── src/bin/
│       ├── research_backtest.rs
│       └── cross_asset_momentum.rs
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