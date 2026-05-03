import { useState } from "react"

import "./style.css"

function IndexPopup() {
  const [data, setData] = useState("")

  return (
    <div
      style={{
        padding: 16,
        minWidth: 300
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
              backgroundColor: "#10b981"
            }}
          />
          <span>Ready to capture</span>
        </div>
      </div>

      <div style={{ marginTop: 16 }}>
        <h3>Site Authorization</h3>
        <button
          style={{
            padding: "8px 16px",
            backgroundColor: "#3b82f6",
            color: "white",
            border: "none",
            borderRadius: 4,
            cursor: "pointer"
          }}
          onClick={() => {
            setData("Authorized for this site")
          }}>
          Enable for this site
        </button>
      </div>

      {data && (
        <div style={{ marginTop: 16, padding: 8, backgroundColor: "#f3f4f6", borderRadius: 4 }}>
          {data}
        </div>
      )}
    </div>
  )
}

export default IndexPopup