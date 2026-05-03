// b00t Browser Extension - NATS.io Integration Client
// Connects browser extension to b00t-website and Cloudflare backend

interface NatsConfig {
  websocketUrl: string
  reconnectDelay: number
  maxReconnectAttempts: number
  heartbeatInterval: number
  commandTimeout: number
}

interface NatsMessage {
  subject: string
  data: any
  timestamp: number
  correlationId?: string
  replyTo?: string
}

interface CommandMessage {
  id: string
  type: 'capture_screenshot' | 'get_telemetry' | 'authorize_site' | 'clear_data' | 'export_data'
  params?: any
  timestamp: number
  origin: 'website' | 'cloudflare' | 'agent'
}

interface ResponseMessage {
  id: string
  success: boolean
  data?: any
  error?: string
  timestamp: number
}

class B00tNatsClient {
  private config: NatsConfig
  private websocket: WebSocket | null = null
  private connected: boolean = false
  private reconnectAttempts: number = 0
  private heartbeatTimer: NodeJS.Timeout | null = null
  private pendingCommands: Map<string, {resolve: Function, reject: Function, timeout: NodeJS.Timeout}> = new Map()
  
  private extensionId: string
  private operatorId: string | null = null

  constructor(config: Partial<NatsConfig> = {}) {
    this.config = {
      websocketUrl: '', // Will be discovered dynamically
      reconnectDelay: 5000,
      maxReconnectAttempts: 10,
      heartbeatInterval: 30000,
      commandTimeout: 15000,
      ...config
    }
    
    // Generate unique extension instance ID
    this.extensionId = `ext_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
    
    this.loadOperatorId()
    this.discoverAndConnect()
  }

  private async loadOperatorId(): Promise<void> {
    try {
      const result = await chrome.storage.local.get(['b00t_operator_id'])
      if (result.b00t_operator_id) {
        this.operatorId = result.b00t_operator_id
      } else {
        // Generate new operator ID if not exists
        this.operatorId = `op_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`
        await chrome.storage.local.set({ 'b00t_operator_id': this.operatorId })
      }
      console.log('🥾 b00t NATS: Operator ID:', this.operatorId)
    } catch (error) {
      console.warn('🥾 b00t NATS: Failed to load operator ID:', error)
    }
  }

  private async discoverNatsServer(): Promise<string> {
    // Discovery endpoints in priority order
    const discoveryEndpoints = [
      // User-configured server (highest priority)
      async () => {
        const result = await chrome.storage.local.get(['b00t_nats_server'])
        return result.b00t_nats_server || null
      },
      
      // b00t-website API discovery
      async () => {
        try {
          const response = await fetch('https://b00t.promptexecution.com/api/browser-extensions/ws')
          if (response.ok) {
            const data = await response.json()
            return data.natsUrl
          }
        } catch (error) {
          console.log('🥾 b00t NATS: API discovery failed:', error.message)
        }
        return null
      },
      
      // Local development server
      async () => {
        try {
          const response = await fetch('http://localhost:8787/api/browser-extensions/ws')
          if (response.ok) {
            const data = await response.json()
            return data.natsUrl
          }
        } catch (error) {
          // Expected to fail in production
        }
        return null
      },
      
      // DNS TXT record discovery (future enhancement)
      async () => {
        // Would use DNS-over-HTTPS to query TXT record for _nats._ws.b00t.promptexecution.com
        return null
      },
      
      // Default fallback servers
      async () => 'wss://nats.b00t.promptexecution.com/ws',
      async () => 'wss://nats-backup.b00t.promptexecution.com/ws'
    ]

    console.log('🥾 b00t NATS: Discovering server endpoints...')

    for (const [index, discoveryMethod] of discoveryEndpoints.entries()) {
      try {
        const serverUrl = await discoveryMethod()
        if (serverUrl) {
          console.log(`🥾 b00t NATS: Server discovered via method ${index + 1}: ${serverUrl}`)
          
          // Cache successful discovery for faster future connections
          if (index < 2) { // Don't cache fallback servers
            await chrome.storage.local.set({ 
              'b00t_nats_server_cache': serverUrl,
              'b00t_nats_server_cache_time': Date.now()
            })
          }
          
          return serverUrl
        }
      } catch (error) {
        console.log(`🥾 b00t NATS: Discovery method ${index + 1} failed:`, error.message)
      }
    }

    throw new Error('Failed to discover NATS server - all discovery methods failed')
  }

  private async discoverAndConnect(): Promise<void> {
    try {
      // Check for cached server first (valid for 5 minutes)
      const result = await chrome.storage.local.get(['b00t_nats_server_cache', 'b00t_nats_server_cache_time'])
      const cacheAge = Date.now() - (result.b00t_nats_server_cache_time || 0)
      
      if (result.b00t_nats_server_cache && cacheAge < 300000) { // 5 minutes
        this.config.websocketUrl = result.b00t_nats_server_cache
        console.log('🥾 b00t NATS: Using cached server:', this.config.websocketUrl)
      } else {
        this.config.websocketUrl = await this.discoverNatsServer()
      }
      
      await this.connect()
    } catch (error) {
      console.error('🥾 b00t NATS: Discovery and connection failed:', error)
      // Still attempt connection with any configured URL
      if (this.config.websocketUrl) {
        await this.connect()
      }
    }
  }

  public async connect(): Promise<void> {
    if (this.connected || !this.operatorId) {
      return
    }

    try {
      console.log('🥾 b00t NATS: Connecting to', this.config.websocketUrl)
      
      this.websocket = new WebSocket(this.config.websocketUrl)
      
      this.websocket.onopen = () => {
        console.log('🥾 b00t NATS: Connected')
        this.connected = true
        this.reconnectAttempts = 0
        this.setupHeartbeat()
        this.subscribe()
      }
      
      this.websocket.onmessage = (event) => {
        this.handleMessage(event.data)
      }
      
      this.websocket.onclose = () => {
        console.log('🥾 b00t NATS: Connection closed')
        this.connected = false
        this.cleanup()
        this.attemptReconnect()
      }
      
      this.websocket.onerror = (error) => {
        console.warn('🥾 b00t NATS: Connection error:', error)
      }
      
    } catch (error) {
      console.warn('🥾 b00t NATS: Connect failed:', error)
      this.attemptReconnect()
    }
  }

  private setupHeartbeat(): void {
    this.heartbeatTimer = setInterval(() => {
      if (this.connected && this.websocket) {
        this.sendMessage({
          subject: 'b00t.heartbeat',
          data: { extensionId: this.extensionId, operatorId: this.operatorId },
          timestamp: Date.now()
        })
      }
    }, this.config.heartbeatInterval)
  }

  private subscribe(): void {
    if (!this.connected || !this.operatorId) return

    // Subscribe to operator-specific commands
    this.sendMessage({
      subject: 'NATS.SUB',
      data: { 
        subject: `b00t.operator.${this.operatorId}.command`,
        queue: `ext_${this.extensionId}`
      },
      timestamp: Date.now()
    })

    // Subscribe to broadcast commands
    this.sendMessage({
      subject: 'NATS.SUB', 
      data: {
        subject: 'b00t.broadcast.command',
        queue: `ext_${this.extensionId}`
      },
      timestamp: Date.now()
    })

    console.log('🥾 b00t NATS: Subscribed to command channels')
  }

  private handleMessage(rawData: string): void {
    try {
      const message: NatsMessage = JSON.parse(rawData)
      
      if (message.subject.includes('.command')) {
        this.handleCommand(message.data as CommandMessage)
      } else if (message.subject.includes('.response')) {
        this.handleResponse(message.data as ResponseMessage)
      } else if (message.subject === 'b00t.heartbeat.ack') {
        // Heartbeat acknowledged
      }
      
    } catch (error) {
      console.warn('🥾 b00t NATS: Failed to parse message:', error)
    }
  }

  private async handleCommand(command: CommandMessage): Promise<void> {
    console.log('🥾 b00t NATS: Received command:', command.type, command.id)
    
    let response: ResponseMessage = {
      id: command.id,
      success: false,
      timestamp: Date.now()
    }

    try {
      switch (command.type) {
        case 'capture_screenshot':
          response = await this.handleScreenshotCommand(command)
          break
          
        case 'get_telemetry':
          response = await this.handleTelemetryCommand(command)
          break
          
        case 'authorize_site':
          response = await this.handleAuthorizeSiteCommand(command)
          break
          
        case 'clear_data':
          response = await this.handleClearDataCommand(command)
          break
          
        case 'export_data':
          response = await this.handleExportDataCommand(command)
          break
          
        default:
          response.error = `Unknown command type: ${command.type}`
      }
      
    } catch (error) {
      response.error = error instanceof Error ? error.message : 'Command execution failed'
    }

    // Send response back
    this.sendMessage({
      subject: `b00t.operator.${this.operatorId}.response`,
      data: response,
      timestamp: Date.now(),
      correlationId: command.id
    })
  }

  private async handleScreenshotCommand(command: CommandMessage): Promise<ResponseMessage> {
    // Request screenshot from content script
    return new Promise((resolve) => {
      chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
        const activeTab = tabs[0]
        if (!activeTab?.id) {
          resolve({
            id: command.id,
            success: false,
            error: 'No active tab found',
            timestamp: Date.now()
          })
          return
        }

        chrome.runtime.sendMessage({ type: 'CAPTURE_SCREENSHOT' }, (response) => {
          resolve({
            id: command.id,
            success: response?.success || false,
            data: response?.screenshot,
            error: response?.error,
            timestamp: Date.now()
          })
        })
      })
    })
  }

  private async handleTelemetryCommand(command: CommandMessage): Promise<ResponseMessage> {
    try {
      const data = await chrome.storage.local.get(['b00t_events', 'b00t_network_events', 'b00t_screenshots'])
      
      return {
        id: command.id,
        success: true,
        data: {
          events: data.b00t_events || [],
          networkEvents: data.b00t_network_events || [],
          screenshots: data.b00t_screenshots || [],
          metadata: {
            extensionId: this.extensionId,
            operatorId: this.operatorId,
            timestamp: Date.now()
          }
        },
        timestamp: Date.now()
      }
    } catch (error) {
      return {
        id: command.id,
        success: false,
        error: error instanceof Error ? error.message : 'Failed to get telemetry',
        timestamp: Date.now()
      }
    }
  }

  private async handleAuthorizeSiteCommand(command: CommandMessage): Promise<ResponseMessage> {
    try {
      const { domain } = command.params || {}
      if (!domain) {
        throw new Error('Domain parameter required')
      }

      const result = await chrome.storage.local.get(['authorizedSites'])
      const authorizedSites = result.authorizedSites || []
      
      if (!authorizedSites.includes(domain)) {
        authorizedSites.push(domain)
        await chrome.storage.local.set({ authorizedSites })
      }

      return {
        id: command.id,
        success: true,
        data: { domain, authorized: true },
        timestamp: Date.now()
      }
    } catch (error) {
      return {
        id: command.id,
        success: false,
        error: error instanceof Error ? error.message : 'Failed to authorize site',
        timestamp: Date.now()
      }
    }
  }

  private async handleClearDataCommand(command: CommandMessage): Promise<ResponseMessage> {
    try {
      await chrome.storage.local.clear()
      // Restore operator ID
      await chrome.storage.local.set({ 'b00t_operator_id': this.operatorId })

      return {
        id: command.id,
        success: true,
        data: { cleared: true },
        timestamp: Date.now()
      }
    } catch (error) {
      return {
        id: command.id,
        success: false,
        error: error instanceof Error ? error.message : 'Failed to clear data',
        timestamp: Date.now()
      }
    }
  }

  private async handleExportDataCommand(command: CommandMessage): Promise<ResponseMessage> {
    try {
      const data = await chrome.storage.local.get()
      
      return {
        id: command.id,
        success: true,
        data: {
          exportData: data,
          metadata: {
            extensionId: this.extensionId,
            operatorId: this.operatorId,
            exportTime: Date.now()
          }
        },
        timestamp: Date.now()
      }
    } catch (error) {
      return {
        id: command.id,
        success: false,
        error: error instanceof Error ? error.message : 'Failed to export data',
        timestamp: Date.now()
      }
    }
  }

  private handleResponse(response: ResponseMessage): void {
    const pending = this.pendingCommands.get(response.id)
    if (pending) {
      clearTimeout(pending.timeout)
      this.pendingCommands.delete(response.id)
      
      if (response.success) {
        pending.resolve(response)
      } else {
        pending.reject(new Error(response.error || 'Command failed'))
      }
    }
  }

  public async sendCommand(type: string, params?: any): Promise<ResponseMessage> {
    if (!this.connected || !this.operatorId) {
      throw new Error('NATS client not connected')
    }

    const command: CommandMessage = {
      id: `cmd_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      type: type as any,
      params,
      timestamp: Date.now(),
      origin: 'website'
    }

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingCommands.delete(command.id)
        reject(new Error('Command timeout'))
      }, this.config.commandTimeout)

      this.pendingCommands.set(command.id, { resolve, reject, timeout })

      this.sendMessage({
        subject: `b00t.website.command`,
        data: command,
        timestamp: Date.now()
      })
    })
  }

  private sendMessage(message: NatsMessage): void {
    if (this.websocket && this.connected) {
      this.websocket.send(JSON.stringify(message))
    }
  }

  private attemptReconnect(): void {
    if (this.reconnectAttempts >= this.config.maxReconnectAttempts) {
      console.warn('🥾 b00t NATS: Max reconnect attempts reached')
      return
    }

    this.reconnectAttempts++
    console.log(`🥾 b00t NATS: Reconnecting in ${this.config.reconnectDelay}ms (attempt ${this.reconnectAttempts})`)
    
    setTimeout(() => {
      this.connect()
    }, this.config.reconnectDelay)
  }

  private cleanup(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer)
      this.heartbeatTimer = null
    }

    // Reject all pending commands
    this.pendingCommands.forEach(({ reject, timeout }) => {
      clearTimeout(timeout)
      reject(new Error('Connection lost'))
    })
    this.pendingCommands.clear()
  }

  public disconnect(): void {
    this.connected = false
    this.cleanup()
    
    if (this.websocket) {
      this.websocket.close()
      this.websocket = null
    }
  }

  public isConnected(): boolean {
    return this.connected
  }

  public getOperatorId(): string | null {
    return this.operatorId
  }

  public getExtensionId(): string {
    return this.extensionId
  }

  public getServerUrl(): string {
    return this.config.websocketUrl
  }
}

// Singleton instance
const natsClient = new B00tNatsClient()

export { natsClient, B00tNatsClient, type CommandMessage, type ResponseMessage, type NatsMessage }