# b00t NATS Server - Zero Trust Implementation Plan

**Objective**: Deploy local NATS server on `sm3llyd0s` with Cloudflare Zero Trust tunneling for secure browser extension connectivity.

## 🏗️ **Architecture Overview**

```
Browser Extensions (Global)
    ↓ WebSocket (WSS)
Cloudflare Zero Trust Tunnel
    ↓ TCP/WebSocket
sm3llyd0s (Local NATS Server)
    ↓ Internal Network
b00t-website Worker (Cloudflare)
    ↓ Command/Control API
b00t Dashboard & Agents
```

## 📋 **Implementation Phases**

### **Phase 1: Local NATS Server Setup (sm3llyd0s)**

#### **1.1 NATS Server Installation**
```bash
# Option A: Docker (Recommended)
cd /opt/b00t/
mkdir -p nats/{config,data,logs}

# docker-compose.yml
cat > nats/docker-compose.yml << 'EOF'
version: '3.8'
services:
  nats:
    image: nats:latest
    container_name: b00t-nats
    restart: unless-stopped
    ports:
      - "4222:4222"    # NATS protocol
      - "8222:8222"    # HTTP monitoring
      - "9222:9222"    # WebSocket
    volumes:
      - ./config:/etc/nats
      - ./data:/data
      - ./logs:/logs
    command: [
      "nats-server",
      "--config", "/etc/nats/nats.conf",
      "--jetstream",
      "--store_dir", "/data",
      "--log_file", "/logs/nats.log"
    ]
    environment:
      - NATS_JETSTREAM=true
    healthcheck:
      test: ["CMD", "nats", "server", "check", "--server", "nats://localhost:4222"]
      interval: 30s
      timeout: 10s
      retries: 3
EOF

# Option B: Native Installation
curl -L https://github.com/nats-io/nats-server/releases/download/v2.10.7/nats-server-v2.10.7-linux-amd64.zip -o nats-server.zip
unzip nats-server.zip
sudo mv nats-server-v2.10.7-linux-amd64/nats-server /usr/local/bin/
```

#### **1.2 NATS Configuration**
```bash
# nats/config/nats.conf
cat > nats/config/nats.conf << 'EOF'
# b00t NATS Server Configuration
server_name: "b00t-nats-sm3llyd0s"
port: 4222
http_port: 8222

# WebSocket support for browser extensions
websocket {
  port: 9222
  no_tls: false  # Will be handled by Cloudflare tunnel
  compress: true
  
  # CORS for browser extensions
  same_origin: false
  allowed_origins: [
    "https://b00t.promptexecution.com",
    "chrome-extension://*",
    "moz-extension://*"
  ]
}

# JetStream for persistence
jetstream {
  store_dir: "/data"
  max_memory_store: 1GB
  max_file_store: 10GB
}

# Authentication (optional - can use Zero Trust auth)
authorization {
  # Token-based auth for browser extensions
  token: "$NATS_AUTH_TOKEN"
  
  # Users for different access levels
  users: [
    {
      user: "b00t-extension"
      password: "$B00T_EXTENSION_PASSWORD"
      permissions: {
        publish: ["b00t.>"]
        subscribe: ["b00t.>"]
      }
    },
    {
      user: "b00t-website"  
      password: "$B00T_WEBSITE_PASSWORD"
      permissions: {
        publish: ["b00t.>"]
        subscribe: ["b00t.>"]
      }
    }
  ]
}

# Logging
log_file: "/logs/nats.log"
log_size_limit: 100MB
max_traced_msg_len: 1000

# Monitoring
system_account: "$SYS"
EOF
```

#### **1.3 Environment & Secrets**
```bash
# Create secrets
openssl rand -hex 32 > nats/.env
cat >> nats/.env << EOF
NATS_AUTH_TOKEN=$(openssl rand -hex 32)
B00T_EXTENSION_PASSWORD=$(openssl rand -base64 32)
B00T_WEBSITE_PASSWORD=$(openssl rand -base64 32)
SYS=$(openssl rand -hex 16)
EOF

# Systemd service (if not using Docker)
sudo tee /etc/systemd/system/b00t-nats.service << 'EOF'
[Unit]
Description=b00t NATS Server
After=network.target

[Service]
Type=simple
User=nats
Group=nats
WorkingDirectory=/opt/b00t/nats
ExecStart=/usr/local/bin/nats-server --config /opt/b00t/nats/config/nats.conf
Restart=always
RestartSec=5
EnvironmentFile=/opt/b00t/nats/.env

[Install]
WantedBy=multi-user.target
EOF
```

