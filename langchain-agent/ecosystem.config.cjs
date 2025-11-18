// PM2 Ecosystem Configuration for b00t-langchain-agent
// 🤓: Uses CommonJS for PM2 compatibility

module.exports = {
  apps: [{
    name: 'b00t-langchain-agent',
    script: 'uv',
    args: 'run b00t-langchain serve',
    interpreter: 'none',  // uv is the interpreter
    instances: 1,
    exec_mode: 'fork',
    watch: false,
    max_memory_restart: '2G',  // LangChain agents can be memory-intensive
    env: {
      NODE_ENV: 'production',
      REDIS_URL: process.env.REDIS_URL || 'redis://localhost:6379',
      _B00T_Path: process.env._B00T_Path || '~/.dotfiles/_b00t_',
      LANGCHAIN_COMMAND_CHANNEL: 'b00t:langchain',
      LANGCHAIN_TRACING_V2: 'true',
      LANGCHAIN_PROJECT: 'b00t',
      LOG_LEVEL: 'info',
      // API keys from environment
      ANTHROPIC_API_KEY: process.env.ANTHROPIC_API_KEY,
      LANGCHAIN_API_KEY: process.env.LANGCHAIN_API_KEY,
      LANGSMITH_API_KEY: process.env.LANGSMITH_API_KEY,
      OPENAI_API_KEY: process.env.OPENAI_API_KEY
    },
    error_file: '~/.pm2/logs/b00t-langchain-agent-error.log',
    out_file: '~/.pm2/logs/b00t-langchain-agent-out.log',
    log_date_format: 'YYYY-MM-DD HH:mm:ss Z',
    merge_logs: true,
    autorestart: true,
    max_restarts: 10,
    min_uptime: '30s',  // Allow more time for Python service startup
    restart_delay: 5000,  // 5 second delay between restarts
    exp_backoff_restart_delay: 1000,  // Exponential backoff starting at 1s
    listen_timeout: 8000,  // 8 second timeout for graceful reload
    kill_timeout: 5000,   // 5 second timeout for graceful shutdown
    // Health check via Redis ping
    health_check: {
      enabled: true,
      interval: 30000,  // Check every 30 seconds
      timeout: 5000
    }
  }]
};
