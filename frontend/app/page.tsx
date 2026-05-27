"use client";

import { useEffect, useState } from "react";

import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
} from "recharts";

type Result = Record<string, string | number>;

interface MarketTick {
  symbol: string;
  price: number;
  size: number;
  timestamp: string;
}

interface Candle {
  symbol?: string;
  timeframe?: string;
  second: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

interface MarketEvent {
  event_type: "tick" | "candle" | "order_book";
  data: MarketTick | Candle | OrderBookSnapshot;
}

interface SurfaceCell {
  strike: number;
  vol: number;
  price: number;
}

interface PricingSurfacePoint {
  strike: number;
  volatility: number;
  price: number;
  delta: number;
  gamma: number;
  vega: number;
}

interface OrderBookLevel {
  price: number;
  size: number;
}

interface OrderBookSnapshot {
  symbol: string;
  bids: OrderBookLevel[];
  asks: OrderBookLevel[];
}

interface StrategySignal {
  signal: "LONG" | "SHORT" | "FLAT";
  price: number;
  short_ma: number;
  long_ma: number;
  timestamp: string;
}

interface ExecutionState {
  signal: string;
  fill: string;
  qty: number;
  avg_price: number;
  mark: number;
  realized_pnl: number;
  unrealized_pnl: number;
  timestamp: string;
}

interface RiskState {
  risk_state: string;
  qty: number;
  mark: number;
  notional_exposure: number;
  exposure_utilization: number;
  realized_pnl: number;
  unrealized_pnl: number;
  total_pnl: number;
  var_95: number;
  expected_shortfall: number;
}

export default function Home() {
  const [spot, setSpot] = useState(100);
  const [strike, setStrike] = useState(110);
  const [rate, setRate] = useState(0.05);
  const [volatility, setVolatility] = useState(0.2);
  const [maturity, setMaturity] = useState(1);
  const [simulations, setSimulations] = useState(1000000);
  const [marketPrice, setMarketPrice] = useState(6.04);

  const [btc, setBtc] = useState<MarketTick | null>(null);
  const [wsStatus, setWsStatus] = useState("Disconnected");
  const [apiStatus, setApiStatus] = useState("Unknown");

  const [ticks, setTicks] = useState<MarketTick[]>([]);
  const [candles, setCandles] = useState<Candle[]>([]);
  const [surfaceData, setSurfaceData] = useState<SurfaceCell[]>([]);
  const [pricingSurface, setPricingSurface] = useState<PricingSurfacePoint[]>([]);

  const [title, setTitle] = useState("No result yet");
  const [result, setResult] = useState<Result | null>(null);
  const [latency, setLatency] = useState<number | null>(null);

  const [orderBook, setOrderBook] = useState<OrderBookSnapshot | null>(null);

  const [greeksHistory, setGreeksHistory] = useState<
  { time: string; delta: number; gamma: number; vega: number }[]
>([]);

const [smileData, setSmileData] = useState<
  { strike: number; impliedVol: number }[]
>([]);

const [position, setPosition] = useState(0);
const [entryPrice, setEntryPrice] = useState<number | null>(null);

const [strategy, setStrategy] = useState<StrategySignal | null>(null);
const [strategyStatus, setStrategyStatus] = useState("Disconnected");

const [execution, setExecution] = useState<ExecutionState | null>(null);
const [executionStatus, setExecutionStatus] = useState("Disconnected");

const [risk, setRisk] = useState<RiskState | null>(null);
const [riskStatus, setRiskStatus] = useState("Disconnected");

const [equityCurve, setEquityCurve] = useState<
  { time: string; equity: number; drawdown: number }[]
>([]);

const [sharpe, setSharpe] = useState(0);
const [rollingVolatility, setRollingVolatility] = useState(0);
const [winRate, setWinRate] = useState(0);

const [tradeLog, setTradeLog] = useState<
  {
    time: string;
    side: string;
    price: number;
    qty: number;
    pnl: number;
  }[]
>([]);

const [backtest, setBacktest] = useState<any>(null);

  function query() {
    return `spot=${spot}&strike=${strike}&rate=${rate}&volatility=${volatility}&maturity=${maturity}`;
  }

  useEffect(() => {
    const socket = new WebSocket("ws://127.0.0.1:9001/ws/btc");

    socket.onopen = () => setWsStatus("Connected");
    socket.onerror = () => setWsStatus("Error");
    socket.onclose = () => setWsStatus("Disconnected");

    socket.onmessage = (event) => {
      const message: MarketEvent = JSON.parse(event.data);

      if (message.event_type === "tick") {
        const tick = message.data as MarketTick;
        setBtc(tick);
        setTicks((prev) => [...prev.slice(-49), tick]);
      }

      if (message.event_type === "candle") {
        const candle = message.data as Candle;
        setCandles((prev) => {
          const latest = prev[prev.length - 1];
          if (!latest || latest.second !== candle.second) {
            return [...prev.slice(-59), candle];
          }
          return [...prev.slice(0, -1), candle];
        });
      }

      if (message.event_type === "order_book") {
        const book = message.data as OrderBookSnapshot;
        setOrderBook(book);
      }
    };

    return () => socket.close();
  }, []);

  useEffect(() => {
    async function pingBackend() {
      try {
        const start = performance.now();

        const res = await fetch(
          "http://127.0.0.1:8080/price?spot=100&strike=110&rate=0.05&volatility=0.2&maturity=1"
        );

        await res.json();

        const end = performance.now();

        setApiStatus(res.ok ? "Online" : "Error");
        setLatency(end - start);
      } catch {
        setApiStatus("Offline");
        setLatency(null);
      }
    }

    pingBackend();
    const interval = setInterval(pingBackend, 3000);

    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    const socket = new WebSocket("ws://127.0.0.1:9101/ws/strategy");

    socket.onopen = () => setStrategyStatus("Connected");
    socket.onerror = () => setStrategyStatus("Error");
    socket.onclose = () => setStrategyStatus("Disconnected");

    socket.onmessage = (event) => {
      const data: StrategySignal = JSON.parse(event.data);
      setStrategy(data);
    };

    return () => socket.close();
  }, []);

  useEffect(() => {
    const socket = new WebSocket("ws://127.0.0.1:9201/ws/execution");

    socket.onopen = () => setExecutionStatus("Connected");
    socket.onerror = () => setExecutionStatus("Error");
    socket.onclose = () => setExecutionStatus("Disconnected");

    socket.onmessage = (event) => {
      const data: ExecutionState = JSON.parse(event.data);
      setExecution(data);

      if (data.fill !== "NONE") {
        setTradeLog((prev) => [
          {
            time: new Date().toLocaleTimeString(),
            side: data.fill,
            price: data.mark,
            qty: data.qty,
            pnl: data.realized_pnl + data.unrealized_pnl,
          },
          ...prev.slice(0, 24),
        ]);
      }
    };

    return () => socket.close();
  }, []);

  useEffect(() => {
    const socket = new WebSocket("ws://127.0.0.1:9301/ws/risk");

    socket.onopen = () => setRiskStatus("Connected");
    socket.onerror = () => setRiskStatus("Error");
    socket.onclose = () => setRiskStatus("Disconnected");

    socket.onmessage = (event) => {
      const data: RiskState = JSON.parse(event.data);
      setRisk(data);

      setEquityCurve((prev) => {
        const equity = data.total_pnl;
        const peak = Math.max(equity, ...prev.map((x) => x.equity));
        const drawdown = peak !== 0 ? equity - peak : 0;

        return [
          ...prev.slice(-99),
          {
            time: new Date().toLocaleTimeString(),
            equity,
            drawdown,
          },
        ];
      });
    };

    return () => socket.close();
  }, []);

  useEffect(() => {
    async function loadBacktest() {
      try {
        const res = await fetch("http://127.0.0.1:9401/backtest");
        const data = await res.json();
        setBacktest(data);
      } catch (err) {
        console.error(err);
      }
    }

    loadBacktest();
  }, []);

  useEffect(() => {
    const returns = equityCurve.map((x, i, arr) =>
      i === 0 ? 0 : x.equity - arr[i - 1].equity
    );

    if (returns.length > 5) {
      const mean = returns.reduce((sum, r) => sum + r, 0) / returns.length;

      const variance =
        returns.reduce((sum, r) => sum + Math.pow(r - mean, 2), 0) /
        returns.length;

      const std = Math.sqrt(variance);

      setRollingVolatility(std);

      if (std > 0) {
        setSharpe((mean / std) * Math.sqrt(252));
      }

      const wins = returns.filter((r) => r > 0).length;
      setWinRate((wins / returns.length) * 100);
    }
  }, [equityCurve]);




   async function fetchAndMeasure(label: string, endpoint: string) {
    const start = performance.now();
    const res = await fetch(endpoint);
    const data = await res.json();
    const end = performance.now();

    setTitle(label);
    setResult(data);
    setLatency(end - start);

    if (label === "Greeks") {
      setGreeksHistory((prev) => [
        ...prev.slice(-29),
        {
          time: new Date().toLocaleTimeString(),
          delta: Number(data.delta ?? 0),
          gamma: Number(data.gamma ?? 0),
          vega: Number(data.vega ?? 0),
        },
      ]);
    }
  }

  async function generateSurface() {
    const vols = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
    const strikes = [80, 90, 100, 110, 120, 130];
    const results: SurfaceCell[] = [];

    for (const vol of vols) {
      for (const k of strikes) {
        const res = await fetch(
          `http://127.0.0.1:8080/price?spot=${spot}&strike=${k}&rate=${rate}&volatility=${vol}&maturity=${maturity}`
        );

        const data = await res.json();

        results.push({
          strike: k,
          vol,
          price: data.price,
        });
      }
    }

    setSurfaceData(results);

    setSmileData(
      results
        .filter((x) => x.vol === volatility)
        .map((x) => ({
          strike: x.strike,
          impliedVol: x.price,
        }))
    );
  }

  async function generatePricingSurface() {
    const res = await fetch(
      `http://127.0.0.1:9501/surface?spot=${spot}&strike=${strike}&rate=${rate}&volatility=${volatility}&maturity=${maturity}`
    );

    const data = await res.json();

    setPricingSurface(data.points);
  }

  const fields = [
    ["Spot", spot, setSpot],
    ["Strike", strike, setStrike],
    ["Rate", rate, setRate],
    ["Volatility", volatility, setVolatility],
    ["Maturity", maturity, setMaturity],
    ["Simulations", simulations, setSimulations],
    ["Market Price", marketPrice, setMarketPrice],
  ] as const;

  const tickChartData = ticks.map((t, i) => ({
    tick: i,
    price: t.price,
  }));

  const mid = btc?.price ?? 0;
  const bids = orderBook?.bids.slice(0, 10) ?? [];
  const asks = orderBook?.asks.slice(0, 10) ?? [];

  const depthData = bids
    .slice()
    .reverse()
    .map((_, i) => ({
      level: i + 1,
      bidDepth: bids.slice(0, i + 1).reduce((sum, b) => sum + b.size, 0),
      askDepth: asks.slice(0, i + 1).reduce((sum, a) => sum + a.size, 0),
    }));

  const bestBid = bids[0]?.price ?? 0;
  const bestAsk = asks[0]?.price ?? 0;
  const spread = bestAsk && bestBid ? bestAsk - bestBid : 0;

  const totalBidDepth = bids.reduce((sum, b) => sum + b.size, 0);
  const totalAskDepth = asks.reduce((sum, a) => sum + a.size, 0);

  const imbalance =
    totalBidDepth + totalAskDepth > 0
      ? (totalBidDepth - totalAskDepth) / (totalBidDepth + totalAskDepth)
      : 0;

  const microprice =
    totalBidDepth + totalAskDepth > 0
      ? (bestAsk * totalBidDepth + bestBid * totalAskDepth) /
        (totalBidDepth + totalAskDepth)
      : 0;

  const unrealizedPnl =
    btc && entryPrice !== null ? position * (btc.price - entryPrice) : 0;

  return (
    <main className="min-h-screen bg-black text-white px-8 py-8">
      <div className="max-w-7xl mx-auto space-y-8">
        <header>
          <h1 className="text-5xl font-bold">Rust Quant Platform</h1>
          <p className="text-zinc-400 mt-2">
            Real-time market data, order book ladder, candles, derivatives
            pricing, Greeks, Monte Carlo, and volatility surface analytics.
          </p>
        </header>

        <section className="grid grid-cols-1 lg:grid-cols-4 gap-5">
          <div className="lg:col-span-2 bg-zinc-950 border border-emerald-800 rounded-3xl p-6">
            <p className="text-zinc-400 uppercase tracking-widest text-sm">
              Live Market Data
            </p>
            <h2 className="text-3xl font-bold mt-3">
              {btc?.symbol ?? "BTC-USD"}
            </h2>
            <p className="text-green-400 text-5xl font-mono mt-4">
              {btc ? `$${btc.price.toFixed(2)}` : "--"}
            </p>
            <p className="text-zinc-400 mt-3">Size: {btc ? btc.size : "--"}</p>
            <p className="text-zinc-500 text-sm mt-1">
              Last update: {btc?.timestamp ?? "--"}
            </p>
          </div>

          <div className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
            <p className="text-zinc-400 uppercase tracking-widest text-sm">
              WebSocket Status
            </p>
            <p className="text-green-400 text-3xl font-mono mt-4">
              {wsStatus}
            </p>
          </div>

          <div className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
            <p className="text-zinc-400 uppercase tracking-widest text-sm">
              Backend API
            </p>
            <p className="text-green-400 text-3xl font-mono mt-4">
              {apiStatus}
            </p>
            <p className="text-zinc-400 mt-4 uppercase tracking-widest text-sm">
              Latency
            </p>
            <p className="text-green-400 text-2xl font-mono mt-2">
              {latency === null ? "--" : `${latency.toFixed(3)} ms`}
            </p>
          </div>
        </section>

        <section className="bg-zinc-950 border border-purple-800 rounded-3xl p-6">
          <p className="text-zinc-400 uppercase tracking-widest text-sm">
            Strategy Engine
          </p>

          <div className="grid grid-cols-1 md:grid-cols-5 gap-4 mt-4">
            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Status
              </p>
              <p className="text-green-400 font-mono text-xl mt-2">
                {strategyStatus}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Signal
              </p>
              <p
                className={`font-mono text-2xl mt-2 ${
                  strategy?.signal === "LONG"
                    ? "text-green-400"
                    : strategy?.signal === "SHORT"
                    ? "text-red-400"
                    : "text-zinc-400"
                }`}
              >
                {strategy?.signal ?? "--"}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Price
              </p>
              <p className="text-green-400 font-mono text-xl mt-2">
                {strategy ? strategy.price.toFixed(2) : "--"}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Short MA
              </p>
              <p className="text-purple-400 font-mono text-xl mt-2">
                {strategy ? strategy.short_ma.toFixed(2) : "--"}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Long MA
              </p>
              <p className="text-blue-400 font-mono text-xl mt-2">
                {strategy ? strategy.long_ma.toFixed(2) : "--"}
              </p>
            </div>
          </div>
        </section>

        <section className="bg-zinc-950 border border-orange-800 rounded-3xl p-6">
          <p className="text-zinc-400 uppercase tracking-widest text-sm">
            Execution Engine
          </p>

          <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mt-4">
            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Status
              </p>
              <p className="text-green-400 font-mono text-xl mt-2">
                {executionStatus}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Fill
              </p>
              <p className="text-orange-400 font-mono text-xl mt-2">
                {execution?.fill ?? "--"}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Qty
              </p>
              <p className="text-green-400 font-mono text-xl mt-2">
                {execution ? execution.qty.toFixed(2) : "--"}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Avg Price
              </p>
              <p className="text-green-400 font-mono text-xl mt-2">
                {execution ? execution.avg_price.toFixed(2) : "--"}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Mark
              </p>
              <p className="text-blue-400 font-mono text-xl mt-2">
                {execution ? execution.mark.toFixed(2) : "--"}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Realized PnL
              </p>
              <p className="text-purple-400 font-mono text-xl mt-2">
                {execution ? execution.realized_pnl.toFixed(4) : "--"}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Unrealized PnL
              </p>
              <p
                className={`font-mono text-xl mt-2 ${
                  execution && execution.unrealized_pnl >= 0
                    ? "text-green-400"
                    : "text-red-400"
                }`}
              >
                {execution ? execution.unrealized_pnl.toFixed(4) : "--"}
              </p>
            </div>

            <div className="bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-500 text-xs uppercase tracking-widest">
                Signal
              </p>
              <p className="text-orange-400 font-mono text-xl mt-2">
                {execution?.signal ?? "--"}
              </p>
            </div>
          </div>
        </section>




<section className="bg-zinc-950 border border-red-800 rounded-3xl p-6">
  <p className="text-zinc-400 uppercase tracking-widest text-sm">
    Risk Engine
  </p>

  <div className="grid grid-cols-1 md:grid-cols-5 gap-4 mt-4">
    {[
      ["Status", riskStatus],
      ["Risk State", risk?.risk_state ?? "--"],
      ["Exposure", risk ? risk.notional_exposure.toFixed(2) : "--"],
      ["Utilization", risk ? `${(risk.exposure_utilization * 100).toFixed(2)}%` : "--"],
      ["VaR 95", risk ? risk.var_95.toFixed(2) : "--"],
      ["Expected Shortfall", risk ? risk.expected_shortfall.toFixed(2) : "--"],
      ["Total PnL", risk ? risk.total_pnl.toFixed(4) : "--"],
      ["Realized PnL", risk ? risk.realized_pnl.toFixed(4) : "--"],
      ["Unrealized PnL", risk ? risk.unrealized_pnl.toFixed(4) : "--"],
      ["Qty", risk ? risk.qty.toFixed(2) : "--"],
    ].map(([label, value]) => (
      <div key={label} className="bg-black border border-zinc-800 rounded-2xl p-4">
        <p className="text-zinc-500 text-xs uppercase tracking-widest">
          {label}
        </p>
        <p className="text-red-400 font-mono text-xl mt-2">{value}</p>
      </div>
    ))}
  </div>
</section>

<section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
  <h2 className="text-2xl font-bold mb-4">Portfolio Equity Curve</h2>

  <div className="h-[280px]">
    <ResponsiveContainer width="100%" height="100%">
      <LineChart data={equityCurve}>
        <CartesianGrid stroke="#27272a" />
        <XAxis dataKey="time" stroke="#a1a1aa" />
        <YAxis stroke="#a1a1aa" />
        <Tooltip />

        <Line
          type="monotone"
          dataKey="equity"
          stroke="#22c55e"
          strokeWidth={2}
          dot={false}
        />

        <Line
          type="monotone"
          dataKey="drawdown"
          stroke="#ef4444"
          strokeWidth={2}
          dot={false}
        />
      </LineChart>
    </ResponsiveContainer>
  </div>
</section>


<section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
  <h2 className="text-2xl font-bold mb-4">
    Backtest Optimization
  </h2>

  {backtest && (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
      <div className="bg-black rounded-2xl p-4 border border-zinc-800">
        <p className="text-zinc-500 text-sm">
          Best Sharpe
        </p>

        <p className="text-emerald-400 text-3xl font-mono mt-2">
          {backtest.sharpe_ratio.toFixed(4)}
        </p>
      </div>

      <div className="bg-black rounded-2xl p-4 border border-zinc-800">
        <p className="text-zinc-500 text-sm">
          Total Return
        </p>

        <p className="text-cyan-400 text-3xl font-mono mt-2">
          {backtest.total_return_pct.toFixed(2)}%
        </p>
      </div>

      <div className="bg-black rounded-2xl p-4 border border-zinc-800">
        <p className="text-zinc-500 text-sm">
          Short MA
        </p>

        <p className="text-orange-400 text-3xl font-mono mt-2">
          {backtest.best_short_ma}
        </p>
      </div>

      <div className="bg-black rounded-2xl p-4 border border-zinc-800">
        <p className="text-zinc-500 text-sm">
          Long MA
        </p>

        <p className="text-pink-400 text-3xl font-mono mt-2">
          {backtest.best_long_ma}
        </p>
      </div>
    </div>
  )}
</section>

<section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
  <h2 className="text-2xl font-bold mb-4">Trade Blotter</h2>

  <div className="overflow-x-auto">
    <table className="w-full text-left text-sm">
      <thead>
        <tr className="border-b border-zinc-800 text-zinc-500 uppercase">
          <th className="p-3">Time</th>
          <th className="p-3">Side</th>
          <th className="p-3">Price</th>
          <th className="p-3">Qty</th>
          <th className="p-3">PnL</th>
        </tr>
      </thead>

      <tbody>
        {tradeLog.map((trade, i) => (
          <tr
            key={i}
            className="border-b border-zinc-900 hover:bg-zinc-900/40"
          >
            <td className="p-3 font-mono text-zinc-400">
              {trade.time}
            </td>

            <td
              className={`p-3 font-mono ${
                trade.side === "BUY"
                  ? "text-green-400"
                  : "text-red-400"
              }`}
            >
              {trade.side}
            </td>

            <td className="p-3 font-mono text-cyan-400">
              {trade.price.toFixed(2)}
            </td>

            <td className="p-3 font-mono text-orange-400">
              {trade.qty.toFixed(2)}
            </td>

            <td
              className={`p-3 font-mono ${
                trade.pnl >= 0
                  ? "text-green-400"
                  : "text-red-400"
              }`}
            >
              {trade.pnl.toFixed(4)}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  </div>
</section>









<section className="grid grid-cols-1 md:grid-cols-3 gap-4">
  <div className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
    <p className="text-zinc-500 uppercase tracking-widest text-sm">
      Sharpe Ratio
    </p>

    <p className="text-cyan-400 font-mono text-4xl mt-4">
      {sharpe.toFixed(4)}
    </p>
  </div>

  <div className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
    <p className="text-zinc-500 uppercase tracking-widest text-sm">
      Rolling Volatility
    </p>

    <p className="text-orange-400 font-mono text-4xl mt-4">
    {rollingVolatility.toFixed(4)}
    </p>
  </div>

  <div className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
    <p className="text-zinc-500 uppercase tracking-widest text-sm">
      Win Rate
    </p>

    <p className="text-green-400 font-mono text-4xl mt-4">
      {winRate.toFixed(2)}%
    </p>
  </div>
</section>

<section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
  <h2 className="text-2xl font-bold mb-6">
    Backtest Equity Curve
  </h2>

  {backtest && (
    <div className="h-[400px]">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={backtest.equity_curve}>
          <XAxis
            dataKey="step"
            stroke="#71717a"
          />

          <YAxis stroke="#71717a" />

          <Tooltip />

          <Line
            type="monotone"
            dataKey="equity"
            stroke="#10b981"
            strokeWidth={2}
            dot={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  )}
</section>

        <section className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <div className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
            <h2 className="text-2xl font-bold mb-4">Live BTC Price Stream</h2>
            <div className="h-[280px]">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={tickChartData}>
                  <CartesianGrid stroke="#27272a" />
                  <XAxis dataKey="tick" stroke="#a1a1aa" />
                  <YAxis stroke="#a1a1aa" domain={["auto", "auto"]} />
                  <Tooltip />
                  <Line
                    type="monotone"
                    dataKey="price"
                    stroke="#22c55e"
                    strokeWidth={2}
                    dot={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>
          </div>

          <div className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
            <h2 className="text-2xl font-bold mb-4">Order Book Ladder</h2>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <p className="text-red-400 font-bold mb-2">Asks</p>
                {asks
                  .slice()
                  .reverse()
                  .map((ask, i) => (
                    <div
                      key={`ask-${i}`}
                      className="flex justify-between border-b border-zinc-800 py-2 text-sm"
                    >
                      <span className="text-red-400 font-mono">
                        {ask.price ? ask.price.toFixed(2) : "--"}
                      </span>
                      <span className="text-zinc-400 font-mono">
                        {ask.size ? ask.size.toFixed(6) : "--"}
                      </span>
                    </div>
                  ))}
              </div>

              <div>
                <p className="text-green-400 font-bold mb-2">Bids</p>
                {bids.map((bid, i) => (
                  <div
                    key={`bid-${i}`}
                    className="flex justify-between border-b border-zinc-800 py-2 text-sm"
                  >
                    <span className="text-green-400 font-mono">
                      {bid.price ? bid.price.toFixed(2) : "--"}
                    </span>
                    <span className="text-zinc-400 font-mono">
                      {bid.size ? bid.size.toFixed(6) : "--"}
                    </span>
                  </div>
                ))}
              </div>
            </div>

            <div className="mt-5 bg-black border border-zinc-800 rounded-2xl p-4">
              <p className="text-zinc-400 text-sm uppercase tracking-widest">
                Synthetic Mid Price
              </p>
              <p className="text-green-400 font-mono text-3xl mt-2">
                {mid ? mid.toFixed(2) : "--"}
              </p>
            </div>

            <div className="mt-6 h-[220px]">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={depthData}>
                  <CartesianGrid stroke="#27272a" />
                  <XAxis dataKey="level" stroke="#a1a1aa" />
                  <YAxis stroke="#a1a1aa" />
                  <Tooltip />
                  <Line
                    type="monotone"
                    dataKey="bidDepth"
                    stroke="#22c55e"
                    strokeWidth={2}
                    dot={false}
                  />
                  <Line
                    type="monotone"
                    dataKey="askDepth"
                    stroke="#ef4444"
                    strokeWidth={2}
                    dot={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>

            <div className="grid grid-cols-2 gap-3 mt-6">
              <div className="bg-black border border-zinc-800 rounded-2xl p-4">
                <p className="text-zinc-500 text-xs uppercase tracking-widest">
                  Spread
                </p>
                <p className="text-green-400 font-mono text-xl mt-2">
                  {spread.toFixed(4)}
                </p>
              </div>

              <div className="bg-black border border-zinc-800 rounded-2xl p-4">
                <p className="text-zinc-500 text-xs uppercase tracking-widest">
                  Imbalance
                </p>
                <p className="text-green-400 font-mono text-xl mt-2">
                  {imbalance.toFixed(4)}
                </p>
              </div>

              <div className="bg-black border border-zinc-800 rounded-2xl p-4">
                <p className="text-zinc-500 text-xs uppercase tracking-widest">
                  Bid Depth
                </p>
                <p className="text-green-400 font-mono text-xl mt-2">
                  {totalBidDepth.toFixed(6)}
                </p>
              </div>

              <div className="bg-black border border-zinc-800 rounded-2xl p-4">
                <p className="text-zinc-500 text-xs uppercase tracking-widest">
                  Ask Depth
                </p>
                <p className="text-red-400 font-mono text-xl mt-2">
                  {totalAskDepth.toFixed(6)}
                </p>
              </div>

              <div className="col-span-2 bg-black border border-zinc-800 rounded-2xl p-4">
                <p className="text-zinc-500 text-xs uppercase tracking-widest">
                  Microprice
                </p>
                <p className="text-purple-400 font-mono text-2xl mt-2">
                  {microprice.toFixed(4)}
                </p>
              </div>
            </div>
          </div>
        </section>

        <section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
  <h2 className="text-2xl font-bold mb-4">PnL Simulator</h2>

  <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
    <div className="bg-black border border-zinc-800 rounded-2xl p-4">
      <p className="text-zinc-500 text-xs uppercase tracking-widest">Position</p>
      <p className="text-green-400 font-mono text-2xl mt-2">{position}</p>
    </div>

    <div className="bg-black border border-zinc-800 rounded-2xl p-4">
      <p className="text-zinc-500 text-xs uppercase tracking-widest">Entry</p>
      <p className="text-green-400 font-mono text-2xl mt-2">
        {entryPrice === null ? "--" : entryPrice.toFixed(2)}
      </p>
    </div>

    <div className="bg-black border border-zinc-800 rounded-2xl p-4">
      <p className="text-zinc-500 text-xs uppercase tracking-widest">Mark</p>
      <p className="text-green-400 font-mono text-2xl mt-2">
        {btc ? btc.price.toFixed(2) : "--"}
      </p>
    </div>

    <div className="bg-black border border-zinc-800 rounded-2xl p-4">
      <p className="text-zinc-500 text-xs uppercase tracking-widest">Unrealized PnL</p>
      <p
        className={`font-mono text-2xl mt-2 ${
          unrealizedPnl >= 0 ? "text-green-400" : "text-red-400"
        }`}
      >
        {unrealizedPnl.toFixed(4)}
      </p>
    </div>
  </div>

  <div className="flex flex-wrap gap-4 mt-6">
    <button
      onClick={() => {
        if (!btc) return;
        setPosition((p) => p + 1);
        setEntryPrice(btc.price);
      }}
      className="bg-green-900 border border-green-500 px-6 py-3 rounded-2xl font-bold"
    >
      Buy +1
    </button>

    <button
      onClick={() => {
        if (!btc) return;
        setPosition((p) => p - 1);
        setEntryPrice(btc.price);
      }}
      className="bg-red-900 border border-red-500 px-6 py-3 rounded-2xl font-bold"
    >
      Sell -1
    </button>

    <button
      onClick={() => {
        setPosition(0);
        setEntryPrice(null);
      }}
      className="bg-zinc-900 border border-zinc-700 px-6 py-3 rounded-2xl font-bold"
    >
      Flatten
    </button>
  </div>
</section>

        <section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
          <h2 className="text-2xl font-bold mb-4">
            1-Second Candle Aggregation
          </h2>

          <div className="h-[260px] mb-6">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={candles}>
                <CartesianGrid stroke="#27272a" />
                <XAxis dataKey="second" stroke="#a1a1aa" />
                <YAxis stroke="#a1a1aa" domain={["auto", "auto"]} />
                <Tooltip />
                <Line
                  type="monotone"
                  dataKey="close"
                  stroke="#a855f7"
                  strokeWidth={2}
                  dot={false}
                />
              </LineChart>
            </ResponsiveContainer>
          </div>

          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm">
              <thead className="text-zinc-400">
                <tr>
                  <th className="p-3">Time</th>
                  <th className="p-3">Open</th>
                  <th className="p-3">High</th>
                  <th className="p-3">Low</th>
                  <th className="p-3">Close</th>
                  <th className="p-3">Volume</th>
                </tr>
              </thead>

              <tbody>
                {candles
                  .slice(-8)
                  .reverse()
                  .map((c) => (
                    <tr key={c.second} className="border-t border-zinc-800">
                      <td className="p-3 text-zinc-400">{c.second}</td>
                      <td className="p-3 font-mono">{c.open.toFixed(2)}</td>
                      <td className="p-3 font-mono text-green-400">
                        {c.high.toFixed(2)}
                      </td>
                      <td className="p-3 font-mono text-red-400">
                        {c.low.toFixed(2)}
                      </td>
                      <td className="p-3 font-mono text-purple-400">
                        {c.close.toFixed(2)}
                      </td>
                      <td className="p-3 font-mono">{c.volume.toFixed(6)}</td>
                    </tr>
                  ))}
              </tbody>
            </table>
          </div>
        </section>

        <section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
  <h2 className="text-2xl font-bold mb-4">Greeks History</h2>

  <div className="h-[280px]">
    <ResponsiveContainer width="100%" height="100%">
      <LineChart data={greeksHistory}>
        <CartesianGrid stroke="#27272a" />
        <XAxis dataKey="time" stroke="#a1a1aa" />
        <YAxis stroke="#a1a1aa" />
        <Tooltip />

        <Line type="monotone" dataKey="delta" stroke="#22c55e" strokeWidth={2} dot={false} />
        <Line type="monotone" dataKey="gamma" stroke="#a855f7" strokeWidth={2} dot={false} />
        <Line type="monotone" dataKey="vega" stroke="#38bdf8" strokeWidth={2} dot={false} />
      </LineChart>
    </ResponsiveContainer>
  </div>
</section>

        <section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
          <h2 className="text-2xl font-bold mb-6">Pricing Inputs</h2>

          <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
            {fields.map(([label, value, setter]) => (
              <div
                key={label}
                className="bg-black border border-zinc-800 p-4 rounded-2xl"
              >
                <label className="block text-zinc-400 mb-2">{label}</label>
                <input
                  type="number"
                  value={value}
                  onChange={(e) => setter(Number(e.target.value))}
                  className="w-full bg-zinc-950 border border-zinc-700 rounded-xl p-3 text-white text-lg"
                />
              </div>
            ))}
          </div>
        </section>

        <section className="flex flex-wrap gap-4">
          <button
            onClick={() =>
              fetchAndMeasure(
                "Black-Scholes Price",
                `http://127.0.0.1:8080/price?${query()}`
              )
            }
            className="bg-white text-black px-7 py-4 rounded-2xl text-xl font-bold"
          >
            Price
          </button>

          <button
            onClick={() =>
              fetchAndMeasure(
                "Greeks",
                `http://127.0.0.1:8080/greeks?${query()}`
              )
            }
            className="bg-zinc-900 border border-zinc-700 px-7 py-4 rounded-2xl text-xl font-bold"
          >
            Greeks
          </button>

          <button
            onClick={() =>
              fetchAndMeasure(
                "American Put",
                `http://127.0.0.1:8080/american-put?${query()}`
              )
            }
            className="bg-zinc-900 border border-zinc-700 px-7 py-4 rounded-2xl text-xl font-bold"
          >
            American Put
          </button>

          <button
            onClick={() =>
              fetchAndMeasure(
                "Parallel Monte Carlo",
                `http://127.0.0.1:8080/monte-carlo?${query()}&simulations=${simulations}`
              )
            }
            className="bg-zinc-900 border border-zinc-700 px-7 py-4 rounded-2xl text-xl font-bold"
          >
            Monte Carlo
          </button>

          <button
            onClick={() =>
              fetchAndMeasure(
                "Implied Volatility",
                `http://127.0.0.1:8080/implied-vol?market_price=${marketPrice}&spot=${spot}&strike=${strike}&rate=${rate}&maturity=${maturity}`
              )
            }
            className="bg-purple-950 border border-purple-500 px-7 py-4 rounded-2xl text-xl font-bold"
          >
            Implied Vol
          </button>

          <button
            onClick={generateSurface}
            className="bg-emerald-900 border border-emerald-500 px-7 py-4 rounded-2xl text-xl font-bold"
          >
            Vol Surface
          </button>
        </section>

        <button
  onClick={generatePricingSurface}
  className="bg-blue-950 border border-blue-500 px-7 py-4 rounded-2xl text-xl font-bold"
>
  Pricing Engine Surface
</button>

        <section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
          <h2 className="text-2xl font-bold mb-5">{title}</h2>

          {result ? (
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              {Object.entries(result).map(([key, value]) => (
                <div
                  key={key}
                  className="bg-black border border-zinc-800 rounded-2xl p-5"
                >
                  <p className="text-zinc-400 uppercase text-sm tracking-widest">
                    {key}
                  </p>
                  <p className="text-green-400 text-2xl font-mono mt-3 break-words">
                    {typeof value === "number" ? value.toFixed(6) : value}
                  </p>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-green-400">Click a model to run pricing.</p>
          )}
        </section>

        <section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
  <h2 className="text-2xl font-bold mb-4">
    Volatility Smile / Skew
  </h2>

  <div className="h-[280px]">
    <ResponsiveContainer width="100%" height="100%">
      <LineChart data={smileData}>
        <CartesianGrid stroke="#27272a" />
        <XAxis dataKey="strike" stroke="#a1a1aa" />
        <YAxis stroke="#a1a1aa" />
        <Tooltip />

        <Line
          type="monotone"
          dataKey="impliedVol"
          stroke="#f97316"
          strokeWidth={2}
          dot={false}
        />
      </LineChart>
    </ResponsiveContainer>
  </div>
</section>

        <section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
          <h2 className="text-2xl font-bold mb-4">
            Volatility Surface Heatmap
          </h2>

          {surfaceData.length === 0 ? (
            <p className="text-zinc-500">
              Click Vol Surface to generate real Rust API-based surface prices.
            </p>
          ) : (
            <div className="overflow-x-auto">
              <table className="border-collapse w-full">
                <thead>
                  <tr>
                    <th className="p-3 border border-zinc-800 bg-zinc-900">
                      Vol \ Strike
                    </th>

                    {[80, 90, 100, 110, 120, 130].map((k) => (
                      <th
                        key={k}
                        className="p-3 border border-zinc-800 bg-zinc-900 text-green-400 font-mono"
                      >
                        {k}
                      </th>
                    ))}
                  </tr>
                </thead>

                <tbody>
                  {[0.1, 0.2, 0.3, 0.4, 0.5, 0.6].map((vol) => (
                    <tr key={vol}>
                      <td className="p-3 border border-zinc-800 bg-zinc-900 text-purple-400 font-mono">
                        σ={vol}
                      </td>

                      {[80, 90, 100, 110, 120, 130].map((k) => {
                        const cell = surfaceData.find(
                          (x) => x.strike === k && x.vol === vol
                        );

                        const value = cell?.price ?? 0;
                        const intensity = Math.min(value / 35, 1);

                        return (
                          <td
                            key={`${k}-${vol}`}
                            className="p-4 border border-zinc-800 text-center font-mono text-lg"
                            style={{
                              backgroundColor: `rgba(34,197,94,${intensity})`,
                              color: intensity > 0.5 ? "black" : "#00ff99",
                            }}
                          >
                            {value.toFixed(2)}
                          </td>
                        );
                      })}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>

        <section className="bg-zinc-950 border border-blue-800 rounded-3xl p-6">
  <h2 className="text-2xl font-bold mb-4">
    Pricing Engine Greeks Surface
  </h2>

  {pricingSurface.length === 0 ? (
    <p className="text-zinc-500">
      Click Pricing Engine Surface to load live pricing-engine Greeks surface.
    </p>
  ) : (
    <div className="overflow-x-auto">
      <table className="w-full text-left text-sm">
        <thead>
          <tr className="border-b border-zinc-800 text-zinc-500 uppercase">
            <th className="p-3">Strike</th>
            <th className="p-3">Vol</th>
            <th className="p-3">Price</th>
            <th className="p-3">Delta</th>
            <th className="p-3">Gamma</th>
            <th className="p-3">Vega</th>
          </tr>
        </thead>

        <tbody>
          {pricingSurface.map((point, i) => (
            <tr key={i} className="border-b border-zinc-900">
              <td className="p-3 font-mono text-cyan-400">
                {point.strike.toFixed(2)}
              </td>

              <td className="p-3 font-mono text-blue-400">
                {point.volatility.toFixed(2)}
              </td>

              <td className="p-3 font-mono text-green-400">
                {point.price.toFixed(4)}
              </td>

              <td className="p-3 font-mono text-purple-400">
                {point.delta.toFixed(4)}
              </td>

              <td className="p-3 font-mono text-orange-400">
                {point.gamma.toFixed(6)}
              </td>

              <td className="p-3 font-mono text-pink-400">
                {point.vega.toFixed(4)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )}
</section>

      </div>
    </main>
  );
}