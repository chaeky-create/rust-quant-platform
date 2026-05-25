import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  async rewrites() {
    return [
      {
        source: "/rust/:path*",
        destination: "http://127.0.0.1:8080/:path*",
      },
    ];
  },
};

export default nextConfig;