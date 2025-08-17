// b00t Browser Extension - Visual Snapshot System
// Captures screenshots at navigation boundaries

interface ScreenshotData {
  timestamp: number
  url: string
  dataUrl: string
  dimensions: {
    width: number
    height: number
    viewportWidth: number
    viewportHeight: number
  }
  context: {
    scrollPosition: { x: number, y: number }
    clickTarget?: string
    eventType: 'navigation' | 'click' | 'form_submit'
  }
}

class B00tScreenshotCapture {
  private canvas: HTMLCanvasElement | null = null
  private context: CanvasRenderingContext2D | null = null
  
  constructor() {
    this.initializeCanvas()
  }

  private initializeCanvas() {
    // Create off-screen canvas for image processing
    this.canvas = document.createElement('canvas')
    this.context = this.canvas.getContext('2d')
  }

  public async captureScreenshot(eventType: 'navigation' | 'click' | 'form_submit', clickTarget?: string): Promise<ScreenshotData | null> {
    try {
      // Use Chrome's tab capture API through content script messaging
      const screenshotResult = await this.requestTabScreenshot()
      
      if (!screenshotResult) {
        console.warn('🥾 b00t: Screenshot capture failed')
        return null
      }

      const screenshot: ScreenshotData = {
        timestamp: Date.now(),
        url: window.location.href,
        dataUrl: screenshotResult.dataUrl,
        dimensions: {
          width: screenshotResult.width,
          height: screenshotResult.height,
          viewportWidth: window.innerWidth,
          viewportHeight: window.innerHeight
        },
        context: {
          scrollPosition: {
            x: window.scrollX,
            y: window.scrollY
          },
          clickTarget,
          eventType
        }
      }

      console.log('🥾 b00t screenshot captured:', eventType, screenshot.dimensions)
      return screenshot
    } catch (error) {
      console.warn('🥾 b00t: Screenshot capture error:', error)
      return null
    }
  }

  private async requestTabScreenshot(): Promise<{dataUrl: string, width: number, height: number} | null> {
    return new Promise((resolve) => {
      // Request screenshot from background script
      chrome.runtime.sendMessage(
        { type: 'CAPTURE_SCREENSHOT' },
        (response) => {
          if (chrome.runtime.lastError || !response?.success) {
            resolve(null)
          } else {
            resolve(response.screenshot)
          }
        }
      )
      
      // Timeout after 5 seconds
      setTimeout(() => resolve(null), 5000)
    })
  }

  public async processAndCompressScreenshot(dataUrl: string, maxWidth: number = 800): Promise<string> {
    if (!this.canvas || !this.context) {
      return dataUrl
    }

    return new Promise((resolve) => {
      const img = new Image()
      
      img.onload = () => {
        // Calculate dimensions maintaining aspect ratio
        let { width, height } = img
        if (width > maxWidth) {
          height = (height * maxWidth) / width
          width = maxWidth
        }

        // Resize canvas and draw image
        this.canvas!.width = width
        this.canvas!.height = height
        this.context!.drawImage(img, 0, 0, width, height)
        
        // Compress as JPEG with quality reduction
        const compressedDataUrl = this.canvas!.toDataURL('image/jpeg', 0.7)
        resolve(compressedDataUrl)
      }
      
      img.onerror = () => resolve(dataUrl)
      img.src = dataUrl
    })
  }

  public async storeScreenshot(screenshot: ScreenshotData, compress: boolean = true): Promise<void> {
    try {
      // Optionally compress the screenshot
      if (compress) {
        screenshot.dataUrl = await this.processAndCompressScreenshot(screenshot.dataUrl)
      }

      // Store in chrome.storage.local with size management
      const result = await chrome.storage.local.get(['b00t_screenshots'])
      let screenshots = result.b00t_screenshots || []
      
      screenshots.push(screenshot)
      
      // Keep only last 20 screenshots to manage storage
      if (screenshots.length > 20) {
        screenshots = screenshots.slice(-20)
      }
      
      // Check storage quota
      const storageSize = JSON.stringify(screenshots).length
      const maxSize = 5 * 1024 * 1024 // 5MB limit
      
      if (storageSize > maxSize) {
        // Remove oldest screenshots if exceeding limit
        screenshots = screenshots.slice(-10)
        console.log('🥾 b00t: Screenshot storage trimmed due to size limit')
      }

      await chrome.storage.local.set({ 'b00t_screenshots': screenshots })
      
    } catch (error) {
      console.warn('🥾 b00t: Failed to store screenshot:', error)
    }
  }

  // Clean up old screenshots
  public async cleanupOldScreenshots(maxAge: number = 24 * 60 * 60 * 1000): Promise<void> {
    try {
      const result = await chrome.storage.local.get(['b00t_screenshots'])
      const screenshots = result.b00t_screenshots || []
      
      const cutoff = Date.now() - maxAge
      const validScreenshots = screenshots.filter((s: ScreenshotData) => s.timestamp > cutoff)
      
      if (validScreenshots.length !== screenshots.length) {
        await chrome.storage.local.set({ 'b00t_screenshots': validScreenshots })
        console.log(`🥾 b00t: Cleaned up ${screenshots.length - validScreenshots.length} old screenshots`)
      }
    } catch (error) {
      console.warn('🥾 b00t: Screenshot cleanup failed:', error)
    }
  }
}

export { B00tScreenshotCapture, type ScreenshotData }