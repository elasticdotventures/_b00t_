#!/usr/bin/env node
/**
 * b00t PM2 Tasker - Process manager with k0mmand3r IPC integration
 *
 * Listens to Redis pub/sub channels for slash commands and executes
 * task datums via PM2 process management.
 */

import { Redis } from 'ioredis';
import { z } from 'zod';
import { Pm2TaskerService } from './pm2-service.js';
import { K0mmand3rListener } from './k0mmand3r-listener.js';

// Environment configuration schema
const EnvSchema = z.object({
  REDIS_URL: z.string().default('redis://localhost:6379'),
  PM2_TASKER_CHANNEL: z.string().default('b00t:k0mmand3r'),
  LOG_LEVEL: z.enum(['debug', 'info', 'warn', 'error']).default('info'),
  NODE_ENV: z.enum(['development', 'production', 'test']).default('development'),
});

type Env = z.infer<typeof EnvSchema>;

// Parse and validate environment
const env: Env = EnvSchema.parse({
  REDIS_URL: process.env.REDIS_URL,
  PM2_TASKER_CHANNEL: process.env.PM2_TASKER_CHANNEL,
  LOG_LEVEL: process.env.LOG_LEVEL,
  NODE_ENV: process.env.NODE_ENV,
});

// Simple logger
const log = {
  debug: (...args: unknown[]) => env.LOG_LEVEL === 'debug' && console.log('[DEBUG]', ...args),
  info: (...args: unknown[]) => ['debug', 'info'].includes(env.LOG_LEVEL) && console.log('[INFO]', ...args),
  warn: (...args: unknown[]) => console.warn('[WARN]', ...args),
  error: (...args: unknown[]) => console.error('[ERROR]', ...args),
};

async function main() {
  log.info('🥾 b00t PM2 Tasker starting...');
  log.info(`Environment: ${env.NODE_ENV}`);
  log.info(`Redis URL: ${env.REDIS_URL}`);
  log.info(`K0mmand3r channel: ${env.PM2_TASKER_CHANNEL}`);

  // Initialize Redis clients (separate for pub and sub)
  const redisClient = new Redis(env.REDIS_URL, {
    retryStrategy: (times) => {
      const delay = Math.min(times * 50, 2000);
      log.warn(`Redis connection failed, retrying in ${delay}ms...`);
      return delay;
    },
  });

  const redisSub = new Redis(env.REDIS_URL);

  // Handle Redis connection events
  redisClient.on('connect', () => log.info('✅ Redis client connected'));
  redisClient.on('error', (err) => log.error('❌ Redis client error:', err));
  redisSub.on('connect', () => log.info('✅ Redis subscriber connected'));
  redisSub.on('error', (err) => log.error('❌ Redis subscriber error:', err));

  // Initialize PM2 Tasker Service
  const pm2Service = new Pm2TaskerService(redisClient, log);
  await pm2Service.initialize();

  // Initialize k0mmand3r listener
  const k0mmand3rListener = new K0mmand3rListener(
    redisSub,
    pm2Service,
    env.PM2_TASKER_CHANNEL,
    log
  );

  await k0mmand3rListener.start();

  log.info('✅ b00t PM2 Tasker ready');
  log.info(`Listening for commands on channel: ${env.PM2_TASKER_CHANNEL}`);

  // Graceful shutdown
  async function gracefulShutdown(signal: string) {
    log.info(`⚠️  ${signal} received, shutting down gracefully...`);
    await k0mmand3rListener.stop();
    await pm2Service.shutdown();
    await redisClient.quit();
    await redisSub.quit();
    process.exit(0);
  }

  process.on('SIGINT', () => {
    gracefulShutdown('SIGINT');
  });

  process.on('SIGTERM', () => {
    gracefulShutdown('SIGTERM');
  });
}

// Run
main().catch((error) => {
  console.error('❌ Fatal error:', error);
  process.exit(1);
});
