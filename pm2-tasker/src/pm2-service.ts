/**
 * PM2 Service - Manages task processes via PM2
 */

import pm2 from 'pm2';
import { promisify } from 'util';
import type { Redis } from 'ioredis';
import type {
  TaskDatum,
  Pm2ProcessInfo,
  TaskExecutionResult,
  Logger,
} from './types.js';

export class Pm2TaskerService {
  private redis: Redis;
  private log: Logger;
  private initialized: boolean = false;
  private pm2Connect: () => Promise<void>;
  private pm2Disconnect: () => Promise<void>;
  private pm2List: () => Promise<Pm2ProcessInfo[]>;
  private pm2Start: (options: pm2.StartOptions) => Promise<pm2.Proc>;
  private pm2Stop: (process: string | number) => Promise<pm2.Proc>;
  private pm2Restart: (process: string | number) => Promise<pm2.Proc>;
  private pm2Delete: (process: string | number) => Promise<pm2.Proc>;
  private pm2Describe: (process: string) => Promise<pm2.ProcessDescription[]>;

  constructor(redis: Redis, log: Logger) {
    this.redis = redis;
    this.log = log;

    // Promisify PM2 methods
    this.pm2Connect = promisify(pm2.connect.bind(pm2));
    this.pm2Disconnect = promisify(pm2.disconnect.bind(pm2));
    this.pm2List = promisify(pm2.list.bind(pm2));
    this.pm2Start = promisify(pm2.start.bind(pm2));
    this.pm2Stop = promisify(pm2.stop.bind(pm2));
    this.pm2Restart = promisify(pm2.restart.bind(pm2));
    this.pm2Delete = promisify(pm2.delete.bind(pm2));
    this.pm2Describe = promisify(pm2.describe.bind(pm2));
  }

  async initialize(): Promise<void> {
    try {
      await this.pm2Connect();
      this.initialized = true;
      this.log.info('✅ PM2 connected');
    } catch (error) {
      this.log.error('❌ Failed to connect to PM2:', error);
      throw error;
    }
  }

  async shutdown(): Promise<void> {
    if (this.initialized) {
      await this.pm2Disconnect();
      this.initialized = false;
      this.log.info('PM2 disconnected');
    }
  }

  /**
   * Start a task via PM2
   */
  async startTask(datum: TaskDatum): Promise<TaskExecutionResult> {
    if (!this.initialized) {
      return {
        success: false,
        error: 'PM2 not initialized',
        message: 'PM2 service not initialized',
      };
    }

    try {
      this.log.info(`Starting task: ${datum.name}`);

      // Check if task already running
      const existing = await this.pm2Describe(datum.name);
      if (existing.length > 0 && existing[0].pm2_env?.status === 'online') {
        this.log.warn(`Task ${datum.name} already running`);
        return {
          success: false,
          name: datum.name,
          status: 'already_running',
          message: `Task ${datum.name} is already running`,
        };
      }

      // Ensure a valid script or command is provided
      if (!datum.script && !datum.command) {
        this.log.error(`No valid script or command provided for task: ${datum.name}`);
        return {
          success: false,
          name: datum.name,
          error: 'No valid script or command provided',
          message: `Task ${datum.name} could not be started: no script or command specified`,
        };
      }

      // Build PM2 start options from datum
      const pm2Options: pm2.StartOptions = {
        name: datum.name,
        script: datum.script || datum.command,
        args: datum.args,
        cwd: datum.cwd || process.cwd(),
        env: datum.env || {},
        instances: datum.instances || 1,
        max_memory_restart: datum.max_memory_restart || '500M',
        autorestart: true,
        watch: false,
      };

      const proc = await this.pm2Start(pm2Options);

      this.log.info(`✅ Started task: ${datum.name} (PM2 ID: ${proc.pm2_env?.pm_id})`);

      // Publish status to Redis
      await this.publishTaskStatus(datum.name, 'started', {
        pm_id: proc.pm2_env?.pm_id,
      });

      return {
        success: true,
        pm_id: proc.pm2_env?.pm_id,
        name: datum.name,
        status: 'started',
        message: `Task ${datum.name} started successfully`,
      };
    } catch (error) {
      this.log.error(`❌ Failed to start task ${datum.name}:`, error);
      return {
        success: false,
        name: datum.name,
        error: String(error),
        message: `Failed to start task: ${error}`,
      };
    }
  }

