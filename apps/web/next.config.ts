import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  async rewrites() {
    const target = process.env.TRPG_API_PROXY_TARGET;
    if (!target) {
      return [];
    }

    return [
      {
        source: "/api/:path*",
        destination: `${target.replace(/\/$/, "")}/api/:path*`
      },
      {
        source: "/openapi.json",
        destination: `${target.replace(/\/$/, "")}/openapi.json`
      }
    ];
  }
};

export default nextConfig;
