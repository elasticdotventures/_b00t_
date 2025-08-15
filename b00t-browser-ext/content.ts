// b00t Browser Extension - Content Script
// Captures user-initiated navigation and DOM telemetry

console.log("🥾 b00t browser extension loaded")

interface B00tTelemetryEvent {
  timestamp: number
  url: string
  type: 'click' | 'navigation' | 'form_submit'
  target?: {
    tagName: string
    id?: string
    className?: string
    href?: string
    text?: string
  }
  dom?: {
    title: string
    forms: number
    buttons: number
    links: number
  }
}

class B00tTelemetryCapture {
  private enabled: boolean = false
  private events: B00tTelemetryEvent[] = []

  constructor() {
    this.init()
  }

  private init() {
    // Check if extension is authorized for this site
    this.checkAuthorization()
    
    // Set up event listeners for user interactions
    this.setupEventListeners()
  }

  private async checkAuthorization() {
    try {
      const result = await chrome.storage.local.get(['authorizedSites'])
      const authorizedSites = result.authorizedSites || []
      const currentDomain = window.location.hostname
      
      this.enabled = authorizedSites.includes(currentDomain)
      console.log(`🥾 b00t capture ${this.enabled ? 'enabled' : 'disabled'} for ${currentDomain}`)
    } catch (error) {
      console.warn('🥾 b00t: Failed to check authorization', error)
    }
  }

  private setupEventListeners() {
    // Capture click events
    document.addEventListener('click', (event) => {
      if (!this.enabled) return
      
      const target = event.target as HTMLElement
      
      // Filter for navigation-relevant clicks
      if (this.isNavigationClick(target)) {
        this.captureClickEvent(target)
      }
    }, { passive: true })

    // Capture form submissions
    document.addEventListener('submit', (event) => {
      if (!this.enabled) return
      
      this.captureFormSubmit(event.target as HTMLFormElement)
    }, { passive: true })

    // Capture page navigation
    this.capturePageNavigation()
  }

  private isNavigationClick(target: HTMLElement): boolean {
    // Check if click is on navigation elements
    return (
      target.tagName === 'A' ||
      target.tagName === 'BUTTON' ||
      target.getAttribute('role') === 'button' ||
      target.onclick !== null ||
      target.closest('a') !== null ||
      target.closest('button') !== null
    )
  }

  private captureClickEvent(target: HTMLElement) {
    const event: B00tTelemetryEvent = {
      timestamp: Date.now(),
      url: window.location.href,
      type: 'click',
      target: {
        tagName: target.tagName,
        id: target.id || undefined,
        className: target.className || undefined,
        href: (target as HTMLAnchorElement).href || undefined,
        text: target.textContent?.substring(0, 100) || undefined
      },
      dom: this.captureDOMState()
    }

    this.addEvent(event)
  }

  private captureFormSubmit(form: HTMLFormElement) {
    const event: B00tTelemetryEvent = {
      timestamp: Date.now(),
      url: window.location.href,
      type: 'form_submit',
      target: {
        tagName: 'FORM',
        id: form.id || undefined,
        className: form.className || undefined
      },
      dom: this.captureDOMState()
    }

    this.addEvent(event)
  }

  private capturePageNavigation() {
    const event: B00tTelemetryEvent = {
      timestamp: Date.now(),
      url: window.location.href,
      type: 'navigation',
      dom: this.captureDOMState()
    }

    this.addEvent(event)
  }

  private captureDOMState() {
    return {
      title: document.title,
      forms: document.querySelectorAll('form').length,
      buttons: document.querySelectorAll('button, input[type="button"], input[type="submit"]').length,
      links: document.querySelectorAll('a[href]').length
    }
  }

  private addEvent(event: B00tTelemetryEvent) {
    this.events.push(event)
    
    // Keep only last 100 events in memory
    if (this.events.length > 100) {
      this.events = this.events.slice(-100)
    }

    // Store in chrome storage
    this.storeEvents()
    
    console.log('🥾 b00t captured event:', event.type, event.target?.tagName)
  }

  private async storeEvents() {
    try {
      await chrome.storage.local.set({
        'b00t_events': this.events
      })
    } catch (error) {
      console.warn('🥾 b00t: Failed to store events', error)
    }
  }

  // Public method to enable/disable capture
  public setEnabled(enabled: boolean) {
    this.enabled = enabled
    console.log(`🥾 b00t capture ${enabled ? 'enabled' : 'disabled'}`)
  }

  // Public method to get captured events
  public getEvents(): B00tTelemetryEvent[] {
    return [...this.events]
  }
}

// Initialize the telemetry capture
const b00tCapture = new B00tTelemetryCapture()

// Expose to window for debugging
;(window as any).b00tCapture = b00tCapture

export {}