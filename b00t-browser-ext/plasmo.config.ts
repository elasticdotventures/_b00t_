import { PlasmoConfig } from "plasmo"

const config: PlasmoConfig = {
  manifest: {
    permissions: [
      "activeTab",
      "webRequest", 
      "storage",
      "tabs"
    ],
    host_permissions: [
      "<all_urls>"
    ],
    action: {
      default_title: "b00t",
      default_popup: "popup.html"
    }
    // Removed webRequestBlocking - not compatible with MV3
  }
}

export default config