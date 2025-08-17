# b00t Browser Extension

**Experimental browser extension to capture operator navigation and DOM telemetry, relaying data to b00t agents through the operator's browser interface.**

## Overview

A neuromorphic browser extension that creates a bidirectional feedback loop between operator browsing behavior and b00t agent capabilities. Captures user-initiated navigation events and DOM state as training data for the hive mind.

**Privacy Model**: All your base belong to b00t. The extension operates within the b00t ecosystem's trust boundary.

## Current Capabilities (Phase 2 Complete)

### 🔍 Enhanced DOM Analysis Engine
- **Page Structure Extraction**: Comprehensive analysis of forms, buttons, links, and interactive elements
- **Viewport Tracking**: Screen dimensions, scroll positions, and visual context
- **Semantic HTML Analysis**: Heading hierarchy, sections, navigation landmarks
- **Content Detection**: Text analysis, media presence (video, audio, canvas)
- **Metadata Extraction**: Meta tags, language detection, SEO data
- **Accessibility Metrics**: Alt text coverage, ARIA labels, landmark detection

**Example DOM Data Captured:**
```json
{
  "elements": {
    "forms": [{"id": "login", "method": "post", "inputs": 3, "hasFileUpload": false}],
    "buttons": [{"type": "button", "text": "Submit", "disabled": false}],
    "links": [{"href": "/dashboard", "text": "Go to Dashboard", "isExternal": false}]
  },
  "accessibility": {
    "hasAltImages": 12,
    "totalImages": 15,
    "hasAriaLabels": 8,
    "hasLandmarks": 4
  }
}
```

### 🌐 Advanced Network Request Monitoring
- **Complete Lifecycle Tracking**: Request start → headers → response → completion
- **Performance Metrics**: Duration, timing data, error detection
- **Header Analysis**: Safe request/response headers (excludes sensitive data)
- **Request Classification**: Main frame, sub frame, XHR filtering
- **Initiator Tracking**: Understanding request origins and dependencies
- **Self-Traffic Exclusion**: Filters extension and dev server requests

**Example Network Data:**
```json
{
  "url": "https://api.example.com/user",
  "method": "GET",
  "responseStatus": 200,
  "timing": {"duration": 245, "startTime": 1629123456789},
  "requestHeaders": {"accept": "application/json", "content-type": "application/json"},
  "responseHeaders": {"content-type": "application/json", "cache-control": "no-cache"}
}
```

### 📸 Visual Snapshot System
- **Event-Triggered Screenshots**: Captures on navigation, clicks, form submissions
- **Image Optimization**: Automatic compression and resizing for storage efficiency
- **Context Metadata**: Scroll position, target elements, event types
- **Storage Management**: Auto-cleanup with retention policies (max 50 screenshots)
- **Tab Capture Integration**: Background script coordination for screenshot capture

**Screenshot Features:**
- Compressed JPEG format (70% quality)
- Maximum width 800px (maintains aspect ratio)
- Automatic cleanup of screenshots older than 24 hours
- Context tracking (what was clicked, scroll position)

### 💾 Intelligent Storage & Buffering System
- **Smart Retention Policies**: 500 events, 200 network requests, 50 screenshots
- **Quota Monitoring**: Storage usage tracking with automatic cleanup
- **Data Compression**: Size optimization for large datasets
- **Batch Operations**: Efficient storage writes and reads
- **Export Functionality**: Complete data export for analysis
- **Maintenance Automation**: Periodic cleanup and optimization

**Storage Management:**
- Monitors browser storage quota usage
- Performs aggressive cleanup when >80% quota used
- Gentle cleanup when >60% quota used
- Maintains data integrity during cleanup operations

### 🎯 User Interface & Controls
- **Real-time Statistics**: Live telemetry data display in popup
- **Storage Monitoring**: Visual indicators for data usage (events, network, screenshots, size)
- **Site Authorization**: Per-domain enable/disable controls
- **Data Management**: Clear all data functionality
- **Status Indicators**: Recording state and site authorization

## Capture Strategy

### 🎯 User-Initiated Only
- **Click-triggered Recording**: Only captures on actual user interactions
- **Navigation Events**: Page loads, form submissions, button clicks
- **Passive Browsing Ignored**: No background or automatic data collection
- **User Control**: Easy site-by-site authorization

### 🛡️ Privacy & Security
- **Site-Specific Authorization**: Extension enabled per domain basis
- **Self-Exclusion**: Filters out extension's own network traffic
- **Safe Data Only**: No passwords, tokens, or sensitive headers captured
- **Local Processing**: All analysis happens locally in browser

## Technical Architecture

```
Operator Browser (Plasmo Extension)
    ↓ User Click Events + DOM State
Enhanced Storage Manager
    ↓ Buffered Telemetry Data
Local Chrome Storage
    ↓ [Phase 3: MCP Integration]
b00t Agent Ecosystem
    ↓ Learning & Context
Enhanced Agent Capabilities
```

## Technology Stack

- **Framework**: Plasmo (Modern browser extension framework)
- **Language**: TypeScript with React for popup
- **Storage**: Chrome Storage API with intelligent buffering
- **Permissions**: `activeTab`, `webRequest`, `storage`, `tabs`, `tabCapture`
- **Target**: Chrome MV3, Firefox (planned)

## Installation & Usage

