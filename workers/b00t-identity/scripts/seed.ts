// SP1-06: idempotent seed of the first-party tenants. Run against `wrangler dev`
// or a deploy:  `pnpm exec tsx scripts/seed.ts <base-url> <admin-key>`
const BASE = process.argv[2] ?? "http://localhost:8787";
const ADMIN = process.argv[3] ?? process.env.REGISTRY_ADMIN_KEY ?? "";

const SEEDS = [
  { kind: "organizational", displayName: "PromptExecution", slug: "promptexecution", ownerAgentId: "agent/operator" },
  { kind: "organizational", displayName: "app4dog", slug: "app4dog", ownerAgentId: "agent/operator" },
];

for (const s of SEEDS) {
  const existing = await fetch(`${BASE}/tenants/${s.slug}`, {
    headers: { Authorization: `Bearer ${ADMIN}` },
  });
  if (existing.ok) {
    console.log(`ok   ${s.slug} (exists)`);
    continue;
  }
  const res = await fetch(`${BASE}/tenants`, {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${ADMIN}` },
    body: JSON.stringify(s),
  });
  console.log(`${res.ok ? "seed" : "FAIL"} ${s.slug} -> ${res.status}`);
}
