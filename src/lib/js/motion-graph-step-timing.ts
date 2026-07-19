import { normalizeMotionConfig } from "$lib/js/motion-config";
import { activeOrchestratorTimeline } from "$lib/js/motion-orchestrator";
import { normalizePageJsConfig } from "$lib/js/page-config";
import { applyMotionTimelineStepTiming } from "$lib/project/io";
import type {
  MotionTimelineStepTimingPatch,
  MotionTimelineStepTimingReceipt,
  PageJsConfig,
  PanaMotionTimelineItem,
} from "$lib/types";

export type MotionTimelineStepTimingProjectionInput = {
  config: PageJsConfig;
  timelineId: string;
  stepId?: string;
  stepIndex: number;
  patch: MotionTimelineStepTimingPatch;
};

export type MotionTimelineStepTimingProjection = {
  changed: boolean;
  config: PageJsConfig;
  timeline: PanaMotionTimelineItem | null;
  selectedStepId: string | null;
  receipt: MotionTimelineStepTimingReceipt;
};

export async function applyMotionTimelineStepTimingProjection(
  input: MotionTimelineStepTimingProjectionInput,
): Promise<MotionTimelineStepTimingProjection> {
  const receipt = await applyMotionTimelineStepTiming({
    config: normalizePageJsConfig(input.config),
    timelineId: input.timelineId,
    stepId: input.stepId,
    stepIndex: input.stepIndex,
    patch: input.patch,
  });
  const config = normalizePageJsConfig(receipt.afterConfig);
  const motion = normalizeMotionConfig(config.motion);

  return {
    changed: receipt.changed,
    config,
    timeline: activeOrchestratorTimeline(motion),
    selectedStepId: receipt.stepId || null,
    receipt,
  };
}
