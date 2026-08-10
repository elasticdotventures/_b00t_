# Fax MVP — Design

**Date:** 2026-08-09
**Status:** Approved for implementation planning
**Tracking:** b00t task #168 (parent mission); this spec covers sub-project 2 of 5
**Memory:** `project_telnyx_fax_service_ledgrrr.md`
**Depends on:** nothing (credproxy, sub-project 1, is explicitly out of scope — see below)

## Background

Second of five sub-projects decomposed out of the Telnyx/wrangler mission (see
`2026-08-09-windows-keystore-credproxy-design.md` for the full decomposition
and sub-project 1). This spec covers the P0 fax send/receive loop: getting
one working end-to-end fax through Telnyx, self-directed (no external
counterparty needed to validate it), per b00t's playable-first doctrine.

## Scope decision: credential delivery

Sub-project 1 (Windows-keystore credential proxy) is **backlogged** (b00t
task #169). This spec uses the simplest possible credential path instead:
`TELNYX_API_KEY` — already present in `~/.env` on this box — passed into
wrangler's container as a plain environment variable at `podman run` time.
No keystore bridge, no `windows-rs`, no bootstrap-seeded podman secret. This
is a deliberate simplification to unblock the fax loop now; revisiting it
for a real secret-lifecycle story is deferred until past dev-convenience
secrets.

## Flow

1. **One-time setup:** create a Telnyx Fax Application (`create_fax_applications`)
   for the test number, with `webhook_url` pointed at a **stable, named**
   dev tunnel (e.g. a `cloudflared` tunnel bound to a fixed hostname) —
   configured once, reused by every test run. (Rejected: an ephemeral
   per-run tunnel, which would require calling `update_fax_applications`
   to repoint the webhook before every send — more moving parts for no
   benefit at this stage.)
2. **Send:** wrangler calls `create_faxes` with `from` = `to` = the test
   number (self-fax) and `media_url` pointing at a static test PDF served
   over that same tunnel. (`upload_media`/`media_name` was considered as an
   alternative to `media_url`, but it still requires fetching from a public
   URL itself — no advantage for a one-shot test PDF; skipped.)
3. **Receive confirmation:** because `to == from`, Telnyx routes the fax
   back to the same number. The Fax Application's `webhook_url` receives
   both the outbound leg's lifecycle (`fax.queued` → `fax.media.processed`
   → `fax.sending.started` → `fax.delivered`) and a separate inbound
   `fax.received` event for the same logical test run.

## P0 success criteria

A single test run produces both the outbound `fax.delivered` and inbound
`fax.received` webhook events, correlated to the same run (via
`client_state` or timing). That is the whole loop — send and receive
exercised together, bi-directional-capable architecture validated, without
needing a second party.

## Document ingestion into ledgrrr

Once a fax completes (either direction), its PDF should land in ledgrrr,
not just Telnyx's own temporary storage. `ledgerr-mcp` already has the
right hook for this: `proxy_docling_ingest_pdf`
(`ledgerr-mcp::mcp_adapter::handle_ingest_pdf`) — a working, already-wired
tool that runs a PDF through `docling` extraction and calls
`service.ingest_pdf(request)`, landing rows in ledgrrr's transaction system
with `docling`-tagged provenance (source, tool version, backend call id).
No new ledgrrr-side code needed for P0 — wrangler calls this existing MCP
tool with the fax's PDF (outbound: the same test PDF already being sent;
inbound: `fax.received`'s media, fetched from Telnyx first) once the
`fax.delivered`/`fax.received` webhook confirms the transfer completed.

## Error handling

`fax.failed` webhook (carries Telnyx's error code/message) is logged
verbatim, not swallowed. If no webhook arrives within a timeout, fall back
to polling `retrieve_faxes` by id rather than hanging indefinitely.

## Out of scope for this spec

- **clawPDF** / real document generation — a static test PDF is sufficient
  for P0; clawPDF's role (converting a real ledgrrr document to PDF) is a
  later, separate concern once this loop works.
- **credproxy** (sub-project 1) — backlogged, see above.
- **Voice/TTS/STT** (sub-project 5) — unrelated capability, tracked
  separately.
- Production public endpoint for wrangler — the dev tunnel is explicitly a
  stopgap; sub-project 3 (cloudflare-os hosting) is where wrangler gets a
  real deployment story. This spec does not block on that.

## References

- Telnyx `create_faxes`, `create_fax_applications`, `upload_media` — schemas
  pulled live via the Telnyx MCP connector, 2026-08-09.
