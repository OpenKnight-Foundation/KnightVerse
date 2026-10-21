import type { NextConfig } from "next";
import { withSentryConfig } from "@sentry/nextjs";
import { buildCsp } from "./lib/csp";

// connect-src is derived from the deployment's NEXT_PUBLIC_* config (#FE-87).
const CSP_DIRECTIVES = buildCsp(process.env);

const securityHeaders = [
  {
    key: "Content-Security-Policy",
    value: CSP_DIRECTIVES,
  },
  {
    key: "X-Frame-Options",
    value: "DENY",
  },
  {
    key: "X-Content-Type-Options",
    value: "nosniff",
  },
  {
    key: "Referrer-Policy",
    value: "strict-origin-when-cross-origin",
  },
  {
    key: "Permissions-Policy",
    // Voice move input (VoiceMoveInput.tsx) needs the microphone on our own origin (#FE-89).
    value: "camera=(), microphone=(self), geolocation=()",
  },
];

const nextConfig: NextConfig = {
  reactStrictMode: true,
  // Emit a self-contained `.next/standalone` server so the Docker image
  // (frontend/Dockerfile) can run `node server.js` without a full node_modules.
  output: "standalone",
  images: {
    unoptimized: true,
  },
  async headers() {
    return [
      {
        source: "/(.*)",
        headers: securityHeaders,
      },
    ];
  },
};

export default withSentryConfig(nextConfig, {
  org: process.env.SENTRY_ORG,
  project: process.env.SENTRY_PROJECT,
  silent: !process.env.CI,
  widenClientFileUpload: true,
});

