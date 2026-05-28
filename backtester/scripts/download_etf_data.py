import csv
import os
import time
import urllib.request

SYMBOLS = {
    "SPX": "^spx",
    "NDX": "^ndx",
    "DJI": "^dji",
    "RUT": "^rut",
    "GOLD": "xauusd",
    "SILVER": "xagusd",
    "CRUDE": "cl.f",
    "DOLLAR": "dx.f",
    "EURUSD": "eurusd",
    "JPYUSD": "jpyusd",
}

START = "2015-01-01"

os.makedirs("data", exist_ok=True)

for symbol, stooq_symbol in SYMBOLS.items():
    url = f"https://stooq.com/q/d/l/?s={stooq_symbol}&i=d"
    raw_path = f"data/{symbol.lower()}_raw.csv"
    out_path = f"data/{symbol.lower()}.csv"

    print(f"Downloading {symbol} from Stooq: {url}")

    req = urllib.request.Request(
        url,
        headers={"User-Agent": "Mozilla/5.0"},
    )

    try:
        with urllib.request.urlopen(req, timeout=20) as response:
            content = response.read().decode("utf-8")
    except Exception as e:
        print(f"Failed {symbol}: {e}")
        continue

    with open(raw_path, "w") as f:
        f.write(content)

    rows = []
    with open(raw_path, newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            date = row.get("Date")
            close = row.get("Close")

            if not date or not close:
                continue

            if date < START:
                continue

            try:
                rows.append((len(rows), float(close)))
            except ValueError:
                continue

    with open(out_path, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["time", "price"])
        writer.writerows(rows)

    print(f"Saved {len(rows)} rows to {out_path}")
    time.sleep(1.0)
