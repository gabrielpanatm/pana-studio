import { createLatestWinsAsyncQueue } from "$lib/session/latest-wins-async-queue";
import type { MotionTimelineStepTimingPatch } from "$lib/types";

export type MotionStepTimingTask = {
  projectRoot: string;
  runtimeSessionId: string;
  templatePath: string;
  timelineId: string;
  stepId: string;
  stepIndex: number;
  patch: MotionTimelineStepTimingPatch;
};

export function createMotionStepTimingQueue(
  run: (
    task: MotionStepTimingTask,
    context: { isCurrent: () => boolean },
  ) => Promise<void>,
  delayMs = 60,
) {
  return createLatestWinsAsyncQueue<MotionStepTimingTask>({
    key: (task) => `${task.projectRoot}\u0000${task.runtimeSessionId}\u0000${task.templatePath}\u0000${task.timelineId}\u0000${task.stepId}`,
    delayMs,
    merge: (previous, next) => ({
      ...next,
      patch: { ...previous.patch, ...next.patch },
    }),
    run,
    onError: (error, task) => {
      console.warn("[Pana Motion] stepTiming command failed", task.stepId, error);
    },
  });
}
