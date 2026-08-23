import type { PageJsConfig } from "$lib/js/contracts";
import { normalizeMotionDocument } from "$lib/js/motion-v2";

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