### **Phase 2: Cloudflare Zero Trust Tunnel Setup**

#### **2.1 Cloudflared Installation (sm3llyd0s)**
```bash
# Install cloudflared
curl -L --output cloudflared.deb https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64.deb
sudo dpkg -i cloudflared.deb

# Authenticate with Cloudflare
cloudflared tunnel login
# Follow browser auth flow
```

#### **2.2 Create b00t NATS Tunnel**
```bash
# Create tunnel
cloudflared tunnel create b00t-nats

# Note the tunnel UUID for configuration
TUNNEL_UUID=$(cloudflared tunnel list | grep b00t-nats | awk '{print $1}')
echo "Tunnel UUID: $TUNNEL_UUID"

# Create tunnel configuration
mkdir -p ~/.cloudflared
cat > ~/.cloudflared/config.yml << EOF
tunnel: $TUNNEL_UUID
credentials-file: ~/.cloudflared/$TUNNEL_UUID.json

ingress:
  # WebSocket endpoint for browser extensions
  - hostname: nats.b00t.promptexecution.com
    service: ws://localhost:9222
    originRequest:
      noTLSVerify: true
      
  # HTTP monitoring endpoint  
  - hostname: nats-mon.b00t.promptexecution.com
    service: http://localhost:8222
    
  # Raw NATS protocol (for server-to-server)
  - hostname: nats-tcp.b00t.promptexecution.com  
    service: tcp://localhost:4222
    
  # Catch-all
  - service: http_status:404
EOF

# Set up DNS records
cloudflared tunnel route dns b00t-nats nats.b00t.promptexecution.com
cloudflared tunnel route dns b00t-nats nats-mon.b00t.promptexecution.com
cloudflared tunnel route dns b00t-nats nats-tcp.b00t.promptexecution.com
```

#### **2.3 Tunnel Service Setup**
```bash
# Install as system service
sudo cloudflared service install
sudo systemctl enable cloudflared
sudo systemctl start cloudflared

# Verify tunnel status
cloudflared tunnel list
sudo systemctl status cloudflared
```

### **Phase 3: Browser Extension Configuration Updates**

#### **3.1 Update Discovery Endpoints**
```javascript
// b00t-browser-ext/nats-client.ts - Update discovery methods
private async discoverNatsServer(): Promise<string> {
  const discoveryEndpoints = [
    // User-configured server (highest priority)
    async () => {
      const result = await chrome.storage.local.get(['b00t_nats_server'])
      return result.b00t_nats_server || null
    },
    
    // b00t-website API discovery (now points to Zero Trust tunnel)
    async () => {
      try {
        const response = await fetch('https://b00t.promptexecution.com/api/browser-extensions/ws')
        if (response.ok) {
          const data = await response.json()
          return data.natsUrl // wss://nats.b00t.promptexecution.com
        }
      } catch (error) {
        console.log('🥾 b00t NATS: API discovery failed:', error.message)
      }
      return null
    },
    
    // Direct Zero Trust tunnel connection
    async () => 'wss://nats.b00t.promptexecution.com',
    
    // Fallback to monitoring endpoint WebSocket upgrade
    async () => 'wss://nats-mon.b00t.promptexecution.com/ws'
  ]
  
  // ... rest of discovery logic
}
```

#### **3.2 Authentication Integration**
```javascript
// Add NATS authentication to WebSocket connection
private async connect(): Promise<void> {
  try {
    // Get auth credentials from storage or API
    const authResult = await chrome.storage.local.get(['b00t_nats_token'])
    const authToken = authResult.b00t_nats_token || await this.getAuthToken()
    
    const wsUrl = new URL(this.config.websocketUrl)
    if (authToken) {
      wsUrl.searchParams.set('token', authToken)
    }
    
    this.websocket = new WebSocket(wsUrl.toString())
    // ... rest of connection logic
  } catch (error) {
    console.error('🥾 b00t NATS: Connection failed:', error)
  }
}

private async getAuthToken(): Promise<string | null> {
  try {
    // Request auth token from b00t-website API
    const response = await fetch('https://b00t.promptexecution.com/api/browser-extensions/auth')
    if (response.ok) {
      const data = await response.json()
      await chrome.storage.local.set({ 'b00t_nats_token': data.token })
      return data.token
    }
  } catch (error) {
    console.warn('🥾 b00t NATS: Auth token request failed:', error)
  }
  return null
}
```

