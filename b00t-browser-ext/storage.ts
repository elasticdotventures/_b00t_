// b00t Browser Extension - Enhanced Storage and Buffering System
// Manages local storage, data compression, and efficient buffering

interface StorageQuota {
  used: number
  available: number
  total: number
}

interface BufferConfig {
  maxEvents: number
  maxNetworkEvents: number
  maxScreenshots: number
  compressionThreshold: number
  syncInterval: number
}

interface StorageStats {
  events: number
  networkEvents: number
  screenshots: number
  totalSize: number
  quota: StorageQuota
}

class B00tStorageManager {
  private config: BufferConfig = {
    maxEvents: 500,
    maxNetworkEvents: 200,
    maxScreenshots: 50,
    compressionThreshold: 1024 * 1024, // 1MB
    syncInterval: 30000 // 30 seconds
  }

  private syncTimer: NodeJS.Timeout | null = null
  private pendingWrites: Set<string> = new Set()

  constructor() {
    this.startPeriodicSync()
    this.setupStorageMonitoring()
  }

  // Enhanced event storage with compression
  public async storeEvents(events: any[], key: string = 'b00t_events'): Promise<void> {
    try {
      if (this.pendingWrites.has(key)) {
        console.log(`🥾 b00t: Skipping ${key} write - already pending`)
        return
      }

      this.pendingWrites.add(key)

      const result = await chrome.storage.local.get([key])
      let existingEvents = result[key] || []
      
      // Merge new events
      existingEvents = [...existingEvents, ...events]
      
      // Apply retention policy
      const maxEvents = this.getMaxEventsForKey(key)
      if (existingEvents.length > maxEvents) {
        existingEvents = existingEvents.slice(-maxEvents)
        console.log(`🥾 b00t: Trimmed ${key} to ${maxEvents} items`)
      }

      // Check size and compress if needed
      const dataStr = JSON.stringify(existingEvents)
      const shouldCompress = dataStr.length > this.config.compressionThreshold

      if (shouldCompress) {
        console.log(`🥾 b00t: Compressing ${key} data (${dataStr.length} bytes)`)
        // Simple compression: remove older events if too large
        existingEvents = existingEvents.slice(-Math.floor(maxEvents * 0.7))
      }

      await chrome.storage.local.set({ [key]: existingEvents })
      console.log(`🥾 b00t: Stored ${events.length} ${key} items (total: ${existingEvents.length})`)

    } catch (error) {
      console.warn(`🥾 b00t: Failed to store ${key}:`, error)
    } finally {
      this.pendingWrites.delete(key)
    }
  }

  // Batch storage operation
  public async storeBatch(data: Record<string, any[]>): Promise<void> {
    const operations = Object.entries(data).map(([key, events]) => 
      this.storeEvents(events, key)
    )
    
    await Promise.allSettled(operations)
  }

  // Get storage statistics
  public async getStorageStats(): Promise<StorageStats> {
    try {
      const quota = await this.getStorageQuota()
      const data = await chrome.storage.local.get(['b00t_events', 'b00t_network_events', 'b00t_screenshots'])
      
      const totalSize = JSON.stringify(data).length
      
      return {
        events: (data.b00t_events || []).length,
        networkEvents: (data.b00t_network_events || []).length,
        screenshots: (data.b00t_screenshots || []).length,
        totalSize,
        quota
      }
    } catch (error) {
      console.warn('🥾 b00t: Failed to get storage stats:', error)
      return {
        events: 0,
        networkEvents: 0,
        screenshots: 0,
        totalSize: 0,
        quota: { used: 0, available: 0, total: 0 }
      }
    }
  }

  // Clean up old data based on age and importance
  public async performMaintenance(): Promise<void> {
    try {
      console.log('🥾 b00t: Starting storage maintenance...')
      
      const stats = await this.getStorageStats()
      const quota = stats.quota
      
      // If using > 80% of quota, aggressive cleanup
      const usagePercent = quota.total > 0 ? (quota.used / quota.total) * 100 : 0
      
      if (usagePercent > 80) {
        console.log(`🥾 b00t: High storage usage (${usagePercent.toFixed(1)}%), performing cleanup`)
        await this.performAggressiveCleanup()
      } else if (usagePercent > 60) {
        console.log(`🥾 b00t: Moderate storage usage (${usagePercent.toFixed(1)}%), gentle cleanup`)
        await this.performGentleCleanup()
      }

      console.log('🥾 b00t: Storage maintenance completed')
      
    } catch (error) {
      console.warn('🥾 b00t: Maintenance failed:', error)
    }
  }

