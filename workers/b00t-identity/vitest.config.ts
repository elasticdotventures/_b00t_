import { defineWorkersConfig } from "@cloudflare/vitest-pool-workers/config";

// Test-only RSA key + kid for the RS256 token/JWKS suites (SP1-01/SP1-02).
// This key is a throwaway fixture — it signs nothing outside `pnpm test`.
const TEST_JWT_PRIVATE_KEY_PEM =
  "-----BEGIN PRIVATE KEY-----\nMIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCRQTcE2IGnzryV\nuKDvf7dJ3FuJSXwBwvXSLWUUOyh/O2rl8D9qM4Q+KMT8l+udqj0Utal95mVXAVHd\n+vg5jCEVGnkhE3UZpQShuWwGmAYlgYFCtkPYFc3qD+L2HJtxet6RZFaaeCmV8FO+\nbPStmPbLYXY6dFECG8mYBTITpFYBtLIiLS4S3Qsw0YArrR4CDgdlpCg41wI0z5uX\n/aGkWKhlpxt8lyr3jaUCsi260vb1xS/2zdM4HJ/LDIMljJICac6ugUyoRAZKYtc8\nvTc3xXHpEcxkafPtyOzG3Fl3lFMsgxRRPEgfn1FKzZ+RExTHDsjOpc9Xi4ihZWQn\nySXqmR4hAgMBAAECggEAESXq0eahf+cXOnG+hifEwrKCF/YV7rtOfA6h5T6KrGKe\nXyD6y5XjYdc8Ujm5NjbX2S8NIHDnu9rLCHLNhTW23h/u9umuJGXn4xPZ3flqmFju\noqqT3dnNInnXqIh+DWqdBfsbgkb3Wd0ydcO1Kx1o3V/XLlV3DtGq/gh2/fyjrrWx\n12Db2m5IlhEfbMz2Rc2XqqH9BsI/mVhBmbxPEFys9Q6JyMV/qZvgQ4lc+aYz51/5\nbxZruYfMKsUOJXg0Aa12M+Ud+4kJ9gvMCcUjPhj5ww7ZkP7972Zg5a8D14iJjXKN\na1qrW+qSJhB/JUe+DK4LW4LO7Fhv7xuwlMccPW05sQKBgQDHdJIcPGaRGQOMAs7k\ncKCtjwnum4BqJu67juH1tWviXShToQSHnauwgRxQrGEOxRyQipHOXVMvghvG9hdk\nF4souiVYCryeQaskpeAn2crnnU2FrF9+og6fYiet7YDlvSeJIeLvy0IA2ji2wD5u\nLY/GuQTNTG9k1k/LPBWBiAD3HQKBgQC6bwv1G6UQXbILY8jZBphqaGYGGvagq6VJ\nMFHxMmZC4dUULlDg25GfSqOT9xRAFcuiy7KLp+Fv7249YC83UX4RrS1F0v9/n3C4\nuZEXEZra5YnSlgIW2bJq59Xfb5fXg81nMEXEPhDQK6vueyuPuNO+oh00f0TlCnW6\nQfjDCisf1QKBgHQvU21fQeAD0i0c9afcc7ymNgLoUkWDqE1ZTgbzR4T0/yi4Awt8\nrSaEDxpvT5pq99i633R2qJ5kDAo6ECYeENIInPhMSNNnLWqLtaeBFtEUsLPNVVNO\n03XEl5iZYRxyszUOqENHA4u7ko3iLnu/zqDT5hgxDjKPJKwes+hgcS+BAoGAEo/N\n0/iNpaR+fo3PyHPUpvt/9OmoVnTgfvn1npsS/WO4sEqwOMMDq6Vlxeyasoq4/Jtl\nSmxLkLZ49llmOg6+C4p/cG1CjPVV5r5rCK3zCgpCf5n52UaRcf1lGNrmdkmkILr4\np0I6sE84zgSrYKLZSiif2cM2G8u/zuyUlO6lPoUCgYBscEwq4AC0t5eHARTpCD92\nFKYbb0W9EL9i0ZwxQ0vJKdbLtMzgANAuFzp6+4807Jxnxr5HKuwc20OBTJ4C40st\n2+HLb/1FtvcEjMeEhG15wOyZfMCwJ4tyeitun2iAuGQfTfMRbye0Y2u4k9ZahJuq\nSwxvnEFXx9VJ2QOvD4beOw==\n-----END PRIVATE KEY-----\n";

export default defineWorkersConfig({
  test: {
    poolOptions: {
      workers: {
        wrangler: { configPath: "./wrangler.jsonc" },
        miniflare: {
          bindings: {
            REGISTRY_ADMIN_KEY: "test-admin-key",
            JWT_KID: "test-kid",
            JWT_PRIVATE_KEY_PEM: TEST_JWT_PRIVATE_KEY_PEM,
          },
        },
      },
    },
  },
});
