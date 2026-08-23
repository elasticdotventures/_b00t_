// telnyx-fax-handler: the fax MVP's real, deployed Worker — mirrors
// telnyx-sms-forwarder's shape (single fetch handler, no framework, a
// real Worker secret for TELNYX_API_KEY) rather than the local
// wrangler-dev + tunnel design this spec originally had.
//
// Two routes:
//   POST /send-test-fax  — triggers the P0 self-fax test (send to our own
//                          number) via Telnyx's create_faxes API.
//   POST /webhook        — receives Telnyx's fax lifecycle webhooks
//                          (fax.queued/delivered/failed/received/etc.),
//                          logs the event, returns 200 OK.
//
// ledgrrr document ingestion (proxy_docling_ingest_pdf) on fax.delivered/
// fax.received is real per the fax MVP spec, but not wired here yet — it
// needs ledgerr-mcp reachable over HTTP (spec 4), which isn't deployed.
// The webhook handler logs enough to add that call later without
// redesigning the route.

export interface Env {
  TELNYX_API_KEY: string;
  TELNYX_CONNECTION_ID: string;
  TEST_FAX_NUMBER: string; // E.164, used as both `from` and `to` for the self-test
  TEST_FAX_MEDIA_URL: string; // publicly reachable PDF URL (e.g. an R2 public bucket URL)
}

interface TelnyxFaxWebhookPayload {
  data?: {
    event_type?: string;
    payload?: {
      id?: string;
      status?: string;
      direction?: "inbound" | "outbound";
      from?: string;
      to?: string;
      client_state?: string;
    };
  };
}

async function sendTestFax(env: Env): Promise<Response> {
  const response = await fetch("https://api.telnyx.com/v2/faxes", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${env.TELNYX_API_KEY}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      connection_id: env.TELNYX_CONNECTION_ID,
      from: env.TEST_FAX_NUMBER,
      to: env.TEST_FAX_NUMBER,
      media_url: env.TEST_FAX_MEDIA_URL,
    }),
  });

  const body = await response.text();
  console.log("Telnyx create_faxes response:", response.status, body);

  if (!response.ok) {
    return new Response(`Failed to send test fax: ${body}`, { status: 502 });
  }
  return new Response(body, {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

async function handleWebhook(request: Request): Promise<Response> {
  let payload: TelnyxFaxWebhookPayload;
  try {
    payload = await request.json();
  } catch (err) {
    console.error("Failed to parse webhook JSON payload", err);
    return new Response("Invalid JSON", { status: 400 });
  }

  const eventType = payload?.data?.event_type;
  const fax = payload?.data?.payload;
  console.log(
    "Telnyx fax webhook:",
    eventType,
    "id=", fax?.id,
    "status=", fax?.status,
    "direction=", fax?.direction
  );

  // P0 success criterion (per the fax MVP spec): fax.delivered (outbound
  // leg) and fax.received (inbound leg, since to==from routes back to the
  // same number) both observed for one correlated test run. Logging here
  // is the P0 verification mechanism — read these logs (`wrangler tail`)
  // to confirm the self-test loop actually completed both directions.

  return new Response(`Acknowledged: ${eventType ?? "unknown event"}`, {
    status: 200,
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "POST" && url.pathname === "/send-test-fax") {
      return sendTestFax(env);
    }
    if (request.method === "POST" && url.pathname === "/webhook") {
      return handleWebhook(request);
    }

    return new Response("Not found", { status: 404 });
  },
};
