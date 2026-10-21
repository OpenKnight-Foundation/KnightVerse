# KnightVerse frontend (Next.js 15) — production image for docker compose.
#
# Uses the Next.js standalone build output (next.config.ts sets
# `output: 'standalone'`) which produces a self-contained `server.js` with a
# trimmed dependency tree, so the runtime image needs no full node_modules.
#
# `NEXT_PUBLIC_*` values are inlined into the client bundle at BUILD time, so
# the API URL is passed as a build arg. For the local compose stack the browser
# must reach the backend on its host-published port (8080), hence the default.

# ---- deps: install node_modules once and cache the layer ----
FROM node:20-alpine AS deps
WORKDIR /app
ENV NEXT_TELEMETRY_DISABLED=1
COPY package.json package-lock.json ./
RUN npm ci

# ---- builder: compile the application ----
FROM node:20-alpine AS builder
WORKDIR /app
ENV NEXT_TELEMETRY_DISABLED=1
COPY --from=deps /app/node_modules ./node_modules
COPY . .
# NEXT_PUBLIC_* are baked into the client bundle at build time.
ARG NEXT_PUBLIC_API_URL=http://localhost:8080
ARG NEXT_PUBLIC_BACKEND_URL=http://localhost:8080
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL
ENV NEXT_PUBLIC_BACKEND_URL=$NEXT_PUBLIC_BACKEND_URL
RUN npm run build

# ---- runner: minimal production image ----
FROM node:20-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
# Next.js standalone server reads PORT + HOSTNAME to bind its listener.
ENV PORT=3000
ENV HOSTNAME=0.0.0.0

RUN addgroup -g 1001 -S nodejs && adduser -S nextjs -u 1001 -G nodejs

# Standalone server (includes a trimmed node_modules).
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
# Static assets and public files are NOT bundled into standalone; copy them.
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static
COPY --from=builder --chown=nextjs:nodejs /app/public ./public

USER nextjs
EXPOSE 3000
CMD ["node", "server.js"]
