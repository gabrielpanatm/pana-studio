import type {
  PanaMotionAnimationItem,
  PanaMotionConfig,
  PanaMotionItem,
  PanaMotionTimelineItem,
  PanaMotionTimelineStep,
} from "$lib/types";
import { firstTimelineItem } from "$lib/js/motion-timeline";

export function motionTimelineItems(items: PanaMotionItem[]): PanaMotionTimelineItem[] {
  return items.filter((item): item is PanaMotionTimelineItem => item.type === "timeline");
}

export function motionActorItems(items: PanaMotionItem[]): Exclude<PanaMotionItem, PanaMotionTimelineItem>[] {
  return items.filter((item): item is Exclude<PanaMotionItem, PanaMotionTimelineItem> => item.type !== "timeline");
}

export function activeOrchestratorTimeline(motion: PanaMotionConfig): PanaMotionTimelineItem | null {
  return firstTimelineItem(motion.items, motion.activeItemId);
}

export function timelineComposerItems(items: PanaMotionItem[], activeTimelineId: string | null | undefined): PanaMotionItem[] {
  return items.filter((item) => item.type !== "timeline" || item.id !== activeTimelineId);
}

export function timelineReferencedTargetIds(timelines: PanaMotionTimelineItem[]): Set<string> {
  return new Set(timelines.flatMap((timeline) => timeline.steps.map((step) => step.targetItemId).filter(Boolean)));
}

export function sanitizeOrchestratorTimeline(timeline: PanaMotionTimelineItem): PanaMotionTimelineItem {
  let changed = false;
  const steps = timeline.steps.map((step) => {
    if (step.type !== "sync" || step.targetItemId !== timeline.id) return step;
    changed = true;
    return { ...step, targetItemId: "" };
  });
  return changed ? { ...timeline, steps } : timeline;
}

export function orphanTimelineAnimations(items: PanaMotionItem[]): PanaMotionAnimationItem[] {
  const referenced = timelineReferencedTargetIds(motionTimelineItems(items));
  return motionActorItems(items).filter(
    (item): item is PanaMotionAnimationItem => item.type === "animation" && !referenced.has(item.id),
  );
}

export function replaceOrchestratorTimeline(
  motion: PanaMotionConfig,
  nextTimeline: PanaMotionTimelineItem,
  activeId: string | null | undefined = motion.activeItemId,
): PanaMotionConfig {
  const timeline = sanitizeOrchestratorTimeline(nextTimeline);
  const index = motion.items.findIndex((item) => item.id === timeline.id && item.type === "timeline");
  if (index < 0) return motion;
  return {
    ...motion,
    activeItemId: activeId ?? motion.activeItemId,
    items: motion.items.map((item, itemIndex) => itemIndex === index ? timeline : item),
  };
}

export function appendOrchestratorTimeline(motion: PanaMotionConfig, timeline: PanaMotionTimelineItem): PanaMotionConfig {
  const safeTimeline = sanitizeOrchestratorTimeline(timeline);
  return {
    ...motion,
    activeItemId: safeTimeline.id,
    items: [...motion.items, safeTimeline],
  };
}

export function timelineStepTargetItems(
  items: PanaMotionItem[],
  activeTimelineId: string | null | undefined,
  type: PanaMotionTimelineStep["type"],
): PanaMotionItem[] {
  const actors = motionActorItems(items);
  if (type === "timer") return actors.filter((item) => item.type === "timer");
  if (type === "set") {
    return actors.filter((item) => item.type === "animation" || item.type === "waapi" || item.type === "animatable");
  }
  if (type === "sync") {
    return items.filter(
      (item) =>
        item.id !== activeTimelineId &&
        (item.type === "animation" || item.type === "waapi" || item.type === "timeline"),
    );
  }
  return [];
}
