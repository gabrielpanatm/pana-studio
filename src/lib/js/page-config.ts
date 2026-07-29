import type { PageJsConfig } from "$lib/types";
import {
  isMotionDocumentEmpty,
  normalizeMotionDocument,
} from "$lib/js/motion-v2";

export function emptyPageJsConfig(): PageJsConfig {
  return {
    version: 2,
    blocks: [],
    motion: undefined,
  };
}

type LegacyPageJsConfig = Partial<PageJsConfig> & {
  components?: Array<{ id: string }>;
};

export function normalizePageJsConfig(config: LegacyPageJsConfig | null | undefined): PageJsConfig {
  const rawBlocks = Array.isArray(config?.blocks)
    ? config.blocks
    : Array.isArray(config?.components)
      ? config.components
      : [];
  return {
    version: 2,
    blocks: rawBlocks
      .map((block) => ({ id: String(block.id || "").trim() }))
      .filter((block) => block.id.length > 0),
    motion: config?.motion ? normalizeMotionDocument(config.motion) : undefined,
  };
}

export function clonePageJsConfig(config: LegacyPageJsConfig | null | undefined): PageJsConfig {
  return normalizePageJsConfig(config);
}

export function isPageJsConfigEmpty(config: LegacyPageJsConfig | null | undefined): boolean {
  const normalized = normalizePageJsConfig(config);
  return normalized.blocks.length === 0 && isMotionDocumentEmpty(normalized.motion);
}
