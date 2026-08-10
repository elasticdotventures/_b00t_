# AI Audio Interface (Telnyx Voice) — Design

**Date:** 2026-08-10
**Status:** Approved for implementation planning
**Tracking:** b00t task #168 (parent mission); this spec covers sub-project 5 of 5
**Memory:** `project_telnyx_fax_service_ledgrrr.md`
**Depends on:** nothing on the critical path. Extends the STT baseline from
PR #1038 (`b00t-stt-serve`, audio.cpp-backed) in its later phase only.

## Background

Fifth and last sub-project of the Telnyx/wrangler mission: bi-directional
voice — inbound call transcription and outbound TTS/IVR via Telnyx's Voice
API — with voice cloning as an explicitly later phase, not part of the
initial working loop. Directly grounded in Telnyx's real API surface
(pulled live via the Telnyx MCP connector, 2026-08-09):

- `speak_calls_actions` — native TTS, generic voice, text in, plays on the call.
- `start_transcription_calls_actions` / `stop_transcription_calls_actions` — native managed transcription, delivered via a `call.transcription` webhook.
- `start_streaming_calls_actions` — raw call audio as base64 RTP over a WebSocket, for when the hive wants to run its *own* STT instead of Telnyx's.
- `playback` (referenced, not native TTS) — plays an audio *file*, not text — the mechanism for injecting custom-synthesized (e.g. cloned-voice) audio later.

## P0 loop

Self-call test, mirroring the fax MVP's self-test pattern (spec 2):
place an outbound call to our own Telnyx number, `Answer` it, issue
`speak` (native Telnyx TTS, generic voice) with a test message, issue
`start_transcription` (native Telnyx STT) on the same call, and confirm
the `call.transcription` webhook captures the spoken text back. Zero new
infra — pure Call Control orchestration against Telnyx's own managed
TTS/STT. Does not touch `b00t-stt-serve` or any custom synthesis yet.

## Later phase — voice cloning

Swaps both sides out from under the same Call Control skeleton:

- **TTS**: moves from `speak` (text → generic voice) to a custom synthesis
  engine (not yet chosen) + Telnyx's `playback` action (audio file → call;
  no TTS involved on Telnyx's side at all for this path).
- **STT**: optionally moves from native `start_transcription` to Telnyx's
  raw `streaming` action (base64 RTP over WebSocket) feeding
  `b00t-stt-serve` (PR #1038) directly — this is the piece that actually
  extends the hive's own STT stack rather than just consuming Telnyx's.

Neither swap is required for the other — TTS cloning and STT
self-hosting are independent upgrades, not a paired step.

## Credentials

Same `TELNYX_API_KEY` as the fax MVP (spec 2) — plain environment variable,
not the backlogged credproxy (b00t task #169). No separate credential path
for voice; this is the same Telnyx account/key already used for fax.

## Wrangler / architecture fit

Uses the same wrangler middleware and b00t-cli stateless-MCP-proxy pattern
established across the mission — Call Control orchestration (Answer,
speak, start_transcription, hangup) is webhook-driven the same way the fax
flow is, and needs the same stable named dev tunnel (spec 2) for Telnyx to
reach the webhook receiver during local development. No new tunnel
infrastructure — reuses spec 2's.

## Out of scope

- Voice cloning's actual synthesis engine choice (deferred entirely — not
  even a candidate shortlist yet).
- Routing STT through `b00t-stt-serve` for the P0 loop (later-phase only).
- Any cloudflare-os / Gatekeeper integration (specs 3/4's concern, not this one).

## Testing

- P0 success criterion: one self-call test run produces a `call.transcription`
  webhook whose text matches (or closely approximates — STT isn't
  lossless) what `speak` was given, confirming the full loop works.
- `speak`/`start_transcription` are both managed-by-Telnyx; no local
  service to unit-test for P0. Later-phase work (custom TTS, `b00t-stt-serve`
  routing) gets normal test coverage once those components exist.

## References

- Telnyx Call Control API schemas (`speak_calls_actions`,
  `start_transcription_calls_actions`, `start_streaming_calls_actions`) —
  pulled live via the Telnyx MCP connector, 2026-08-09
- `docs/superpowers/specs/2026-08-09-fax-mvp-design.md` — shared dev-tunnel
  and self-test-loop pattern this spec reuses
