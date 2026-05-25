export async function GET() {
    try {
      const res = await fetch(
        "http://localhost:8080/price?spot=100&strike=110&rate=0.05&volatility=0.2&maturity=1",
        { cache: "no-store" }
      );
  
      const data = await res.json();
  
      return Response.json(data);
    } catch (error) {
      return Response.json(
        { error: "Failed to connect to Rust backend" },
        { status: 500 }
      );
    }
  }