/**
 * Type definitions for b00t PM2 Tasker
 */

import { z } from 'zod';

// k0mmand3r message schema - slash command from IPC channel
export const K0mmand3rMessageSchema = z.object({
  verb: z.string().optional(), // /start, /stop, /restart, /status
  params: z.record(z.string()).optional(), // --task=mytask, --datum=mydatum
  content: z.string().optional(), // Additional content after params
  timestamp: z.string().datetime().optional(),
  agent_id: z.string().optional(),
});

export type K0mmand3rMessage = z.infer<typeof K0mmand3rMessageSchema>;

// Task datum configuration
export const TaskDatumSchema = z.object({
  name: z.string(),
  datum_type: z.string(),
  command: z.string().optional(),
  args: z.array(z.string()).optional(),
  script: z.string().optional(),
  env: z.record(z.string()).optional(),
  cwd: z.string().optional(),
  instances: z.number().default(1),
  max_memory_restart: z.string().optional(),
});

export type TaskDatum = z.infer<typeof TaskDatumSchema>;

// PM2 process info
export interface Pm2ProcessInfo {
  name: string;
  pm_id: number;
  status: 'online' | 'stopping' | 'stopped' | 'launching' | 'errored' | 'one-launch-status';
  pid: number;
  pm2_env: {
    status: string;
    restart_time: number;
    unstable_restarts: number;
    created_at: number;
    pm_uptime: number;
  };
  monit: {
    memory: number;
    cpu: number;
  };
}

// Task execution result
export interface TaskExecutionResult {
  success: boolean;
  pm_id?: number;
  name?: string;
  status?: string;
  error?: string;
  message: string;
}

// Logger interface
export interface Logger {
  debug: (...args: unknown[]) => void;
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
  error: (...args: unknown[]) => void;
}
