import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  experimental: {
    // Emit per-route build manifests so the bundle budget checker can read first-load JS sizes.
    optimizePackageImports: ["lucide-react"],
  },
  webpack(config, { isServer }) {
    if (!isServer) {
      // Keep heavy optional features out of shared chunks by giving them explicit names.
      config.optimization = config.optimization ?? {};
      config.optimization.splitChunks = {
        ...(config.optimization.splitChunks || {}),
        cacheGroups: {
          ...((config.optimization.splitChunks &&
            typeof config.optimization.splitChunks === "object" &&
            "cacheGroups" in config.optimization.splitChunks
            ? config.optimization.splitChunks.cacheGroups
            : {}) as Record<string, unknown>),
          heavyVendors: {
            test: /[\\/]node_modules[\\/](three|@react-three|recharts|d3-|jspdf|html2canvas)/,
            name: "heavy-vendors",
            chunks: "async",
            priority: 30,
            reuseExistingChunk: true,
          },
        },
      };
    }
    return config;
  },
};

export default nextConfig;
