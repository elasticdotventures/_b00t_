"use strict";
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));

// b00t-context-monitor.ts
var fs = __toESM(require("fs"));
var path = __toESM(require("path"));
var input = JSON.parse(process.argv[2] || "{}");
var sessionId = input?.session_id ?? process.env.CLAUDE_SESSION_ID ?? "unknown";
var bridgeFile = path.join("/tmp", `b00t-ctx-${sessionId}.json`);
var contextPct = 100;
try {
  const bridge = JSON.parse(fs.readFileSync(bridgeFile, "utf8"));
  contextPct = bridge.remaining_pct ?? 100;
} catch {
}
var advisory = null;
if (contextPct <= 25) {
  advisory = `\u{1F6A8} CONTEXT CRITICAL: Only ${contextPct}% context remaining. Run /compact or finish current task.`;
} else if (contextPct <= 35) {
  advisory = `\u26A0\uFE0F CONTEXT WARNING: ${contextPct}% context remaining. Consider /compact soon.`;
}
if (advisory) {
  process.stdout.write(JSON.stringify({ additionalContext: advisory }));
}
process.exit(0);
