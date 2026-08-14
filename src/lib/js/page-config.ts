import type { PageJsConfig } from "$lib/types";
import {
  isMotionDocumentEmpty,
  normalizeMotionDocument,
} from "$lib/js/motion-v2";

export function emptyPageJsConfig(): PageJsConfig {
  return {
    motion: undefined,
  };
}

export function normalizePageJsConfig(config: Partial<PageJsConfig> | null | undefined): PageJsConfig {
  return {
    motion: config?.motion ? normalizeMotionDocument(config.motion) : undefined,
  };
}

export function clonePageJsConfig(config: Partial<PageJsConfig> | null | undefined): PageJsConfig {
  return normalizePageJsConfig(config);
}

export function isPageJsConfigEmpty(config: Partial<PageJsConfig> | null | undefined): boolean {
  const normalized = normalizePageJsConfig(config);
  return isMotionDocumentEmpty(normalized.motion);
}
