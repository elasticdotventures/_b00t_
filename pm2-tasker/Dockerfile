# PM2 Tasker Dockerfile - Node.js TypeScript service with PM2
FROM node:20-alpine AS builder

WORKDIR /app

# Install dependencies
COPY package.json package-lock.json* ./
RUN npm ci

# Copy source
COPY tsconfig.json ./
COPY src ./src

# Build TypeScript
RUN npm run build

# Production stage
FROM node:20-alpine

# Install PM2 globally
RUN npm install -g pm2

WORKDIR /app

# Copy built artifacts
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/node_modules ./node_modules
COPY package.json ecosystem.config.cjs ./

# Health check

# Expose PM2 metrics port (optional)
EXPOSE 9615

# Environment defaults
ENV NODE_ENV=production \
    REDIS_URL=redis://redis:6379 \
    PM2_TASKER_CHANNEL=b00t:k0mmand3r \
    LOG_LEVEL=info

# Start with PM2
CMD ["pm2-runtime", "start", "ecosystem.config.cjs"]
