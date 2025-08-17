import { PlasmoConfig } from "plasmo"

const config: PlasmoConfig = {
  manifest: {
    permissions: [
      "activeTab",
      "webRequest", 
      "storage",
      "tabs",
      "tabCapture"
    ],
    host_permissions: [
      "<all_urls>"
    ]
    // Removed webRequestBlocking - not compatible with MV3
  }
}

export default config