// b00t Browser Extension - Background Script  
// Handles network monitoring, extension management, and NATS.io integration

import { natsClient } from "./nats-client"

chrome.runtime.onInstalled.addListener(() => {
  console.log("🥾 b00t browser extension installed")
  
  // Initialize NATS connection
  setTimeout(() => {
    console.log("🥾 b00t: Initializing NATS connection...")
    // NATS client auto-connects on instantiation
  }, 2000) // Wait 2 seconds for extension to fully load
})

// Enhanced network request monitoring (MV3 compatible)
chrome.webRequest.onBeforeRequest.addListener(
  (details) => {
    // Filter out extension's own traffic and dev server
    if (details.url.includes('chrome-extension://') || 
        details.url.includes('localhost:1815') || 
        details.url.includes('localhost:1816') ||
        details.url.includes('plasmo')) {
      return
    }

    // Create initial request tracking entry
    const requestId = `${details.requestId}-${details.tabId}`
    const event: NetworkEvent = {
      timestamp: Date.now(),
      url: details.url,
      method: details.method,
      type: details.type,
      tabId: details.tabId,
      initiator: details.initiator?.url,
      timing: {
        startTime: details.timeStamp
      }
    }

    requestTracker.set(requestId, event)
    
    // Only log user-initiated requests
    if (details.type === 'main_frame' || details.type === 'sub_frame' || details.type === 'xmlhttprequest') {
      console.log('🥾 b00t network request start:', details.method, details.url)
    }
  },
  { urls: ["<all_urls>"] }
)

// Capture request headers
chrome.webRequest.onBeforeSendHeaders.addListener(
  (details) => {
    const requestId = `${details.requestId}-${details.tabId}`
    const event = requestTracker.get(requestId)
    
    if (event && details.requestHeaders) {
      // Store selected headers (avoid sensitive data)
      const safeHeaders: Record<string, string> = {}
      details.requestHeaders.forEach(header => {
        const name = header.name.toLowerCase()
        if (name === 'accept' || name === 'content-type' || name === 'user-agent' || name === 'referer') {
          safeHeaders[name] = header.value || ''
        }
      })
      event.requestHeaders = safeHeaders
    }
  },
  { urls: ["<all_urls>"] },
  ['requestHeaders']
)

// Capture response headers and status
chrome.webRequest.onResponseStarted.addListener(
  (details) => {
    const requestId = `${details.requestId}-${details.tabId}`
    const event = requestTracker.get(requestId)
    
    if (event) {
      event.responseStatus = details.statusCode
      
      if (details.responseHeaders) {
        const safeHeaders: Record<string, string> = {}
        details.responseHeaders.forEach(header => {
          const name = header.name.toLowerCase()
          if (name === 'content-type' || name === 'content-length' || name === 'cache-control') {
            safeHeaders[name] = header.value || ''
          }
        })
        event.responseHeaders = safeHeaders
      }
    }
  },
  { urls: ["<all_urls>"] },
  ['responseHeaders']
)

// Complete request tracking
chrome.webRequest.onCompleted.addListener(
  (details) => {
    const requestId = `${details.requestId}-${details.tabId}`
    const event = requestTracker.get(requestId)
    
    if (event && event.timing) {
      event.timing.endTime = details.timeStamp
      event.timing.duration = details.timeStamp - event.timing.startTime
      event.responseStatus = details.statusCode
      
      // Store completed request
      if (event.type === 'main_frame' || event.type === 'sub_frame' || event.type === 'xmlhttprequest') {
        console.log('🥾 b00t network request completed:', event.method, event.url, `${event.timing.duration}ms`)
        storeNetworkEvent(event)
      }
      
      requestTracker.delete(requestId)
    }
  },
  { urls: ["<all_urls>"] }
)

