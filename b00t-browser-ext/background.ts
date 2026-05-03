// b00t Browser Extension - Background Script
// Handles network monitoring and extension management

chrome.runtime.onInstalled.addListener(() => {
  console.log("🥾 b00t browser extension installed")
})

// Network request monitoring (MV3 compatible)
chrome.webRequest.onBeforeRequest.addListener(
  (details) => {
    // Filter out extension's own traffic
    if (details.url.includes('chrome-extension://') || 
        details.url.includes('localhost:1815') || 
        details.url.includes('localhost:1816')) {
      return
    }

    // Only capture user-initiated requests
    if (details.type === 'main_frame' || details.type === 'sub_frame') {
      console.log('🥾 b00t network request:', details.method, details.url)
      
      // Store network event
      storeNetworkEvent({
        timestamp: Date.now(),
        url: details.url,
        method: details.method,
        type: details.type,
        tabId: details.tabId
      })
    }
  },
  { urls: ["<all_urls>"] }
  // Removed "requestBody" as webRequestBlocking is not available in MV3
)

interface NetworkEvent {
  timestamp: number
  url: string
  method: string
  type: string
  tabId: number
}

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
    chrome.storage.local.get(['b00t_events', 'b00t_network_events']).then((result) => {
      sendResponse({
        events: result.b00t_events || [],
        networkEvents: result.b00t_network_events || []
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
})

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