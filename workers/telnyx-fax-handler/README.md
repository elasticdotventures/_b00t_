# telnyx-fax-handler

The fax MVP's real, deployed Worker (task #168, spec 2) — mirrors the
existing `telnyx-sms-forwarder` Worker's shape rather than the local
`wrangler dev` + dev-tunnel design the spec originally called for. See
the "Superseding update (2026-08-11)" section of
`docs/superpowers/specs/2026-08-09-fax-mvp-design.md` in the parent repo
for the full rationale.

Not deployed yet — real, ready-to-deploy source, same status as the
sibling `workers/b00t-mcp-vault`.

## Routes

- `POST /send-test-fax` — triggers the P0 self-test: sends a fax from
  `TEST_FAX_NUMBER` to itself via Telnyx's `create_faxes` API, using
  `TEST_FAX_MEDIA_URL` as the document.
- `POST /webhook` — Telnyx's fax lifecycle webhook target
  (`fax.queued`/`delivered`/`failed`/`received`/etc.). Logs each event;
  `fax.delivered` + `fax.received` both appearing for one test run is the
  P0 success criterion — check with `wrangler tail` after triggering
  `/send-test-fax`.

ledgrrr document ingestion (`proxy_docling_ingest_pdf`) on a completed fax
is real per the spec but not wired into `/webhook` yet — it needs
`ledgerr-mcp` reachable over HTTP (spec 4), which isn't deployed. The
webhook handler already logs enough (fax id, status, direction) to add
that call later without changing the route shape.

## Deploy

```bash
cd workers/telnyx-fax-handler
pnpm install
wrangler secret put TELNYX_API_KEY
# Also set as plain vars (not secret — not credential material) in
# wrangler.jsonc, or via `wrangler secret put` if preferred:
#   TELNYX_CONNECTION_ID, TEST_FAX_NUMBER, TEST_FAX_MEDIA_URL
wrangler deploy
```

`TEST_FAX_MEDIA_URL` needs a real, publicly reachable PDF URL — the
account's existing `b00t-templates` R2 bucket is a candidate, but nothing
has been uploaded to it yet (this session's Cloudflare MCP connector only
exposes R2 bucket-level operations, not object PUT — needs `wrangler r2
object put` or equivalent from the operator).

Once deployed, point the Telnyx Fax Application's `webhook_url` at this
Worker's real `/webhook` URL (permanent — no tunnel-URL churn to manage).
