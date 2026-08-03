#!/usr/bin/env node
// Generate Pingap TOML from a project-local services.mjs route table.
//
// Inputs:
//   PINGAP_CONFIG_DIR      directory containing services.mjs and generated TOML
//                          default: $PINGAP_PROJECT_ROOT/_b00t_/k8s/pingap
//   PINGAP_CONFIG_OUT_DIR  output directory, default: PINGAP_CONFIG_DIR
//   PINGAP_SERVICES_MODULE explicit services module path
//
// Usage:
//   gen-pingap-config.mjs
//   gen-pingap-config.mjs --check

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";

if (process.argv.includes("--help")) {
  console.log(`Usage: gen-pingap-config.mjs [--check]

Environment:
  PINGAP_PROJECT_ROOT      consuming project root
  PINGAP_CONFIG_DIR        directory containing services.mjs
  PINGAP_CONFIG_OUT_DIR    output directory for generated TOML
  PINGAP_SERVICES_MODULE   explicit route module path
`);
  process.exit(0);
}

const projectRoot = resolve(process.env.PINGAP_PROJECT_ROOT ?? process.cwd());
const configDir = resolve(
  process.env.PINGAP_CONFIG_DIR ?? join(projectRoot, "_b00t_", "k8s", "pingap")
);
const outDir = resolve(process.env.PINGAP_CONFIG_OUT_DIR ?? configDir);
const servicesModule = resolve(
  process.env.PINGAP_SERVICES_MODULE ?? join(configDir, "services.mjs")
);

const sourceLabel = relative(projectRoot, servicesModule) || servicesModule;
const { services, certificates } = await import(pathToFileURL(servicesModule).href);

function requireArray(name, value) {
  if (!Array.isArray(value)) {
    throw new Error(`${name} must be an array exported by ${servicesModule}`);
  }
}

function requireObject(name, value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${name} must be an object exported by ${servicesModule}`);
  }
}

requireArray("services", services);
requireObject("certificates", certificates);

const serverName = process.env.PINGAP_SERVER_NAME ?? "devProxy";
const redirectLocation = process.env.PINGAP_REDIRECT_LOCATION ?? `${serverName}Redirect`;

const BANNER = `# GENERATED - do not edit by hand.
# Source: ${sourceLabel}
# Regenerate: PINGAP_PROJECT_ROOT=<repo> pingap-devProxy-b00t/bin/gen-pingap-config.mjs
`;

function genLocations() {
  const blocks = services.map((s) => {
    for (const field of ["name", "host", "upstreamName"]) {
      if (!s[field]) throw new Error(`service is missing ${field}: ${JSON.stringify(s)}`);
    }
    return `[locations.${s.name}]
host = "${s.host}"
upstream = "${s.upstreamName}"
`;
  });
  blocks.push(`[locations.${redirectLocation}]
# Catch-all for the plain-HTTP server; redirects everything to HTTPS.
path = "/"
plugins = ["forceHttps"]
`);
  return `${BANNER}\n${blocks.join("\n")}`;
}

function genUpstreams() {
  const blocks = services.map((s) => {
    if (!s.target) throw new Error(`service is missing target: ${JSON.stringify(s)}`);
    const lines = [`[upstreams.${s.upstreamName}]`, `addrs = ["${s.target}"]`];
    if (s.sni) lines.push(`sni = "${s.sni}"`);
    if (s.verifyCert === false) lines.push("verify_cert = false");
    return `${lines.join("\n")}\n`;
  });
  return `${BANNER}\n${blocks.join("\n")}`;
}

function genCertificates() {
  const blocks = Object.entries(certificates).map(([key, cert]) => {
    const lines = [
      `[certificates.${key}]`,
      `tls_cert = "${cert.tls_cert}"`,
      `tls_key = "${cert.tls_key}"`,
      `domains = "${cert.domains}"`,
    ];
    if (cert.is_default) lines.push("is_default = true");
    return `${lines.join("\n")}\n`;
  });
  return `${BANNER}\n${blocks.join("\n")}`;
}

function genPlugins() {
  return `${BANNER}

[plugins.forceHttps]
category = "redirect"
http_to_https = true
status = 301
`;
}

function genServers() {
  const names = services.map((s) => `    "${s.name}",`).join("\n");
  return `${BANNER}

# Listen addr is mode-dependent (shadow vs cutover) and is rendered by
# pingap-kube-play.sh via envsubst.
#
# global_certificates = true is required for TLS negotiation with the shared
# certificate table.
[servers.${serverName}]
addr = "\${PINGAP_LISTEN_ADDR}"
global_certificates = true
locations = [
${names}
]

[servers.${serverName}Http]
addr = "\${PINGAP_HTTP_ADDR}"
locations = ["${redirectLocation}"]
`;
}

const files = {
  "locations.toml": genLocations(),
  "upstreams.toml": genUpstreams(),
  "certificates.toml": genCertificates(),
  "plugins.toml": genPlugins(),
  "servers.toml": genServers(),
};

const check = process.argv.includes("--check");
let drift = false;

for (const [name, content] of Object.entries(files)) {
  const path = join(outDir, name);
  const existing = existsSync(path) ? readFileSync(path, "utf8") : null;
  if (check) {
    if (existing !== content) {
      console.error(`DRIFT: ${path} differs from generated output`);
      drift = true;
    }
    continue;
  }
  writeFileSync(path, content);
  console.log(`wrote ${path}`);
}

if (check) {
  if (drift) process.exit(1);
  console.log("pingap config: no drift");
}

