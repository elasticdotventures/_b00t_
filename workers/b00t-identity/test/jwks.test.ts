import { describe, it, expect } from "vitest";
import { jwks, loadSigningKey } from "../src/jwks";

async function generateTestPem(): Promise<string> {
  const pair = await crypto.subtle.generateKey(
    {
      name: "RSASSA-PKCS1-v1_5",
      modulusLength: 2048,
      publicExponent: new Uint8Array([1, 0, 1]),
      hash: "SHA-256",
    },
    true,
    ["sign", "verify"],
  );
  const der = await crypto.subtle.exportKey("pkcs8", pair.privateKey);
  const b64 = btoa(String.fromCharCode(...new Uint8Array(der)));
  const lines = b64.match(/.{1,64}/g)!.join("\n");
  return `-----BEGIN PRIVATE KEY-----\n${lines}\n-----END PRIVATE KEY-----\n`;
}

describe("jwks", () => {
  it("produces a well-formed RS256 public JWK set", async () => {
    const env = { JWT_PRIVATE_KEY_PEM: await generateTestPem(), JWT_KID: "test-kid-1" };
    const set = await jwks(env);
    expect(set.keys).toHaveLength(1);
    const k = set.keys[0];
    expect(k.kty).toBe("RSA");
    expect(k.use).toBe("sig");
    expect(k.alg).toBe("RS256");
    expect(k.kid).toBe("test-kid-1");
    expect(typeof k.n).toBe("string");
    expect(k.n.length).toBeGreaterThan(100);
    expect(k.e).toBe("AQAB");
    // no private material leaked
    expect((k as Record<string, unknown>).d).toBeUndefined();
  });

  it("loadSigningKey imports the PEM as a usable signing key", async () => {
    const env = { JWT_PRIVATE_KEY_PEM: await generateTestPem(), JWT_KID: "k" };
    const key = await loadSigningKey(env);
    expect(key.type).toBe("private");
    const sig = await crypto.subtle.sign("RSASSA-PKCS1-v1_5", key, new TextEncoder().encode("hi"));
    expect(sig.byteLength).toBe(256);
  });
});
