import { useState, useEffect } from "react"

import "./style.css"

interface TelemetryStats {
  events: number
  networkEvents: number
  screenshots: number
  totalSize: number
  enabled: boolean
  currentSite: string
}

function IndexPopup() {
  const [data, setData] = useState("")
  const [stats, setStats] = useState<TelemetryStats>({
    events: 0,
    networkEvents: 0,
    screenshots: 0,
    totalSize: 0,
    enabled: false,
    currentSite: ""
  })

  useEffect(() => {
    // Get current tab info and telemetry stats
    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      const currentTab = tabs[0]
      const currentSite = currentTab?.url ? new URL(currentTab.url).hostname : "unknown"
      
      // Get telemetry data
      chrome.runtime.sendMessage({ type: 'GET_EVENTS' }, (response) => {
        if (response) {
          setStats({
            events: response.events?.length || 0,
            networkEvents: response.networkEvents?.length || 0,
            screenshots: response.screenshots?.length || 0,
            totalSize: JSON.stringify(response).length,
            enabled: true, // TODO: check actual site authorization
            currentSite
          })
        }
      })
    })
  }, [])

  const authorizeSite = () => {
    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      const currentTab = tabs[0]
      if (currentTab?.url) {
        const domain = new URL(currentTab.url).hostname
        chrome.runtime.sendMessage({ 
          type: 'AUTHORIZE_SITE', 
          domain 
        }, (response) => {
          if (response?.success) {
            setData(`✅ Authorized for ${domain}`)
            setStats(prev => ({ ...prev, enabled: true }))
          }
        })
      }
    })
  }

  const clearData = () => {
    chrome.storage.local.clear(() => {
      setData("🗑️ All telemetry data cleared")
      setStats(prev => ({ 
        ...prev, 
        events: 0, 
        networkEvents: 0, 
        screenshots: 0, 
        totalSize: 0 
      }))
    })
  }

  return (
    <div
      style={{
        padding: 16,
        minWidth: 320,
        maxWidth: 400
      }}>
      <h2>🥾 b00t Browser Extension</h2>
      <p>Operator telemetry and agent integration</p>
      
      <div style={{ marginTop: 16 }}>
        <h3>Recording Status</h3>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <div
            style={{
              width: 10,
              height: 10,
              borderRadius: "50%",
              backgroundColor: stats.enabled ? "#10b981" : "#ef4444"
            }}
          />
          <span style={{ fontSize: 14 }}>
            {stats.enabled ? `Recording on ${stats.currentSite}` : "Not authorized"}
          </span>
        </div>
      </div>

      <div style={{ marginTop: 16 }}>
        <h3>Telemetry Data</h3>
        <div style={{ fontSize: 12, color: "#6b7280" }}>
          <div>📊 Events: {stats.events}</div>
          <div>🌐 Network: {stats.networkEvents}</div>
          <div>📸 Screenshots: {stats.screenshots}</div>
          <div>💾 Size: {(stats.totalSize / 1024).toFixed(1)} KB</div>
        </div>
      </div>

      <div style={{ marginTop: 16 }}>
        <h3>NATS Connection</h3>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <div
            style={{
              width: 10,
              height: 10,
              borderRadius: "50%",
              backgroundColor: stats.nats?.connected ? "#10b981" : "#ef4444"
            }}
          />
          <span style={{ fontSize: 14 }}>
            {stats.nats?.connected ? "Connected" : "Discovering server..."}
          </span>
        </div>
        {stats.nats?.serverUrl && (
          <div style={{ fontSize: 12, color: "#6b7280", marginTop: 4 }}>
            Server: {new URL(stats.nats.serverUrl).host}
          </div>
        )}
        {stats.nats?.operatorId && (
          <div style={{ fontSize: 12, color: "#6b7280", marginTop: 2 }}>
            Operator: {stats.nats.operatorId.slice(0, 16)}...
          </div>
        )}
        {stats.nats?.extensionId && (
          <div style={{ fontSize: 12, color: "#6b7280" }}>
            Extension: {stats.nats.extensionId.slice(0, 16)}...
          </div>
        )}
      </div>

      <div style={{ marginTop: 16, display: "flex", gap: 8, flexDirection: "column" }}>
        <button
          style={{
            padding: "8px 16px",
            backgroundColor: stats.enabled ? "#6b7280" : "#3b82f6",
            color: "white",
            border: "none",
            borderRadius: 4,
            cursor: "pointer",
            fontSize: 14
          }}
          onClick={authorizeSite}
          disabled={stats.enabled}>
          {stats.enabled ? "✅ Site Authorized" : "🔓 Enable for this site"}
        </button>
        
        <button
          style={{
            padding: "6px 12px",
            backgroundColor: "#ef4444",
            color: "white",
            border: "none",
            borderRadius: 4,
            cursor: "pointer",
            fontSize: 12
          }}
          onClick={clearData}>
          🗑️ Clear All Data
        </button>
      </div>

      {data && (
        <div style={{ 
          marginTop: 16, 
          padding: 8, 
          backgroundColor: "#f3f4f6", 
          borderRadius: 4,
          fontSize: 12
        }}>
          {data}
        </div>
      )}

      <div style={{ marginTop: 16, fontSize: 10, color: "#9ca3af", textAlign: "center" }}>
        Phase 2b Complete: DOM Analysis • Network Monitoring • Visual Snapshots • NATS Command/Control
      </div>
    </div>
  )
}

export default IndexPopup