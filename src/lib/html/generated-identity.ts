export type GeneratedHtmlIdentity = {
  className: string;
  dataAnim: string;
};

const DEFAULT_PREFIX = "ps";

function normalizeTag(tag: string) {
  const normalized = tag.trim().toLowerCase().replace(/[^a-z0-9-]+/g, "-").replace(/^-+|-+$/g, "");
  return normalized || "el";
}

function randomToken() {
  const bytes = new Uint8Array(5);
  const cryptoApi = globalThis.crypto;
  if (cryptoApi?.getRandomValues) {
    cryptoApi.getRandomValues(bytes);
  } else {
    let seed = Date.now() ^ Math.floor(Math.random() * 0xffffffff);
    for (let index = 0; index < bytes.length; index += 1) {
      seed = Math.imul(seed ^ (seed >>> 15), 2246822519);
      bytes[index] = seed & 0xff;
    }
  }

  let value = 0n;
  for (const byte of bytes) {
    value = (value << 8n) + BigInt(byte);
  }
  return value.toString(36).padStart(8, "0").slice(0, 8);
}

function identityExists(candidate: string, sourceTexts: string[]) {
  return sourceTexts.some((source) => source.includes(candidate));
}

export function generateUniqueHtmlIdentity(
  tag: string,
  sourceTexts: string[],
  options: { prefix?: string; maxAttempts?: number } = {},
): GeneratedHtmlIdentity {
  const prefix = options.prefix?.trim() || DEFAULT_PREFIX;
  const normalizedTag = normalizeTag(tag);
  const maxAttempts = options.maxAttempts ?? 80;

  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    const candidate = `${prefix}-${normalizedTag}-${randomToken()}`;
    if (!identityExists(candidate, sourceTexts)) {
      return {
        className: candidate,
        dataAnim: candidate,
      };
    }
  }

  const fallback = `${prefix}-${normalizedTag}-${Date.now().toString(36)}-${randomToken()}`;
  return {
    className: fallback,
    dataAnim: fallback,
  };
}
