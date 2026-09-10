export interface JwksEnv {
  JWT_PRIVATE_KEY_PEM: string; // PKCS#8 PEM
  JWT_KID: string;
}

export interface Jwk {
  kty: "RSA";
  use: "sig";
  alg: "RS256";
  kid: string;
  n: string;
  e: string;
}

function pemToDer(pem: string): ArrayBuffer {
  const b64 = pem
    .replace(/-----BEGIN [A-Z ]+-----/g, "")
    .replace(/-----END [A-Z ]+-----/g, "")
    .replace(/\s+/g, "");
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes.buffer;
}

/** Import the PKCS#8 RSA private key as an RS256 signing key (extractable). */
export async function loadSigningKey(env: JwksEnv): Promise<CryptoKey> {
  return crypto.subtle.importKey(
    "pkcs8",
    pemToDer(env.JWT_PRIVATE_KEY_PEM),
    { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" },
    true,
    ["sign"],
  );
}

/** Public JWKS derived from the private key — only n/e are surfaced. */
export async function jwks(env: JwksEnv): Promise<{ keys: Jwk[] }> {
  const priv = await loadSigningKey(env);
  const full = (await crypto.subtle.exportKey("jwk", priv)) as JsonWebKey;
  return {
    keys: [
      {
        kty: "RSA",
        use: "sig",
        alg: "RS256",
        kid: env.JWT_KID,
        n: full.n as string,
        e: full.e as string,
      },
    ],
  };
}

export function jwksResponse(body: { keys: Jwk[] }): Response {
  return new Response(JSON.stringify(body), {
    headers: {
      "Content-Type": "application/json",
      "Cache-Control": "public, max-age=3600",
    },
  });
}
