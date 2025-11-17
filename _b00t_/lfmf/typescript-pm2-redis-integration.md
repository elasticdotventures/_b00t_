# TypeScript PM2 + Redis IPC Integration

**Topic**: typescript, pm2, redis, ipc, process-management

**Lesson Learned**: Integrating PM2 process manager with Redis pub/sub for k0mmand3r IPC

## Problem

- Need process management for task datums via slash commands
- Require IPC between k0mmand3r parser and task executor
- PM2 configuration and TypeScript/Node.js integration patterns unclear

## Solution

### 1. Use ioredis for Redis Pub/Sub

```typescript
import { Redis } from 'ioredis';

// Separate clients for pub and sub (prevents blocking)
const redisClient = new Redis('redis://localhost:6379');
const redisSub = new Redis('redis://localhost:6379');

// Subscribe to k0mmand3r channel
await redisSub.subscribe('b00t:k0mmand3r');

redisSub.on('message', (channel, message) => {
  const cmd = JSON.parse(message);
  // Handle slash command
});
```

**Why ioredis**: Better TypeScript support, reconnection handling, promise-based API compared to node-redis.

### 2. PM2 Ecosystem Config Requires CommonJS

```javascript
// ecosystem.config.cjs (NOT .js - must be .cjs for ESM projects)
module.exports = {
  apps: [{
    name: 'b00t-tasker',
    script: './dist/index.js',
    instances: 1,
    exec_mode: 'fork',
    env: {
      REDIS_URL: 'redis://localhost:6379',
      PM2_TASKER_CHANNEL: 'b00t:k0mmand3r'
    }
  }]
};
```

**Why .cjs**: PM2 loads config synchronously using `require()`. ESM projects need explicit `.cjs` extension.

### 3. Type-Safe Message Validation with Zod

```typescript
import { z } from 'zod';

const K0mmand3rMessageSchema = z.object({
  verb: z.string().optional(),
  params: z.record(z.string()).optional(),
  content: z.string().optional(),
  timestamp: z.string().datetime().optional(),
});

// Runtime validation prevents malformed messages
const result = K0mmand3rMessageSchema.safeParse(JSON.parse(message));
if (!result.success) {
  log.warn('Invalid message:', result.error);
  return;
}
```

### 4. Environment Variable Params Pattern

Clean separation between command params and environment variables:

```typescript
// Slash command: /start --task=api --env_PORT=8080 --env_NODE_ENV=production

// Parse env params with prefix
function parseEnvParams(params: Record<string, string>) {
  const env: Record<string, string> = {};
  for (const [key, value] of Object.entries(params)) {
    if (key.startsWith('env_')) {
      env[key.substring(4)] = value; // Remove 'env_' prefix
    }
  }
  return env; // { PORT: '8080', NODE_ENV: 'production' }
}
```

### 5. Promisify PM2 API for async/await

```typescript
import pm2 from 'pm2';
import { promisify } from 'util';

const pm2Connect = promisify(pm2.connect.bind(pm2));
const pm2Start = promisify(pm2.start.bind(pm2));
const pm2List = promisify(pm2.list.bind(pm2));

// Now use with async/await
await pm2Connect();
const processes = await pm2List();
```

## Architecture

```
Redis (b00t:k0mmand3r channel)
    ↓
k0mmand3r Listener (TypeScript)
    ↓
PM2 Service (process lifecycle)
    ↓
PM2 Manager (Node.js processes)
```

## Benefits

1. **Type Safety**: Zod validates messages at runtime
2. **Reliability**: ioredis handles reconnection, retries
3. **Process Management**: PM2 handles clustering, monitoring, auto-restart
4. **Clean Architecture**: Separate concerns (IPC, parsing, execution)

## Files

- `pm2-tasker/src/index.ts` - Main entry point
- `pm2-tasker/src/pm2-service.ts` - PM2 lifecycle management
- `pm2-tasker/src/k0mmand3r-listener.ts` - Redis IPC listener
- `pm2-tasker/ecosystem.config.cjs` - PM2 configuration

## Related

- LFMF: k0mmand3r IPC integration pattern
- LFMF: rust PM2 integration testing
- Documentation: `pm2-tasker/INTEGRATION.md`

## Date

2025-11-17

## Category

typescript, pm2, redis, ipc
