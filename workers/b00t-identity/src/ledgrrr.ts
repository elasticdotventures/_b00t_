// b00t -> ledgrrr HTTP client (F2 contract). b00t calls ledgrrr ONLY to
// authorize spend and record usage; everything is fail-closed in prod.
// `LEDGRRR_MODE=mock` (the default until ledgrrr #175 ships) short-circuits
// to an always-ok response so SP1-3 never block on ledgrrr.

export interface LedgrrrEnv {
  LEDGRRR_MODE?: string;     // "mock" | "http"  (unset => "mock")
  LEDGRRR_BASE_URL?: string; // required when mode === "http"
}

export interface AuthorizeSpendInput {
  tenant: string;
  agent: string;
  cost: number;   // cake units
  ref: string;    // caller-supplied idempotency key
}

export interface AuthorizeSpendResult {
  ok: boolean;
  budget_remaining: number;
  reason?: string;
}

export interface RecordUsageInput {
  tenant: string;
  agent: string;
  units: number;  // cake units
  meta?: Record<string, unknown>;
}

export interface RecordUsageResult {
  ok: boolean;
}

const MOCK_BUDGET = 1_000_000;

type FetchImpl = typeof fetch;

function mode(env: LedgrrrEnv): "mock" | "http" {
  return env.LEDGRRR_MODE === "http" ? "http" : "mock";
}

export async function authorizeSpend(
  env: LedgrrrEnv,
  input: AuthorizeSpendInput,
  fetchImpl: FetchImpl = fetch,
): Promise<AuthorizeSpendResult> {
  if (mode(env) === "mock") {
    return { ok: true, budget_remaining: MOCK_BUDGET };
  }
  if (!env.LEDGRRR_BASE_URL) {
    return { ok: false, budget_remaining: 0, reason: "LEDGRRR_BASE_URL not configured" };
  }
  try {
    const res = await fetchImpl(`${env.LEDGRRR_BASE_URL}/v1/authorize-spend`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "Idempotency-Key": input.ref },
      body: JSON.stringify({
        tenant: input.tenant,
        agent: input.agent,
        cost: input.cost,
        ref: input.ref,
      }),
    });
    if (!res.ok) {
      return { ok: false, budget_remaining: 0, reason: `ledgrrr ${res.status}` };
    }
    const body = (await res.json()) as Partial<AuthorizeSpendResult>;
    return {
      ok: body.ok === true,
      budget_remaining: typeof body.budget_remaining === "number" ? body.budget_remaining : 0,
      reason: body.reason,
    };
  } catch (err) {
    // fail closed
    return { ok: false, budget_remaining: 0, reason: `ledgrrr unreachable: ${String(err)}` };
  }
}

export async function recordUsage(
  env: LedgrrrEnv,
  input: RecordUsageInput,
  fetchImpl: FetchImpl = fetch,
): Promise<RecordUsageResult> {
  if (mode(env) === "mock") {
    return { ok: true };
  }
  if (!env.LEDGRRR_BASE_URL) {
    return { ok: false };
  }
  try {
    const res = await fetchImpl(`${env.LEDGRRR_BASE_URL}/v1/usage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        tenant: input.tenant,
        agent: input.agent,
        units: input.units,
        meta: input.meta ?? {},
      }),
    });
    return { ok: res.ok };
  } catch {
    return { ok: false };
  }
}
