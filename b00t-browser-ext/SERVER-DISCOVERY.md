# b00t Browser Extension - NATS Server Discovery

## How the Browser Extension Finds the NATS Server

The b00t browser extension uses a hierarchical server discovery system to locate the NATS.io messaging server, ensuring reliable connectivity across different deployment environments.

## Discovery Methods (Priority Order)

### 1. **User-Configured Server** (Highest Priority)
Users can manually configure a custom NATS server in extension settings:

```javascript
// Stored in chrome.storage.local
{
  "b00t_nats_server": "wss://my-custom-nats.example.com/ws"
}
```

**Use Cases:**
- Enterprise deployments with private NATS servers
- Development environments with local NATS servers
- Custom b00t ecosystem deployments

### 2. **b00t-website API Discovery**
Extension queries the production b00t-website API for current NATS server:

```bash
GET https://b00t.promptexecution.com/api/browser-extensions/ws

Response:
{
  "message": "NATS WebSocket endpoint",
  "natsUrl": "wss://nats.b00t.promptexecution.com/ws",
  "subjects": {
    "heartbeat": "b00t.heartbeat",
    "commands": "b00t.operator.{operatorId}.command",
    "responses": "b00t.operator.{operatorId}.response",
    "broadcast": "b00t.broadcast.command"
  }
}
```

**Benefits:**
- Dynamic server configuration without extension updates
- Load balancing and server migration support
- Centralized configuration management

### 3. **Local Development Discovery**
For development environments, checks local b00t-website server:

```bash
GET http://localhost:8787/api/browser-extensions/ws
```

**Development Setup:**
- Automatic detection of local development servers
- No configuration needed for development workflow
- Seamless transition between dev and production

### 4. **DNS TXT Record Discovery** (Future Enhancement)
Will query DNS TXT records for service discovery:

```bash
# DNS-over-HTTPS query for:
_nats._ws.b00t.promptexecution.com TXT

# Expected TXT record:
"nats=wss://nats.b00t.promptexecution.com/ws;priority=100"
```

**Advantages:**
- Distributed service discovery
- No single point of failure
- DNS-based load balancing

### 5. **Default Fallback Servers**
Hardcoded fallback servers for reliability:

```javascript
const fallbackServers = [
  'wss://nats.b00t.promptexecution.com/ws',
  'wss://nats-backup.b00t.promptexecution.com/ws'
]
```

## Discovery Process

### 1. **Cache Check** (Performance Optimization)
First checks for cached server URL (valid for 5 minutes):

```javascript
const result = await chrome.storage.local.get([
  'b00t_nats_server_cache', 
  'b00t_nats_server_cache_time'
])

if (cacheAge < 300000) { // 5 minutes
  // Use cached server
  return cachedUrl
}
```

### 2. **Sequential Discovery**
Tries each discovery method in order until successful:

```javascript
const discoveryMethods = [
  userConfiguredServer,    // Priority 1
  apiDiscovery,           // Priority 2  
  localDevDiscovery,      // Priority 3
  dnsDiscovery,           // Priority 4
  fallbackServers         // Priority 5
]

for (const method of discoveryMethods) {
  const serverUrl = await method()
  if (serverUrl) {
    return serverUrl // First successful discovery wins
  }
}
```

### 3. **Result Caching**
Successful discoveries are cached for performance:

```javascript
await chrome.storage.local.set({
  'b00t_nats_server_cache': serverUrl,
  'b00t_nats_server_cache_time': Date.now()
})
```

## Configuration Examples

### Enterprise Deployment
```javascript
// Configure custom NATS server for enterprise deployment
await chrome.storage.local.set({
  'b00t_nats_server': 'wss://nats.corp.example.com:4443/ws'
})
```

### Development Environment
```javascript
// Configure local development NATS server
await chrome.storage.local.set({
  'b00t_nats_server': 'ws://localhost:4222/ws'
})
```

### Multiple Environment Support
```javascript
// Environment-specific server discovery
const environment = window.location.hostname.includes('localhost') 
  ? 'development' 
  : 'production'

const servers = {
  development: 'ws://localhost:4222/ws',
  staging: 'wss://nats-staging.b00t.promptexecution.com/ws', 
  production: 'wss://nats.b00t.promptexecution.com/ws'
}
```

## Error Handling

### Discovery Failures
- **Connection Timeouts**: 10-second timeout per discovery method
- **Network Errors**: Graceful fallback to next discovery method
- **Invalid Responses**: JSON parsing errors handled gracefully
- **Complete Failure**: Extension continues with limited functionality

### Recovery Mechanisms
- **Automatic Retry**: Failed discoveries retry on next connection attempt
- **Cache Invalidation**: Failed cached servers are removed automatically
- **Fallback Chain**: Multiple fallback servers ensure connectivity
- **User Notification**: Extension popup shows discovery status

## Monitoring and Debugging

### Console Logging
Discovery process is fully logged for debugging:

```javascript
console.log('🥾 b00t NATS: Discovering server endpoints...')
console.log('🥾 b00t NATS: Server discovered via method 2: wss://nats.b00t.promptexecution.com/ws')
console.log('🥾 b00t NATS: Using cached server: wss://nats.b00t.promptexecution.com/ws')
```

### Extension Popup
Real-time server discovery status in extension popup:
- Connection status indicator (green/red dot)
- Current NATS server hostname
- Operator and Extension IDs
- Discovery method used

### Network Tab Inspection
Discovery HTTP requests visible in browser DevTools:
- API calls to discovery endpoints
- Response data and timing
- Error responses and status codes

## Security Considerations

### HTTPS/WSS Only
- All production servers use secure WebSocket (WSS)
- TLS certificate validation enforced
- No plaintext WebSocket connections in production

### Domain Validation
- Only trusted domains allowed for server discovery
- User-configured servers require manual approval
- Protection against malicious server injection

### Authentication
- NATS server authentication via JWT tokens (when configured)
- Extension identification via unique operator/extension IDs
- Message signing and verification for command/control

## Future Enhancements

### Service Discovery Protocol
- **mDNS/Bonjour**: Local network service discovery
- **Consul Integration**: Service mesh discovery
- **Kubernetes Services**: Native Kubernetes service discovery

### Load Balancing
- **Round-robin**: Multiple NATS servers with automatic failover
- **Geo-location**: Nearest server selection based on latency
- **Health Checking**: Real-time server health monitoring

### Configuration UI
- **Extension Options Page**: Graphical server configuration
- **Discovery Status**: Real-time discovery method visualization
- **Server Testing**: Test connections to configured servers

---

**🥾 b00t Philosophy**: The server discovery system embodies b00t's principles of resilience, flexibility, and seamless operation across diverse deployment environments while maintaining security and performance.