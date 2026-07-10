/**
 * tap-the-sheep telemetry — local, JSON-shaped, localStorage-backed.
 * No backend, no network. Every tap event is appended to an array.
 */

const STORAGE_KEY = 'tap-the-sheep-telemetry'

export interface TapEvent {
  ts: number       // Date.now() at tap time
  score: number    // cumulative score after this tap
  x: number        // sheep horizontal position (percentage 0–100)
  y: number        // sheep vertical position (percentage 0–100)
  latencyMs: number | null  // time since previous tap (null on first tap)
}

export function loadEvents(): TapEvent[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    return JSON.parse(raw) as TapEvent[]
  } catch {
    return []
  }
}

export function saveEvents(events: TapEvent[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(events))
  } catch {
    // localStorage full or unavailable — silently drop
  }
}

export function recordTap(event: TapEvent): void {
  const events = loadEvents()
  events.push(event)
  saveEvents(events)
}

/** Return the last N events (newest first). Useful for in-game display. */
export function recentTaps(n = 5): TapEvent[] {
  return loadEvents().slice(-n).reverse()
}
