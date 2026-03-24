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

// b00t-statusline.ts
var fs = __toESM(require("fs"));
var path = __toESM(require("path"));
var child_process = __toESM(require("child_process"));
var input = JSON.parse(process.argv[2] || "{}");
var sessionId = input?.session_id ?? "unknown";
var model = input?.model ?? "?";
var contextTokensUsed = input?.context_tokens_used ?? 0;
var contextTokensMax = input?.context_tokens_max ?? 2e5;
var remainingPct = contextTokensMax > 0 ? Math.round((contextTokensMax - contextTokensUsed) / contextTokensMax * 100) : 100;
var bridgeFile = path.join("/tmp", `b00t-ctx-${sessionId}.json`);
try {
  fs.writeFileSync(bridgeFile, JSON.stringify({ remaining_pct: remainingPct, updated_at: Date.now() }));
} catch {
}
var b00tVersion = "?";
try {
  b00tVersion = child_process.execSync("b00t-cli --version 2>/dev/null", { timeout: 500 }).toString().trim().split(" ").pop() ?? "?";
} catch {
}
var statusLine = `\u{1F97E} b00t ${b00tVersion} | ${model} | ctx ${remainingPct}%`;
process.stdout.write(JSON.stringify({ statusLine }));
process.exit(0);