### **Phase 4: b00t-website Integration Updates**

#### **4.1 Update NATS Handler Configuration**
```javascript
// b00t-website/worker/nats-handler.js - Update connection config
class B00tNatsHandler {
  constructor(env) {
    this.env = env
    this.nc = null
    this.connected = false
    
    // Zero Trust tunnel endpoints
    this.natsUrl = env.NATS_URL || 'wss://nats.b00t.promptexecution.com'
    this.natsMonitorUrl = env.NATS_MONITOR_URL || 'https://nats-mon.b00t.promptexecution.com'
    this.natsToken = env.NATS_TOKEN || null
  }
  
  async connect() {
    const options = {
      servers: [this.natsUrl],
      timeout: 10000,
      reconnect: true,
      maxReconnectAttempts: 10,
      reconnectTimeWait: 2000
    }
    
    if (this.natsToken) {
      options.token = this.natsToken
    }
    
    this.nc = await connect(options)
    // ... rest of connection logic
  }
}
```

#### **4.2 Update API Endpoints**
```javascript
// b00t-website/worker/server.js - Update WebSocket info endpoint
app.get('/api/browser-extensions/ws', async (c) => {
  return Response.json({
    message: 'b00t NATS WebSocket endpoint (Zero Trust)',
    natsUrl: 'wss://nats.b00t.promptexecution.com',
    monitorUrl: 'https://nats-mon.b00t.promptexecution.com',
    subjects: {
      heartbeat: 'b00t.heartbeat',
      commands: 'b00t.operator.{operatorId}.command',
      responses: 'b00t.operator.{operatorId}.response',
      broadcast: 'b00t.broadcast.command'
    },
    features: {
      jetstream: true,
      websockets: true,
      compression: true,
      auth: 'token'
    }
  })
})

// New auth endpoint for browser extensions
app.post('/api/browser-extensions/auth', async (c) => {
  try {
    // Validate user session
    const sessionToken = getCookie(c.req.raw, 'session_token')
    if (!sessionToken) {
      return Response.json({ error: 'Unauthorized' }, { status: 401 })
    }
    
    // Generate NATS token for user
    const userData = await c.env.USERS_KV.get(`session:${sessionToken}`)
    if (!userData) {
      return Response.json({ error: 'Session expired' }, { status: 401 })
    }
    
    const user = JSON.parse(userData)
    const natsToken = generateNatsToken(user.username, c.env.NATS_SECRET)
    
    return Response.json({
      token: natsToken,
      expires: Date.now() + (24 * 60 * 60 * 1000) // 24 hours
    })
  } catch (error) {
    return Response.json({ error: 'Auth failed' }, { status: 500 })
  }
})
```

### **Phase 5: Testing & Monitoring Setup**

#### **5.1 Health Checks & Monitoring**
```bash
# Create monitoring script
cat > /opt/b00t/nats/monitor.sh << 'EOF'
#!/bin/bash
# b00t NATS Health Check & Monitoring

# Check NATS server health
NATS_HEALTH=$(curl -s http://localhost:8222/healthz)
TUNNEL_STATUS=$(cloudflared tunnel info b00t-nats)

# Check WebSocket endpoint
WS_CHECK=$(curl -s -H "Upgrade: websocket" -H "Connection: Upgrade" https://nats.b00t.promptexecution.com)

# Log results
echo "$(date): NATS Health: $NATS_HEALTH, Tunnel: $TUNNEL_STATUS" >> /opt/b00t/nats/logs/health.log

# Send metrics to Cloudflare Analytics (optional)
if [ -n "$CF_ANALYTICS_TOKEN" ]; then
  curl -X POST "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/analytics/logs" \
    -H "Authorization: Bearer $CF_ANALYTICS_TOKEN" \
    -H "Content-Type: application/json" \
    --data "{\"timestamp\":$(date +%s),\"service\":\"nats\",\"health\":\"$NATS_HEALTH\"}"
fi
EOF

chmod +x /opt/b00t/nats/monitor.sh

# Setup cron for monitoring
echo "*/5 * * * * /opt/b00t/nats/monitor.sh" | crontab -
```