  // Export data for backup/analysis
  public async exportData(): Promise<{events: any[], networkEvents: any[], screenshots: any[], metadata: any}> {
    try {
      const data = await chrome.storage.local.get([
        'b00t_events', 
        'b00t_network_events', 
        'b00t_screenshots',
        'authorizedSites'
      ])
      
      return {
        events: data.b00t_events || [],
        networkEvents: data.b00t_network_events || [],
        screenshots: data.b00t_screenshots || [],
        metadata: {
          exportTime: Date.now(),
          authorizedSites: data.authorizedSites || [],
          stats: await this.getStorageStats()
        }
      }
    } catch (error) {
      console.warn('🥾 b00t: Export failed:', error)
      throw error
    }
  }

  // Clear all extension data
  public async clearAllData(): Promise<void> {
    try {
      await chrome.storage.local.clear()
      console.log('🥾 b00t: All data cleared')
    } catch (error) {
      console.warn('🥾 b00t: Failed to clear data:', error)
    }
  }

  private getMaxEventsForKey(key: string): number {
    switch (key) {
      case 'b00t_events':
        return this.config.maxEvents
      case 'b00t_network_events':
        return this.config.maxNetworkEvents
      case 'b00t_screenshots':
        return this.config.maxScreenshots
      default:
        return 100
    }
  }

  private async getStorageQuota(): Promise<StorageQuota> {
    try {
      if ('storage' in navigator && 'estimate' in navigator.storage) {
        const estimate = await navigator.storage.estimate()
        return {
          used: estimate.usage || 0,
          available: (estimate.quota || 0) - (estimate.usage || 0),
          total: estimate.quota || 0
        }
      }
    } catch (error) {
      console.warn('🥾 b00t: Failed to get storage quota:', error)
    }
    
    // Fallback estimates
    return { used: 0, available: 10 * 1024 * 1024, total: 10 * 1024 * 1024 }
  }

  private async performAggressiveCleanup(): Promise<void> {
    // Keep only most recent data
    await this.trimDataToSize('b00t_events', Math.floor(this.config.maxEvents * 0.3))
    await this.trimDataToSize('b00t_network_events', Math.floor(this.config.maxNetworkEvents * 0.3))
    await this.trimDataToSize('b00t_screenshots', Math.floor(this.config.maxScreenshots * 0.2))
  }

  private async performGentleCleanup(): Promise<void> {
    // Keep reasonable amount of recent data
    await this.trimDataToSize('b00t_events', Math.floor(this.config.maxEvents * 0.7))
    await this.trimDataToSize('b00t_network_events', Math.floor(this.config.maxNetworkEvents * 0.7))
    await this.trimDataToSize('b00t_screenshots', Math.floor(this.config.maxScreenshots * 0.6))
  }

  private async trimDataToSize(key: string, maxSize: number): Promise<void> {
    try {
      const result = await chrome.storage.local.get([key])
      const data = result[key] || []
      
      if (data.length > maxSize) {
        const trimmed = data.slice(-maxSize)
        await chrome.storage.local.set({ [key]: trimmed })
        console.log(`🥾 b00t: Trimmed ${key} from ${data.length} to ${trimmed.length} items`)
      }
    } catch (error) {
      console.warn(`🥾 b00t: Failed to trim ${key}:`, error)
    }
  }

  private startPeriodicSync(): void {
    this.syncTimer = setInterval(() => {
      this.performMaintenance().catch(error => {
        console.warn('🥾 b00t: Periodic maintenance failed:', error)
      })
    }, this.config.syncInterval)
  }

  private setupStorageMonitoring(): void {
    // Monitor storage changes for debugging
    chrome.storage.onChanged?.addListener((changes, namespace) => {
      if (namespace === 'local') {
        const keys = Object.keys(changes).filter(key => key.startsWith('b00t_'))
        if (keys.length > 0) {
          console.log('🥾 b00t: Storage changed:', keys)
        }
      }
    })
  }

  // Cleanup method
  public destroy(): void {
    if (this.syncTimer) {
      clearInterval(this.syncTimer)
      this.syncTimer = null
    }
  }
}

// Singleton instance
const storageManager = new B00tStorageManager()

export { storageManager, B00tStorageManager, type StorageStats, type BufferConfig }