  /**
   * Stop a task via PM2
   */
  async stopTask(taskName: string): Promise<TaskExecutionResult> {
    if (!this.initialized) {
      return {
        success: false,
        error: 'PM2 not initialized',
        message: 'PM2 service not initialized',
      };
    }

    try {
      this.log.info(`Stopping task: ${taskName}`);

      const existing = await this.pm2Describe(taskName);
      if (existing.length === 0) {
        this.log.warn(`Task ${taskName} not found`);
        return {
          success: false,
          name: taskName,
          status: 'not_found',
          message: `Task ${taskName} not found`,
        };
      }

      await this.pm2Stop(taskName);
      this.log.info(`✅ Stopped task: ${taskName}`);

      // Publish status to Redis
      await this.publishTaskStatus(taskName, 'stopped', {});

      return {
        success: true,
        name: taskName,
        status: 'stopped',
        message: `Task ${taskName} stopped successfully`,
      };
    } catch (error) {
      this.log.error(`❌ Failed to stop task ${taskName}:`, error);
      return {
        success: false,
        name: taskName,
        error: String(error),
        message: `Failed to stop task: ${error}`,
      };
    }
  }

  /**
   * Restart a task via PM2
   */
  async restartTask(taskName: string): Promise<TaskExecutionResult> {
    if (!this.initialized) {
      return {
        success: false,
        error: 'PM2 not initialized',
        message: 'PM2 service not initialized',
      };
    }

    try {
      this.log.info(`Restarting task: ${taskName}`);
      await this.pm2Restart(taskName);
      this.log.info(`✅ Restarted task: ${taskName}`);

      await this.publishTaskStatus(taskName, 'restarted', {});

      return {
        success: true,
        name: taskName,
        status: 'restarted',
        message: `Task ${taskName} restarted successfully`,
      };
    } catch (error) {
      this.log.error(`❌ Failed to restart task ${taskName}:`, error);
      return {
        success: false,
        name: taskName,
        error: String(error),
        message: `Failed to restart task: ${error}`,
      };
    }
  }

  /**
   * Delete a task from PM2
   */
  async deleteTask(taskName: string): Promise<TaskExecutionResult> {
    if (!this.initialized) {
      return {
        success: false,
        error: 'PM2 not initialized',
        message: 'PM2 service not initialized',
      };
    }

    try {
      this.log.info(`Deleting task: ${taskName}`);
      await this.pm2Delete(taskName);
      this.log.info(`✅ Deleted task: ${taskName}`);

      await this.publishTaskStatus(taskName, 'deleted', {});

      return {
        success: true,
        name: taskName,
        status: 'deleted',
        message: `Task ${taskName} deleted successfully`,
      };
    } catch (error) {
      this.log.error(`❌ Failed to delete task ${taskName}:`, error);
      return {
        success: false,
        name: taskName,
        error: String(error),
        message: `Failed to delete task: ${error}`,
      };
    }
  }

  /**
   * Get status of all tasks
   */
  async listTasks(): Promise<TaskExecutionResult> {
    if (!this.initialized) {
      return {
        success: false,
        error: 'PM2 not initialized',
        message: 'PM2 service not initialized',
      };
    }

    try {
      const list = await this.pm2List();
      this.log.debug(`PM2 process list:`, list);

      return {
        success: true,
        message: `${list.length} tasks running`,
        status: JSON.stringify(list),
      };
    } catch (error) {
      this.log.error(`❌ Failed to list tasks:`, error);
      return {
        success: false,
        error: String(error),
        message: `Failed to list tasks: ${error}`,
      };
    }
  }

  /**
   * Publish task status to Redis channel
   */
  private async publishTaskStatus(
    taskName: string,
    status: string,
    metadata: Record<string, unknown>
  ): Promise<void> {
    const message = JSON.stringify({
      type: 'task_status',
      task_name: taskName,
      status,
      timestamp: new Date().toISOString(),
      ...metadata,
    });

    try {
      await this.redis.publish('b00t:task:status', message);
    } catch (error) {
      this.log.error(`❌ Failed to publish task status for ${taskName}:`, error);
    }
  }
}
