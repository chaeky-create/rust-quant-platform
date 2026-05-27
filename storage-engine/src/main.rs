use futures_util::StreamExt;
use serde_json::Value;
use tokio_postgres::NoTls;

#[tokio::main]
async fn main() {
    println!("Starting storage-engine...");

    let (pg_client, connection) = tokio_postgres::connect(
        "host=127.0.0.1 user=quant password=quantpass dbname=quantdb port=5432",
        NoTls,
    )
    .await
    .expect("Failed to connect to PostgreSQL");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("PostgreSQL connection error: {}", e);
        }
    });

    println!("Connected to PostgreSQL.");

    let redis_client = redis::Client::open("redis://127.0.0.1:6379/")
        .expect("Failed to create Redis client");

    let mut pubsub = redis_client
        .get_async_pubsub()
        .await
        .expect("Failed to connect to Redis pubsub");

    pubsub
        .subscribe("market-data:ticks")
        .await
        .expect("Failed to subscribe market-data:ticks");

    pubsub
        .subscribe("strategy:signals")
        .await
        .expect("Failed to subscribe strategy:signals");

    pubsub
        .subscribe("execution:fills")
        .await
        .expect("Failed to subscribe execution:fills");

    pubsub
        .subscribe("risk:snapshots")
        .await
        .expect("Failed to subscribe risk:snapshots");

    println!("Subscribed to Redis channels.");

    let mut stream = pubsub.on_message();

    while let Some(message) = stream.next().await {
        let channel = message.get_channel_name().to_string();

        let Ok(payload): Result<String, _> = message.get_payload() else {
            continue;
        };

        let Ok(json) = serde_json::from_str::<Value>(&payload) else {
            continue;
        };

        match channel.as_str() {
            "market-data:ticks" => {
                let data = &json["data"];

                let symbol = data["symbol"].as_str().unwrap_or("BTC-USD");
                let price = data["price"].as_f64().unwrap_or(0.0);
                let size = data["size"].as_f64().unwrap_or(0.0);
                let timestamp = data["timestamp"].as_str().unwrap_or("");

                let _ = pg_client
                    .execute(
                        "INSERT INTO market_ticks (symbol, price, size, timestamp)
                         VALUES ($1, $2, $3, $4)",
                        &[&symbol, &price, &size, &timestamp],
                    )
                    .await;
            }

            "strategy:signals" => {
                let signal = json["signal"].as_str().unwrap_or("FLAT");
                let price = json["price"].as_f64().unwrap_or(0.0);
                let short_ma = json["short_ma"].as_f64().unwrap_or(0.0);
                let long_ma = json["long_ma"].as_f64().unwrap_or(0.0);
                let timestamp = json["timestamp"].as_str().unwrap_or("");

                let _ = pg_client
                    .execute(
                        "INSERT INTO strategy_signals (signal, price, short_ma, long_ma, timestamp)
                         VALUES ($1, $2, $3, $4, $5)",
                        &[&signal, &price, &short_ma, &long_ma, &timestamp],
                    )
                    .await;
            }

            "execution:fills" => {
                let signal = json["signal"].as_str().unwrap_or("FLAT");
                let fill = json["fill"].as_str().unwrap_or("NONE");
                let qty = json["qty"].as_f64().unwrap_or(0.0);
                let avg_price = json["avg_price"].as_f64().unwrap_or(0.0);
                let mark = json["mark"].as_f64().unwrap_or(0.0);
                let realized_pnl = json["realized_pnl"].as_f64().unwrap_or(0.0);
                let unrealized_pnl = json["unrealized_pnl"].as_f64().unwrap_or(0.0);
                let timestamp = json["timestamp"].as_str().unwrap_or("");

                let _ = pg_client
                    .execute(
                        "INSERT INTO execution_fills
                         (signal, fill, qty, avg_price, mark, realized_pnl, unrealized_pnl, timestamp)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                        &[
                            &signal,
                            &fill,
                            &qty,
                            &avg_price,
                            &mark,
                            &realized_pnl,
                            &unrealized_pnl,
                            &timestamp,
                        ],
                    )
                    .await;
            }

            "risk:snapshots" => {
                let risk_state = json["risk_state"].as_str().unwrap_or("OK");
                let qty = json["qty"].as_f64().unwrap_or(0.0);
                let mark = json["mark"].as_f64().unwrap_or(0.0);
                let notional_exposure = json["notional_exposure"].as_f64().unwrap_or(0.0);
                let exposure_utilization = json["exposure_utilization"].as_f64().unwrap_or(0.0);
                let realized_pnl = json["realized_pnl"].as_f64().unwrap_or(0.0);
                let unrealized_pnl = json["unrealized_pnl"].as_f64().unwrap_or(0.0);
                let total_pnl = json["total_pnl"].as_f64().unwrap_or(0.0);
                let var_95 = json["var_95"].as_f64().unwrap_or(0.0);
                let expected_shortfall = json["expected_shortfall"].as_f64().unwrap_or(0.0);

                let _ = pg_client
                    .execute(
                        "INSERT INTO risk_snapshots
                         (risk_state, qty, mark, notional_exposure, exposure_utilization,
                          realized_pnl, unrealized_pnl, total_pnl, var_95, expected_shortfall)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                        &[
                            &risk_state,
                            &qty,
                            &mark,
                            &notional_exposure,
                            &exposure_utilization,
                            &realized_pnl,
                            &unrealized_pnl,
                            &total_pnl,
                            &var_95,
                            &expected_shortfall,
                        ],
                    )
                    .await;
            }

            _ => {}
        }

        println!("Stored event from channel: {}", channel);
    }
}