/**
 * k0mmand3r IPC Listener - Processes slash commands from Redis pub/sub
 */

import type { Redis } from 'ioredis';
import { Pm2TaskerService } from './pm2-service.js';
import { K0mmand3rMessageSchema, type K0mmand3rMessage, type Logger, type TaskDatum } from './types.js';

export class K0mmand3rListener {
  private redisSub: Redis;
  private pm2Service: Pm2TaskerService;
  private channel: string;
  private log: Logger;
  private isRunning: boolean = false;

  constructor(
    redisSub: Redis,
    pm2Service: Pm2TaskerService,
    channel: string,
    log: Logger
  ) {
    this.redisSub = redisSub;
    this.pm2Service = pm2Service;
    this.channel = channel;
    this.log = log;
  }

  async start(): Promise<void> {
    this.log.info(`🎧 Subscribing to k0mmand3r channel: ${this.channel}`);

    try {
      await this.redisSub.subscribe(this.channel);
      this.isRunning = true;

      this.redisSub.on('message', async (channel, message) => {
        if (channel === this.channel) {
          await this.handleMessage(message);
        }
      });

      this.log.info(`✅ K0mmand3r listener started on channel: ${this.channel}`);
    } catch (error) {
      this.log.error('❌ Failed to start k0mmand3r listener:', error);
      throw error;
    }
  }

  async stop(): Promise<void> {
    if (this.isRunning) {
      await this.redisSub.unsubscribe(this.channel);
      this.isRunning = false;
      this.log.info('K0mmand3r listener stopped');
    }
  }

  /**
   * Handle incoming k0mmand3r message
   */
  private async handleMessage(rawMessage: string): Promise<void> {
    try {
      const parsed = JSON.parse(rawMessage);
      const result = K0mmand3rMessageSchema.safeParse(parsed);

      if (!result.success) {
        this.log.warn('Invalid k0mmand3r message:', result.error);
        return;
      }

      const msg: K0mmand3rMessage = result.data;
      this.log.debug('Received k0mmand3r message:', msg);

      // Route based on verb
      await this.routeCommand(msg);
    } catch (error) {
      this.log.error('Error handling k0mmand3r message:', error);
    }
  }

  /**
   * Route slash command to appropriate handler
   */
  private async routeCommand(msg: K0mmand3rMessage): Promise<void> {
    const verb = msg.verb?.toLowerCase();
    const params = msg.params || {};

    switch (verb) {
      case 'start':
      case 'run':
        await this.handleStart(params, msg);
        break;

      case 'stop':
        await this.handleStop(params, msg);
        break;

      case 'restart':
        await this.handleRestart(params, msg);
        break;

      case 'delete':
      case 'remove':
        await this.handleDelete(params, msg);
        break;

      case 'status':
      case 'list':
      case 'ps':
        await this.handleStatus(params, msg);
        break;

      default:
        this.log.warn(`Unknown verb: ${verb}`);
    }
  }

  /**
   * Handle /start command
   * Example: /start --task=mytask --datum=mydatum.cli
   */
  private async handleStart(params: Record<string, string>, msg: K0mmand3rMessage): Promise<void> {
    const taskName = params.task || params.name;
    const datumName = params.datum;

    if (!taskName) {
      this.log.warn('Missing task name in /start command');
      return;
    }

    this.log.info(`Processing /start command for task: ${taskName}`);

    // Build TaskDatum from params
    const datum: TaskDatum = {
      name: taskName,
      datum_type: datumName || 'cli',
      command: params.command || params.cmd,
      args: params.args ? params.args.split(' ') : [],
      script: params.script,
      env: this.parseEnvParams(params),
      cwd: params.cwd,
      instances: (() => {
        if (params.instances) {
          const parsed = parseInt(params.instances, 10);
          if (!Number.isNaN(parsed) && parsed > 0) {
            return parsed;
          } else {
            this.log.warn(`Invalid instances value "${params.instances}" in /start command; defaulting to 1`);
            return 1;
          }
        }
        return 1;
      })(),
      max_memory_restart: params.max_memory_restart || params.memory,
    };

    const result = await this.pm2Service.startTask(datum);
    this.log.info(`/start result:`, result.message);
  }

  /**
   * Handle /stop command
   * Example: /stop --task=mytask
   */
  private async handleStop(params: Record<string, string>, msg: K0mmand3rMessage): Promise<void> {
    const taskName = params.task || params.name;

    if (!taskName) {
      this.log.warn('Missing task name in /stop command');
      return;
    }

    this.log.info(`Processing /stop command for task: ${taskName}`);
    const result = await this.pm2Service.stopTask(taskName);
    this.log.info(`/stop result:`, result.message);
  }

  /**
   * Handle /restart command
   * Example: /restart --task=mytask
   */
  private async handleRestart(params: Record<string, string>, msg: K0mmand3rMessage): Promise<void> {
    const taskName = params.task || params.name;

    if (!taskName) {
      this.log.warn('Missing task name in /restart command');
      return;
    }

    this.log.info(`Processing /restart command for task: ${taskName}`);
    const result = await this.pm2Service.restartTask(taskName);
    this.log.info(`/restart result:`, result.message);
  }

  /**
   * Handle /delete command
   * Example: /delete --task=mytask
   */
  private async handleDelete(params: Record<string, string>, msg: K0mmand3rMessage): Promise<void> {
    const taskName = params.task || params.name;

    if (!taskName) {
      this.log.warn('Missing task name in /delete command');
      return;
    }

    this.log.info(`Processing /delete command for task: ${taskName}`);
    const result = await this.pm2Service.deleteTask(taskName);
    this.log.info(`/delete result:`, result.message);
  }

  /**
   * Handle /status command
   * Example: /status
   */
  private async handleStatus(params: Record<string, string>, msg: K0mmand3rMessage): Promise<void> {
    this.log.info(`Processing /status command`);
    const result = await this.pm2Service.listTasks();
    this.log.info(`/status result:`, result.message);

    if (result.status) {
      this.log.debug('Task list:', result.status);
    }
  }

  /**
   * Parse environment variables from params
   * Example: params.env_FOO=bar, params.env_BAZ=qux => { FOO: 'bar', BAZ: 'qux' }
   */
  private parseEnvParams(params: Record<string, string>): Record<string, string> {
    const env: Record<string, string> = {};

    for (const [key, value] of Object.entries(params)) {
      if (key.startsWith('env_')) {
        const envKey = key.substring(4); // Remove 'env_' prefix
        env[envKey] = value;
      }
    }

    return env;
  }
}
