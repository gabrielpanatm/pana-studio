import type { PageJsConfig } from "$lib/types";
import { emptyMotionConfig, isMotionConfigEmpty, normalizeMotionConfig } from "$lib/js/motion-config";

export function emptyPageJsConfig(): PageJsConfig {
  return {
    version: 1,
    blocks: [],
    motion: emptyMotionConfig(),
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
    version: 1,
    blocks: rawBlocks
      .map((block) => ({ id: String(block.id || "").trim() }))
      .filter((block) => block.id.length > 0),
    motion: normalizeMotionConfig(config?.motion),
  };
}

export function clonePageJsConfig(config: LegacyPageJsConfig | null | undefined): PageJsConfig {
  return normalizePageJsConfig(config);
}

export function isPageJsConfigEmpty(config: LegacyPageJsConfig | null | undefined): boolean {
  const normalized = normalizePageJsConfig(config);
  return normalized.blocks.length === 0 && isMotionConfigEmpty(normalized.motion);
}
