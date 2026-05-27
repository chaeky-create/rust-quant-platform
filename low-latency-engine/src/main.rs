use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
struct MarketEvent {
    price: f64,
    size: f64,
    ts_market: Instant,
}

#[derive(Debug, Clone, Copy)]
enum Signal {
    Long,
    Short,
    Flat,
}

#[derive(Debug, Clone, Copy)]
struct StrategyEvent {
    signal: Signal,
    price: f64,
    short_ma: f64,
    long_ma: f64,
    ts_market: Instant,
    ts_strategy: Instant,
}

#[derive(Debug, Clone, Copy)]
struct ExecutionEvent {
    signal: Signal,
    fill: &'static str,
    qty: f64,
    avg_price: f64,
    mark: f64,
    realized_pnl: f64,
    unrealized_pnl: f64,
    ts_market: Instant,
    ts_strategy: Instant,
    ts_execution: Instant,
}

#[derive(Debug, Clone, Copy)]
struct RiskEvent {
    risk_state: &'static str,
    total_pnl: f64,
    notional_exposure: f64,
    var_95: f64,
    expected_shortfall: f64,
    ts_market: Instant,
    ts_strategy: Instant,
    ts_execution: Instant,
    ts_risk: Instant,
}

#[derive(Debug)]
struct Position {
    qty: f64,
    avg_price: f64,
    realized_pnl: f64,
}

impl Position {
    fn new() -> Self {
        Self {
            qty: 0.0,
            avg_price: 0.0,
            realized_pnl: 0.0,
        }
    }

    fn buy(&mut self, price: f64, qty: f64) {
        let new_qty = self.qty + qty;

        self.avg_price = if new_qty.abs() > 0.0 {
            (self.avg_price * self.qty + price * qty) / new_qty
        } else {
            0.0
        };

        self.qty = new_qty;
    }

    fn sell(&mut self, price: f64, qty: f64) {
        if self.qty > 0.0 {
            let closing_qty = qty.min(self.qty);
            self.realized_pnl += closing_qty * (price - self.avg_price);
        }

        self.qty -= qty;

        if self.qty.abs() < 1e-9 {
            self.qty = 0.0;
            self.avg_price = 0.0;
        }
    }

    fn unrealized_pnl(&self, mark: f64) -> f64 {
        self.qty * (mark - self.avg_price)
    }
}

#[derive(Debug)]
struct LatencyStats {
    samples: Vec<u128>,
}

impl LatencyStats {
    fn new() -> Self {
        Self {
            samples: Vec::with_capacity(1_000_000),
        }
    }

    fn push(&mut self, ns: u128) {
        self.samples.push(ns);
    }

    fn summarize(&mut self, label: &str) {
        if self.samples.is_empty() {
            println!("{label}: no samples");
            return;
        }

        self.samples.sort_unstable();

        let count = self.samples.len();
        let min = self.samples[0];
        let max = self.samples[count - 1];
        let p50 = self.percentile(50.0);
        let p90 = self.percentile(90.0);
        let p99 = self.percentile(99.0);

        let sum: u128 = self.samples.iter().sum();
        let avg = sum as f64 / count as f64;

        println!("=== {label} ===");
        println!("samples: {}", count);
        println!("min:     {} ns ({:.3} µs)", min, min as f64 / 1_000.0);
        println!("avg:     {:.0} ns ({:.3} µs)", avg, avg / 1_000.0);
        println!("p50:     {} ns ({:.3} µs)", p50, p50 as f64 / 1_000.0);
        println!("p90:     {} ns ({:.3} µs)", p90, p90 as f64 / 1_000.0);
        println!("p99:     {} ns ({:.3} µs)", p99, p99 as f64 / 1_000.0);
        println!("max:     {} ns ({:.3} µs)", max, max as f64 / 1_000.0);
        println!();
    }

    fn percentile(&self, pct: f64) -> u128 {
        let idx = ((pct / 100.0) * (self.samples.len() as f64 - 1.0)).round() as usize;
        self.samples[idx]
    }
}

fn generate_market_event(i: usize) -> MarketEvent {
    let base = 100_000.0;
    let wave = ((i as f64) * 0.001).sin() * 100.0;
    let micro_noise = ((i * 17 % 101) as f64 - 50.0) * 0.01;

    MarketEvent {
        price: base + wave + micro_noise,
        size: 1.0 + ((i % 10) as f64) * 0.01,
        ts_market: Instant::now(),
    }
}

fn strategy_step(event: MarketEvent, prices: &mut Vec<f64>) -> Option<StrategyEvent> {
    prices.push(event.price);

    if prices.len() > 50 {
        prices.remove(0);
    }

    if prices.len() < 20 {
        return None;
    }

    let short_ma = prices[prices.len() - 5..].iter().sum::<f64>() / 5.0;
    let long_ma = prices[prices.len() - 20..].iter().sum::<f64>() / 20.0;

    let signal = if short_ma > long_ma {
        Signal::Long
    } else if short_ma < long_ma {
        Signal::Short
    } else {
        Signal::Flat
    };

    Some(StrategyEvent {
        signal,
        price: event.price,
        short_ma,
        long_ma,
        ts_market: event.ts_market,
        ts_strategy: Instant::now(),
    })
}