// Handle request errors
chrome.webRequest.onErrorOccurred.addListener(
  (details) => {
    const requestId = `${details.requestId}-${details.tabId}`
    const event = requestTracker.get(requestId)
    
    if (event && event.timing) {
      event.timing.endTime = details.timeStamp
      event.timing.duration = details.timeStamp - event.timing.startTime
      
      // Store failed request with error info
      if (event.type === 'main_frame' || event.type === 'sub_frame' || event.type === 'xmlhttprequest') {
        console.log('🥾 b00t network request error:', event.method, event.url, details.error)
        storeNetworkEvent({ ...event, responseStatus: 0 }) // 0 indicates error
      }
      
      requestTracker.delete(requestId)
    }
  },
  { urls: ["<all_urls>"] }
)

interface NetworkEvent {
  timestamp: number
  url: string
  method: string
  type: string
  tabId: number
  requestHeaders?: Record<string, string>
  responseStatus?: number
  responseHeaders?: Record<string, string>
  initiator?: string
  redirectChain?: string[]
  timing?: {
    startTime: number
    endTime?: number
    duration?: number
  }
  size?: {
    requestBytes?: number
    responseBytes?: number
  }
}

// Track request timing and response data
const requestTracker = new Map<string, NetworkEvent>()

async function storeNetworkEvent(event: NetworkEvent) {
  try {
    const result = await chrome.storage.local.get(['b00t_network_events'])
    const events = result.b00t_network_events || []
    
    events.push(event)
    
    // Keep only last 50 network events
    const recentEvents = events.slice(-50)
    
    await chrome.storage.local.set({
      'b00t_network_events': recentEvents
    })
  } catch (error) {
    console.warn('🥾 b00t: Failed to store network event', error)
  }
}

// Handle messages from content scripts and popup
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.type === 'GET_EVENTS') {
    chrome.storage.local.get(['b00t_events', 'b00t_network_events', 'b00t_screenshots']).then((result) => {
      sendResponse({
        events: result.b00t_events || [],
        networkEvents: result.b00t_network_events || [],
        screenshots: result.b00t_screenshots || [],
        nats: {
          connected: natsClient.isConnected(),
          operatorId: natsClient.getOperatorId(),
          extensionId: natsClient.getExtensionId(),
          serverUrl: natsClient.getServerUrl()
        }
      })
    })
    return true // Keep message channel open for async response
  }
  
  if (request.type === 'AUTHORIZE_SITE') {
    authorizeSite(request.domain).then(() => {
      sendResponse({ success: true })
    })
    return true
  }

  if (request.type === 'CAPTURE_SCREENSHOT') {
    captureTabScreenshot(sender.tab?.id).then((screenshot) => {
      sendResponse({
        success: !!screenshot,
        screenshot: screenshot
      })
    }).catch((error) => {
      console.warn('🥾 b00t: Screenshot capture failed:', error)
      sendResponse({ success: false, error: error.message })
    })
    return true
  }
})

// Screenshot capture functionality
async function captureTabScreenshot(tabId?: number): Promise<{dataUrl: string, width: number, height: number} | null> {
  try {
    if (!tabId) {
      throw new Error('No tab ID provided')
    }

    // Capture visible tab
    const dataUrl = await chrome.tabs.captureVisibleTab(undefined, {
      format: 'png',
      quality: 90
    })

    // Get tab dimensions
    const tab = await chrome.tabs.get(tabId)
    
    return {
      dataUrl,
      width: tab.width || 1920,
      height: tab.height || 1080
    }
  } catch (error) {
    console.warn('🥾 b00t: Tab screenshot failed:', error)
    return null
  }
}

async function authorizeSite(domain: string) {
  try {
    const result = await chrome.storage.local.get(['authorizedSites'])
    const authorizedSites = result.authorizedSites || []
    
    if (!authorizedSites.includes(domain)) {
      authorizedSites.push(domain)
      await chrome.storage.local.set({ authorizedSites })
      console.log(`🥾 b00t: Authorized site ${domain}`)
    }
  } catch (error) {
    console.warn('🥾 b00t: Failed to authorize site', error)
  }
}

// Tab update listener for navigation tracking
chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  if (changeInfo.status === 'complete' && tab.url) {
    console.log('🥾 b00t tab updated:', tab.url)
  }
})

export {}