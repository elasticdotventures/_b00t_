// PM2 Ecosystem Configuration for b00t-tasker
// 🤓: Uses CommonJS for PM2 compatibility

module.exports = {
  apps: [{
    name: 'b00t-tasker',
    script: './dist/index.js',
    instances: 1,
    exec_mode: 'fork',
    watch: false,
    max_memory_restart: '500M',
    env: {
      NODE_ENV: 'production',
      REDIS_URL: process.env.REDIS_URL || 'redis://localhost:6379',
      PM2_TASKER_CHANNEL: 'b00t:k0mmand3r',
      LOG_LEVEL: 'info'
    },
    error_file: '~/.pm2/logs/b00t-tasker-error.log',
    out_file: '~/.pm2/logs/b00t-tasker-out.log',
    log_date_format: 'YYYY-MM-DD HH:mm:ss Z',
    merge_logs: true,
    autorestart: true,
    max_restarts: 10,
    min_uptime: '10s'
  }]
};
