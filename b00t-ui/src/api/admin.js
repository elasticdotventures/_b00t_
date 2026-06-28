// Axios API client for b00t-admin backend
// Base URL defaults to http://localhost:31337, overridable via VITE_ADMIN_API env

import axios from 'axios'

const api = axios.create({
  baseURL: import.meta.env.VITE_ADMIN_API || 'http://localhost:31337',
  timeout: 10000,
  headers: { Accept: 'application/json' }
})

/**
 * Handles API errors uniformly — logs a concise prefix and re-throws
 * so callers (stores, components) can handle or render error state.
 */
function handleError (err) {
  const detail =
    err?.response?.data?.message ||
    err?.response?.data?.error ||
    err.message ||
    'Unknown error'
  console.error('[Admin API]', err.config?.url, detail)
  throw err
}

// ── Pipeline ──────────────────────────────────────────────────────────────

/** Fetch current pipeline snapshot */
export function getPipeline () {
  return api.get('/api/admin/pipeline').then(r => r.data).catch(handleError)
}

/** Fetch list of running / available processes */
export function getProcesses () {
  return api.get('/api/admin/processes').then(r => r.data).catch(handleError)
}

// ── Visualizations ────────────────────────────────────────────────────────

/**
 * Fetch viz data by type.
 * @param {'entangle'|'task'} type — visualization type identifier
 */
export function getViz (type) {
  return api.get(`/api/admin/viz/${encodeURIComponent(type)}`)
    .then(r => r.data)
    .catch(handleError)
}

// ── Types ─────────────────────────────────────────────────────────────────

/** Fetch all registered b00t types */
export function getTypes () {
  return api.get('/api/admin/types').then(r => r.data).catch(handleError)
}

/** Fetch detail for a single type by name */
export function getTypeDetail (name) {
  return api.get(`/api/admin/types/${encodeURIComponent(name)}`)
    .then(r => r.data)
    .catch(handleError)
}

// ── Simulation ────────────────────────────────────────────────────────────

/** Advance simulation by one tick */
export function simTick () {
  return api.get('/api/admin/simulate/tick').then(r => r.data).catch(handleError)
}

/** Rollback simulation by one tick */
export function simRollback () {
  return api.get('/api/admin/simulate/rollback').then(r => r.data).catch(handleError)
}

/** Fetch current simulation state */
export function simState () {
  return api.get('/api/admin/simulate/state').then(r => r.data).catch(handleError)
}

// ── Health ────────────────────────────────────────────────────────────────

/** Health-check endpoint */
export function getHealth () {
  return api.get('/api/admin/health').then(r => r.data).catch(handleError)
}