fn execution_step(event: StrategyEvent, position: &mut Position, last_signal: &mut Signal) -> ExecutionEvent {
    let mut fill = "NONE";

    let signal_changed = std::mem::discriminant(&event.signal) != std::mem::discriminant(last_signal);

    if signal_changed {
        match event.signal {
            Signal::Long => {
                position.buy(event.price, 1.0);
                fill = "BUY";
            }
            Signal::Short => {
                position.sell(event.price, 1.0);
                fill = "SELL";
            }
            Signal::Flat => {}
        }

        *last_signal = event.signal;
    }

    ExecutionEvent {
        signal: event.signal,
        fill,
        qty: position.qty,
        avg_price: position.avg_price,
        mark: event.price,
        realized_pnl: position.realized_pnl,
        unrealized_pnl: position.unrealized_pnl(event.price),
        ts_market: event.ts_market,
        ts_strategy: event.ts_strategy,
        ts_execution: Instant::now(),
    }
}

fn risk_step(event: ExecutionEvent) -> RiskEvent {
    let notional_exposure = event.qty.abs() * event.mark;
    let total_pnl = event.realized_pnl + event.unrealized_pnl;

    let max_notional_limit = 250_000.0;
    let max_loss_limit = -2_500.0;

    let risk_state = if notional_exposure > max_notional_limit {
        "LIMIT_BREACH"
    } else if total_pnl < max_loss_limit {
        "LOSS_LIMIT"
    } else if notional_exposure / max_notional_limit > 0.75 {
        "WARNING"
    } else {
        "OK"
    };

    let var_95 = notional_exposure * 0.02 * 1.65;
    let expected_shortfall = notional_exposure * 0.02 * 2.06;

    RiskEvent {
        risk_state,
        total_pnl,
        notional_exposure,
        var_95,
        expected_shortfall,
        ts_market: event.ts_market,
        ts_strategy: event.ts_strategy,
        ts_execution: event.ts_execution,
        ts_risk: Instant::now(),
    }
}

fn main() {
    let iterations = 1_000_000;

    let mut prices: Vec<f64> = Vec::with_capacity(50);
    let mut position = Position::new();
    let mut last_signal = Signal::Flat;

    let mut strategy_latency = LatencyStats::new();
    let mut execution_latency = LatencyStats::new();
    let mut risk_latency = LatencyStats::new();
    let mut end_to_end_latency = LatencyStats::new();

    let total_start = Instant::now();

    let mut processed = 0usize;
    let mut fills = 0usize;
    let mut warnings = 0usize;
    let mut breaches = 0usize;

    for i in 0..iterations {
        let market_event = generate_market_event(i);

        let Some(strategy_event) = strategy_step(market_event, &mut prices) else {
            continue;
        };

        let execution_event = execution_step(strategy_event, &mut position, &mut last_signal);

        let risk_event = risk_step(execution_event);

        strategy_latency.push(
            strategy_event
                .ts_strategy
                .duration_since(strategy_event.ts_market)
                .as_nanos(),
        );

        execution_latency.push(
            execution_event
                .ts_execution
                .duration_since(execution_event.ts_strategy)
                .as_nanos(),
        );

        risk_latency.push(
            risk_event
                .ts_risk
                .duration_since(risk_event.ts_execution)
                .as_nanos(),
        );

        end_to_end_latency.push(
            risk_event
                .ts_risk
                .duration_since(risk_event.ts_market)
                .as_nanos(),
        );

        processed += 1;

        if execution_event.fill != "NONE" {
            fills += 1;
        }

        if risk_event.risk_state == "WARNING" {
            warnings += 1;
        }

        if risk_event.risk_state == "LIMIT_BREACH" || risk_event.risk_state == "LOSS_LIMIT" {
            breaches += 1;
        }
    }

    let total_elapsed = total_start.elapsed();

    println!();
    println!("=== Low Latency Engine Benchmark ===");
    println!("iterations:        {}", iterations);
    println!("processed events:  {}", processed);
    println!("fills:             {}", fills);
    println!("warnings:          {}", warnings);
    println!("breaches:          {}", breaches);
    println!("total runtime:     {:.3} ms", total_elapsed.as_secs_f64() * 1000.0);
    println!(
        "throughput:        {:.0} events/sec",
        processed as f64 / total_elapsed.as_secs_f64()
    );
    println!();

    strategy_latency.summarize("Market → Strategy Latency");
    execution_latency.summarize("Strategy → Execution Latency");
    risk_latency.summarize("Execution → Risk Latency");
    end_to_end_latency.summarize("End-to-End Latency");
}