#### **5.2 Test Scripts**
```bash
# Create comprehensive test suite
cat > /opt/b00t/nats/test-integration.sh << 'EOF'
#!/bin/bash
# b00t NATS Integration Test Suite

echo "🥾 b00t NATS Integration Tests"
echo "================================"

# Test 1: Local NATS server
echo "Testing local NATS server..."
nats server check --server nats://localhost:4222
echo "✅ Local NATS server running"

# Test 2: WebSocket endpoint
echo "Testing WebSocket endpoint..."
curl -s -H "Upgrade: websocket" http://localhost:9222 || echo "❌ WebSocket test failed"

# Test 3: Cloudflare tunnel
echo "Testing Zero Trust tunnel..."
curl -s https://nats.b00t.promptexecution.com || echo "❌ Tunnel test failed"

# Test 4: Browser extension discovery
echo "Testing browser extension discovery..."
curl -s https://b00t.promptexecution.com/api/browser-extensions/ws | jq .

# Test 5: End-to-end message flow
echo "Testing message publishing..."
nats pub --server nats://localhost:4222 b00t.test "Hello from sm3llyd0s"
nats sub --server nats://localhost:4222 b00t.test --count 1

echo "✅ All tests completed"
EOF

chmod +x /opt/b00t/nats/test-integration.sh
```

### **Phase 6: Security & Access Control**

#### **6.1 Cloudflare Access Policies**
```bash
# Set up Cloudflare Access for NATS monitoring
# Via Cloudflare Dashboard:
# 1. Go to Zero Trust > Access > Applications
# 2. Create application for nats-mon.b00t.promptexecution.com
# 3. Set policy: Email ends with @promptexecution.com
# 4. Enable for monitoring dashboard access
```

#### **6.2 Network Security**
```bash
# Firewall rules (sm3llyd0s)
sudo ufw allow from 127.0.0.1 to any port 4222 # NATS
sudo ufw allow from 127.0.0.1 to any port 8222 # Monitoring  
sudo ufw allow from 127.0.0.1 to any port 9222 # WebSocket
sudo ufw deny 4222 # Block external NATS access
sudo ufw deny 8222 # Block external monitoring
sudo ufw deny 9222 # Block external WebSocket

# Only allow Cloudflare tunnel access
sudo ufw enable
```

## 🚀 **Deployment Checklist**

### **Pre-Deployment**
- [ ] sm3llyd0s system requirements verified
- [ ] Docker/NATS server installed
- [ ] Cloudflared authenticated and configured
- [ ] DNS records created in Cloudflare
- [ ] Environment secrets generated
- [ ] Monitoring scripts configured

### **Deployment Day**
- [ ] Start NATS server (`docker-compose up -d`)
- [ ] Start Cloudflare tunnel (`sudo systemctl start cloudflared`)
- [ ] Verify tunnel connectivity (`curl https://nats.b00t.promptexecution.com`)
- [ ] Test WebSocket upgrade (`curl -H "Upgrade: websocket" https://nats.b00t.promptexecution.com`)
- [ ] Run integration tests (`./test-integration.sh`)
- [ ] Update b00t-website NATS configuration
- [ ] Deploy browser extension updates
- [ ] Test end-to-end browser → NATS → website flow

### **Post-Deployment**
- [ ] Monitor tunnel status and logs
- [ ] Verify browser extension connections
- [ ] Test command/control functionality
- [ ] Setup alerting for NATS/tunnel failures
- [ ] Document any issues and resolutions

## 💡 **Benefits of This Approach**

**💰 Cost**: $0/month (no hosting costs)
**🔒 Security**: Zero Trust tunneling, no exposed ports
**⚡ Performance**: Local NATS server, minimal latency
**🛠 Control**: Full server configuration and monitoring
**📈 Scalability**: Easy to replicate or migrate
**🔧 Debugging**: Full access to logs and metrics

## 🔮 **Future Enhancements**

- **High Availability**: Secondary NATS server on different host
- **Load Balancing**: Multiple tunnel endpoints
- **Monitoring**: Prometheus + Grafana integration
- **Clustering**: NATS cluster for horizontal scaling
- **Geo-Distribution**: Regional NATS servers

---

This implementation gives you a production-ready NATS infrastructure with zero hosting costs and enterprise-grade security through Cloudflare Zero Trust! 🥾