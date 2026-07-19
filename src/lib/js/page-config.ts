import type { PageJsConfig } from "$lib/types";
import { emptyMotionConfig, isMotionConfigEmpty, normalizeMotionConfig } from "$lib/js/motion-config";

export function emptyPageJsConfig(): PageJsConfig {
  return {
    version: 1,
    components: [],
    motion: emptyMotionConfig(),
  };
}

export function normalizePageJsConfig(config: Partial<PageJsConfig> | null | undefined): PageJsConfig {
  return {
    version: 1,
    components: Array.isArray(config?.components)
      ? config.components
          .map((component) => ({ id: String(component.id || "").trim() }))
          .filter((component) => component.id.length > 0)
      : [],
    motion: normalizeMotionConfig(config?.motion),
  };
}

export function clonePageJsConfig(config: Partial<PageJsConfig> | null | undefined): PageJsConfig {
  return normalizePageJsConfig(config);
}

export function isPageJsConfigEmpty(config: Partial<PageJsConfig> | null | undefined): boolean {
  const normalized = normalizePageJsConfig(config);
  return normalized.components.length === 0 && isMotionConfigEmpty(normalized.motion);
}
