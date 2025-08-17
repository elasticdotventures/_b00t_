// b00t Browser Extension - Content Script
// Captures user-initiated navigation and DOM telemetry

import { B00tScreenshotCapture } from "./screenshot"
import { storageManager } from "./storage"

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
  private screenshotCapture: B00tScreenshotCapture

  constructor() {
    this.screenshotCapture = new B00tScreenshotCapture()
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
        // Handle async click capture
        this.captureClickEvent(target).catch(error => {
          console.warn('🥾 b00t: Click capture failed:', error)
        })
      }
    }, { passive: true })

    // Capture form submissions
    document.addEventListener('submit', (event) => {
      if (!this.enabled) return
      
      // Handle async form capture
      this.captureFormSubmit(event.target as HTMLFormElement).catch(error => {
        console.warn('🥾 b00t: Form capture failed:', error)
      })
    }, { passive: true })

    // Capture page navigation
    this.capturePageNavigation().catch(error => {
      console.warn('🥾 b00t: Navigation capture failed:', error)
    })
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

  private async captureClickEvent(target: HTMLElement) {
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

    // Capture screenshot for navigation clicks
    if (target.tagName === 'A' || target.tagName === 'BUTTON') {
      const targetDescription = `${target.tagName.toLowerCase()}${target.id ? '#' + target.id : ''}${target.className ? '.' + target.className.split(' ')[0] : ''}`
      const screenshot = await this.screenshotCapture.captureScreenshot('click', targetDescription)
      
      if (screenshot) {
        await this.screenshotCapture.storeScreenshot(screenshot, true)
      }
    }
  }

  private async captureFormSubmit(form: HTMLFormElement) {
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

    // Capture screenshot for form submissions
    const formDescription = `form${form.id ? '#' + form.id : ''}${form.className ? '.' + form.className.split(' ')[0] : ''}`
    const screenshot = await this.screenshotCapture.captureScreenshot('form_submit', formDescription)
    
    if (screenshot) {
      await this.screenshotCapture.storeScreenshot(screenshot, true)
    }
  }

  private async capturePageNavigation() {
    const event: B00tTelemetryEvent = {
      timestamp: Date.now(),
      url: window.location.href,
      type: 'navigation',
      dom: this.captureDOMState()
    }

    this.addEvent(event)

    // Capture screenshot for page navigation
    const screenshot = await this.screenshotCapture.captureScreenshot('navigation')
    
    if (screenshot) {
      await this.screenshotCapture.storeScreenshot(screenshot, true)
    }

    // Clean up old screenshots periodically
    if (Math.random() < 0.1) { // 10% chance on each navigation
      await this.screenshotCapture.cleanupOldScreenshots()
    }
  }

  private captureDOMState() {
    return {
      title: document.title,
      url: window.location.href,
      timestamp: Date.now(),
      viewport: {
        width: window.innerWidth,
        height: window.innerHeight,
        scrollX: window.scrollX,
        scrollY: window.scrollY
      },
      elements: {
        forms: this.analyzeForms(),
        buttons: this.analyzeButtons(),
        links: this.analyzeLinks(),
        inputs: this.analyzeInputs(),
        images: document.querySelectorAll('img').length,
        iframes: document.querySelectorAll('iframe').length
      },
      structure: {
        headings: this.analyzeHeadings(),
        sections: document.querySelectorAll('section, article, main, aside').length,
        navigation: document.querySelectorAll('nav').length
      },
      content: {
        textLength: document.body.textContent?.length || 0,
        hasVideo: document.querySelectorAll('video').length > 0,
        hasAudio: document.querySelectorAll('audio').length > 0,
        hasCanvas: document.querySelectorAll('canvas').length > 0
      },
      meta: this.extractMetadata(),
      accessibility: this.analyzeAccessibility()
    }
  }

  private analyzeForms() {
    const forms = Array.from(document.querySelectorAll('form'))
    return forms.map(form => ({
      id: form.id || undefined,
      action: form.action || undefined,
      method: form.method || 'get',
      inputs: form.querySelectorAll('input, select, textarea').length,
      hasFileUpload: form.querySelectorAll('input[type="file"]').length > 0
    }))
  }

  private analyzeButtons() {
    const buttons = Array.from(document.querySelectorAll('button, input[type="button"], input[type="submit"]'))
    return buttons.map(btn => ({
      type: btn.tagName.toLowerCase(),
      text: btn.textContent?.substring(0, 50) || undefined,
      id: btn.id || undefined,
      className: btn.className || undefined,
      disabled: (btn as HTMLButtonElement).disabled
    }))
  }

  private analyzeLinks() {
    const links = Array.from(document.querySelectorAll('a[href]'))
    return links.map(link => ({
      href: (link as HTMLAnchorElement).href,
      text: link.textContent?.substring(0, 50) || undefined,
      target: (link as HTMLAnchorElement).target || undefined,
      isExternal: (link as HTMLAnchorElement).hostname !== window.location.hostname
    }))
  }

  private analyzeInputs() {
    const inputs = Array.from(document.querySelectorAll('input, select, textarea'))
    return inputs.map(input => ({
      type: (input as HTMLInputElement).type || input.tagName.toLowerCase(),
      name: (input as HTMLInputElement).name || undefined,
      id: input.id || undefined,
      required: (input as HTMLInputElement).required,
      placeholder: (input as HTMLInputElement).placeholder || undefined
    }))
  }

  private analyzeHeadings() {
    const headings = Array.from(document.querySelectorAll('h1, h2, h3, h4, h5, h6'))
    return headings.map(heading => ({
      level: parseInt(heading.tagName.charAt(1)),
      text: heading.textContent?.substring(0, 100) || undefined,
      id: heading.id || undefined
    }))
  }

  private extractMetadata() {
    const meta: Record<string, string> = {}
    
    // Standard meta tags
    const metaTags = document.querySelectorAll('meta[name], meta[property]')
    metaTags.forEach(tag => {
      const name = tag.getAttribute('name') || tag.getAttribute('property')
      const content = tag.getAttribute('content')
      if (name && content) {
        meta[name] = content.substring(0, 200) // Limit content length
      }
    })

    // Page language
    const lang = document.documentElement.lang || document.querySelector('html')?.getAttribute('lang')
    if (lang) meta.language = lang

    return meta
  }

  private analyzeAccessibility() {
    return {
      hasAltImages: document.querySelectorAll('img[alt]').length,
      totalImages: document.querySelectorAll('img').length,
      hasAriaLabels: document.querySelectorAll('[aria-label]').length,
      hasSkipLinks: document.querySelectorAll('a[href^="#"]').length,
      hasHeadings: document.querySelectorAll('h1, h2, h3, h4, h5, h6').length > 0,
      hasLandmarks: document.querySelectorAll('main, nav, aside, section').length
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
      await storageManager.storeEvents(this.events.slice(-10), 'b00t_events') // Store last 10 events
      console.log(`🥾 b00t: Stored ${this.events.length} events via enhanced storage`)
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