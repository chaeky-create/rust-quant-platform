"use client";

import { useState } from "react";
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

export default function Home() {
  const [spot, setSpot] = useState(100);
  const [strike, setStrike] = useState(110);
  const [rate, setRate] = useState(0.05);
  const [volatility, setVolatility] = useState(0.2);
  const [maturity, setMaturity] = useState(1);
  const [simulations, setSimulations] = useState(1000000);
  const [marketPrice, setMarketPrice] = useState(6.04);

  const [title, setTitle] = useState("No result yet");
  const [result, setResult] = useState<Result | null>(null);
  const [rawJson, setRawJson] = useState("Click a model.");
  const [latency, setLatency] = useState<number | null>(null);
  const [chartData, setChartData] = useState<any[]>([]);

  function query() {
    return `spot=${spot}&strike=${strike}&rate=${rate}&volatility=${volatility}&maturity=${maturity}`;
  }

  async function fetchAndMeasure(label: string, endpoint: string) {
    const start = performance.now();
    const res = await fetch(endpoint);
    const data = await res.json();
    const end = performance.now();

    setTitle(label);
    setResult(data);
    setRawJson(JSON.stringify(data, null, 2));
    setLatency(end - start);
  }

  async function generateVolChart() {
    const data = [];

    for (let vol = 0.1; vol <= 1.0; vol += 0.1) {
      const res = await fetch(
        `http://127.0.0.1:8080/price?spot=${spot}&strike=${strike}&rate=${rate}&volatility=${vol}&maturity=${maturity}`
      );

      const json = await res.json();

      data.push({
        volatility: vol.toFixed(1),
        price: Number(json.price.toFixed(4)),
      });
    }

    setChartData(data);
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

  return (
    <main className="min-h-screen bg-black text-white px-8 py-10">
      <div className="max-w-7xl mx-auto space-y-8">
        <header>
          <h1 className="text-6xl font-bold tracking-tight">
            Rust Quant Dashboard
          </h1>
          <p className="text-zinc-400 mt-3 text-lg">
            High-performance derivatives pricing powered by a Rust Axum backend.
          </p>
        </header>

        <section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
          <h2 className="text-2xl font-bold mb-6">Input Parameters</h2>

          <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
            {fields.map(([label, value, setter]) => (
              <div key={label} className="bg-black border border-zinc-800 p-4 rounded-2xl">
                <label className="block text-zinc-400 mb-2">{label}</label>
                <input
                  type="number"
                  value={value}
                  onChange={(e) =>
                    setter(Number(e.target.value))
                  }
                  className="w-full bg-zinc-950 border border-zinc-700 rounded-xl p-3 text-white text-lg"
                />
              </div>
            ))}
          </div>
        </section>

        <section className="flex flex-wrap gap-4">
          <button
            onClick={() => {
              fetchAndMeasure(
                "Black-Scholes Price",
                `http://127.0.0.1:8080/price?${query()}`
              );
              generateVolChart();
            }}
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
        </section>

        <section className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          <div className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
            <p className="text-zinc-400 uppercase tracking-widest text-sm">
              Frontend Request Latency
            </p>
            <p className="text-green-400 text-4xl font-mono mt-4">
              {latency === null ? "--" : `${latency.toFixed(3)} ms`}
            </p>
          </div>

          <div className="lg:col-span-2 bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
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
          </div>
        </section>

        <section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
          <h2 className="text-2xl font-bold mb-4">Raw JSON Response</h2>
          <pre className="bg-black border border-zinc-800 rounded-2xl p-5 text-green-400 overflow-x-auto">
            {rawJson}
          </pre>
        </section>

        <section className="bg-zinc-950 border border-zinc-800 rounded-3xl p-6">
          <h2 className="text-2xl font-bold mb-2">
            Option Price vs Volatility
          </h2>
          <p className="text-zinc-400 mb-6">
            Black-Scholes price across different volatility levels.
          </p>

          <div className="h-[360px]">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={chartData}>
                <CartesianGrid stroke="#27272a" />
                <XAxis dataKey="volatility" stroke="#a1a1aa" />
                <YAxis stroke="#a1a1aa" />
                <Tooltip />
                <Line
                  type="monotone"
                  dataKey="price"
                  stroke="#22c55e"
                  strokeWidth={3}
                  dot={{ r: 4 }}
                />
              </LineChart>
            </ResponsiveContainer>
          </div>
        </section>
      </div>
    </main>
  );
}