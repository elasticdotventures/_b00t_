import { describe, it, expect } from "vitest";
import { authorizeSpend, recordUsage } from "../src/ledgrrr";

describe("ledgrrr client — mock mode", () => {
  it("authorizeSpend returns always-ok with the mock budget", async () => {
    const r = await authorizeSpend({}, { tenant: "t", agent: "a", cost: 10, ref: "r1" });
    expect(r).toEqual({ ok: true, budget_remaining: 1_000_000 });
  });
  it("recordUsage returns ok", async () => {
    expect(await recordUsage({ LEDGRRR_MODE: "mock" }, { tenant: "t", agent: "a", units: 3 }))
      .toEqual({ ok: true });
  });
});

describe("ledgrrr client — http mode", () => {
  it("authorizeSpend POSTs the F2 body with an idempotency key and maps the response", async () => {
    let seen: { url: string; init: RequestInit } | null = null;
    const stub: typeof fetch = (async (url: string, init: RequestInit) => {
      seen = { url, init };
      return new Response(JSON.stringify({ ok: true, budget_remaining: 42 }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }) as unknown as typeof fetch;

    const r = await authorizeSpend(
      { LEDGRRR_MODE: "http", LEDGRRR_BASE_URL: "https://ledgrrr.example" },
      { tenant: "acme", agent: "agent-1", cost: 5, ref: "idem-1" },
      stub,
    );
    expect(r).toEqual({ ok: true, budget_remaining: 42 });
    expect(seen!.url).toBe("https://ledgrrr.example/v1/authorize-spend");
    expect((seen!.init.headers as Record<string, string>)["Idempotency-Key"]).toBe("idem-1");
    expect(JSON.parse(seen!.init.body as string)).toEqual({
      tenant: "acme", agent: "agent-1", cost: 5, ref: "idem-1",
    });
  });

  it("authorizeSpend fails closed when the upstream errors", async () => {
    const stub: typeof fetch = (async () => {
      throw new Error("ECONNREFUSED");
    }) as unknown as typeof fetch;
    const r = await authorizeSpend(
      { LEDGRRR_MODE: "http", LEDGRRR_BASE_URL: "https://ledgrrr.example" },
      { tenant: "t", agent: "a", cost: 1, ref: "r" },
      stub,
    );
    expect(r.ok).toBe(false);
    expect(r.budget_remaining).toBe(0);
    expect(r.reason).toContain("unreachable");
  });

  it("authorizeSpend fails closed when a non-2xx is returned", async () => {
    const stub: typeof fetch = (async () =>
      new Response("nope", { status: 402 })) as unknown as typeof fetch;
    const r = await authorizeSpend(
      { LEDGRRR_MODE: "http", LEDGRRR_BASE_URL: "https://ledgrrr.example" },
      { tenant: "t", agent: "a", cost: 1, ref: "r" },
      stub,
    );
    expect(r).toEqual({ ok: false, budget_remaining: 0, reason: "ledgrrr 402" });
  });
});
