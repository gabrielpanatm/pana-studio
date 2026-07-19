import { normalizeMotionConfig } from "$lib/js/motion-config";
import { replaceOrchestratorTimeline } from "$lib/js/motion-orchestrator";
import { normalizePageJsConfig } from "$lib/js/page-config";
import type { PageJsConfig, PanaMotionTimelineItem } from "$lib/types";

export function pageConfigWithTimeline(
  config: PageJsConfig,
  timeline: PanaMotionTimelineItem,
  activeId: string | null | undefined = undefined,
): PageJsConfig {
  const motion = normalizeMotionConfig(config.motion);
  return normalizePageJsConfig({
    ...config,
    version: 1,
    motion: replaceOrchestratorTimeline(motion, timeline, activeId),
  });
}