### Development Setup
```bash
# Install dependencies
npm install

# Start development server with hot reload
npm run dev

# Build for production
npm run build

# Package for distribution
npm run package
```

### Browser Installation
1. **Chrome/Edge/Brave**: Load unpacked extension from `build/chrome-mv3-dev/`
2. **Enable Developer Mode** in `chrome://extensions/`
3. **Click extension icon** to authorize sites and view telemetry
4. **Navigate and interact** - extension captures user-initiated actions only

### Using the Extension
1. **Authorize Sites**: Click extension popup → "Enable for this site"
2. **View Statistics**: Real-time data in popup (events, network, screenshots)
3. **Monitor Storage**: Track data usage and storage quota
4. **Clear Data**: Reset all telemetry data when needed

## Data Examples

### Telemetry Event Structure
```json
{
  "timestamp": 1629123456789,
  "url": "https://example.com/page",
  "type": "click",
  "target": {
    "tagName": "BUTTON",
    "id": "submit-btn",
    "text": "Submit Form"
  },
  "dom": {
    "title": "Contact Form - Example",
    "forms": 1,
    "buttons": 3,
    "links": 12,
    "viewport": {"width": 1920, "height": 1080}
  }
}
```

## Development Commands

```bash
# Local development
just browser-ext-dev          # Start dev server
just browser-ext-build        # Build production version  
just browser-ext-package      # Create distribution package

# Direct npm commands
npm run dev                   # Development with hot reload
npm run build                 # Production build
npm run package              # Create ZIP for distribution
```

## CI/CD Integration

- **Automated Builds**: GitHub Actions on push to main
- **Versioned Packages**: Uses package.json version for artifact naming
- **Release Process**: Git tags trigger GitHub releases
- **Build Artifacts**: Available for 90 days for testing

### 🔗 Phase 2b: NATS.io Command & Control Integration (COMPLETE)
- **Real-time Bidirectional Communication**: WebSocket connection to b00t-website backend
- **Command/Response Messaging**: Remote browser extension control via NATS.io
- **Extension Discovery**: Automatic registration and heartbeat monitoring
- **Operator-specific Targeting**: Commands routed to specific operator extensions
- **Backend Integration**: Cloudflare Workers with NATS handler for message routing
- **Dashboard Control Interface**: API endpoints for browser extension management

**NATS Integration Features:**
- WebSocket NATS client with automatic reconnection
- Command handlers: screenshot capture, telemetry export, site authorization, data management
- Heartbeat system with 5-minute TTL for extension status tracking
- Correlation ID system for request/response matching
- Error handling and timeout management (15-second command timeout)
- Extension identification with unique operator and extension IDs

**Command Types Supported:**
- `capture_screenshot`: Trigger screenshot capture on active tab
- `get_telemetry`: Export all captured telemetry data  
- `authorize_site`: Authorize extension for specific domain
- `clear_data`: Reset all extension storage
- `export_data`: Export complete extension data for analysis

**Backend API Endpoints:**
- `GET /api/browser-extensions` - List active extensions
- `POST /api/browser-extensions/{operatorId}/command` - Send command to extension
- `GET /api/commands/{commandId}/response` - Retrieve command responses
- `GET /api/browser-extensions/ws` - WebSocket connection info

## Current Status

- ✅ **Phase 1 Complete**: Foundation, permissions, basic capture
- ✅ **Phase 2 Complete**: Enhanced DOM, network monitoring, visual snapshots  
- ✅ **Phase 2b Complete**: NATS.io integration with b00t-website backend
- 🟡 **Phase 3 Planned**: MCP integration and b00t agent relay
- 🔴 **Phase 4 Planned**: Real-time agent communication and feedback
- 🔴 **Phase 5 Planned**: Machine learning and predictive capabilities

## Phase 3 Roadmap: MCP Integration

**Upcoming Features:**
- Local MCP relay server (`b00t-browser-relay`)
- WebSocket/HTTP API for real-time data streaming
- Integration with existing b00t MCP ecosystem
- Agent learning interface and feedback loops
- Context-aware data enrichment

## Performance & Storage

**Resource Usage:**
- **Memory**: ~2-5MB for telemetry buffers
- **Storage**: ~1-10MB depending on activity (auto-managed)
- **CPU**: Minimal impact, event-driven capture only
- **Network**: No external connections (local processing only)

**Data Retention:**
- Events: 500 most recent (rolling buffer)
- Network Requests: 200 most recent (user-initiated only)
- Screenshots: 50 most recent (compressed, auto-cleanup)
- Old Data: Automatic cleanup after 24 hours

## Contributing

The extension follows b00t development principles:

1. **DRY/NRtW**: Leverages existing frameworks (Plasmo, Chrome APIs)
2. **User-Centric**: Only captures intentional user actions
3. **Privacy-First**: Local processing, site authorization required
4. **Performance-Aware**: Intelligent buffering and storage management
5. **Agent-Ready**: Designed for seamless b00t ecosystem integration

## License

MIT License - b00t ecosystem integration

---

**🥾 b00t Philosophy**: This extension serves as the sensory input layer for the neuromorphic b00t agent system, capturing the operator's digital interactions to enhance AI understanding and provide contextual assistance.

*Privacy Note: All captured telemetry operates within the b00t ecosystem trust boundary and is used exclusively to enhance the operator's AI agent capabilities.*