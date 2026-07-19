import type {
  PanaMotionAnimationItem,
  PanaMotionItem,
  PanaMotionTimelineItem,
  PanaMotionTimelineStep,
  PanaMotionTimelineTrack,
} from "$lib/types";

export const MOTION_TIMELINE_PX_PER_MS = 0.12;
export const MOTION_TIMELINE_DEFAULT_MS = 10_000;
export const MOTION_TIMELINE_MAX_VISUAL_MS = 120_000;
export const MOTION_TIMELINE_MAX_TICKS = 320;
export const MOTION_TIMELINE_MIN_DURATION_MS = 50;
export const MOTION_TIMELINE_STEP_MS = 50;
export const MOTION_TIMELINE_DEFAULT_TRACK_ID = "track-main";
export const MOTION_TIMELINE_ROW_HEIGHT = 28;
export const MOTION_TIMELINE_LANE_VERTICAL_PADDING = 10;

export type MotionTimelineClip = {
  step: PanaMotionTimelineStep;
  stepIndex: number;
  laneIndex: number;
  targetItem: PanaMotionItem | null;
  targetItemIndex: number | null;
  id: string;
  type: PanaMotionTimelineStep["type"];
  lane: string;
  rowIndex: number;
  label: string;
  trigger: string;
  startMs: number;
  durationMs: number;
  endMs: number;
  propsLabel: string;
};

export type MotionTimelineLane = {
  id: string;
  label: string;
  color: string;
  collapsed: boolean;
  height: number;
  rowCount: number;
  clips: MotionTimelineClip[];
};

export type MotionTimelineModel = {
  lanes: MotionTimelineLane[];
  labels: Array<{ id: string; name: string; ms: number }>;
  maxMs: number;
  widthPx: number;
  ticks: Array<{ ms: number; major: boolean; label: string }>;
};

function safeMs(value: number | null | undefined, fallback = 0): number {
  return Number.isFinite(value) ? Math.max(0, Math.round(Number(value))) : fallback;
}

export function quantizeMotionMs(value: number, step = MOTION_TIMELINE_STEP_MS): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.round(value / step) * step);
}

export function formatMotionTimelinePosition(ms: number): string {
  return String(quantizeMotionMs(ms));
}

function parseAbsoluteMotionTimelinePosition(value: string | number | null | undefined): number | null {
  if (typeof value === "number") return safeMs(value, 0);
  const text = String(value ?? "").trim();
  if (!text) return null;
  const seconds = text.match(/^(-?\d+(?:\.\d+)?)s$/);
  if (seconds) return safeMs(Number(seconds[1]) * 1000, 0);
  const ms = text.match(/^(-?\d+(?:\.\d+)?)ms$/);
  if (ms) return safeMs(Number(ms[1]), 0);
  const raw = Number(text);
  return Number.isFinite(raw) ? safeMs(raw, 0) : null;
}

function parseMotionTimelineOffset(value: string | null | undefined): number | null {
  const absolute = parseAbsoluteMotionTimelinePosition(value);
  return absolute === null ? null : absolute;
}

function applySignedOffset(base: number, sign: string | null | undefined, offset: string | null | undefined): number {
  if (!sign) return safeMs(base, 0);
  const parsedOffset = parseMotionTimelineOffset(offset) ?? 0;
  return safeMs(sign === "-=" ? base - parsedOffset : base + parsedOffset, 0);
}

export function parseMotionTimelinePosition(value: string | number | null | undefined, fallback = 0): number {
  const absolute = parseAbsoluteMotionTimelinePosition(value);
  return absolute === null ? fallback : absolute;
}

export function resolveMotionTimelinePosition(
  value: string | number | null | undefined,
  context: {
    previousStart: number;
    previousEnd: number;
    labels: Map<string, number>;
  },
  fallback = 0,
): number {
  const absolute = parseAbsoluteMotionTimelinePosition(value);
  if (absolute !== null) return absolute;
  const text = String(value ?? "").trim();
  if (!text) return fallback;

  const previousRelative = text.match(/^([<>])(?:(\+=|-=)(.+))?$/);
  if (previousRelative) {
    const base = previousRelative[1] === "<" ? context.previousStart : context.previousEnd;
    return applySignedOffset(base, previousRelative[2], previousRelative[3]);
  }

  const endRelative = text.match(/^(\+=|-=)(.+)$/);
  if (endRelative) {
    return applySignedOffset(context.previousEnd, endRelative[1], endRelative[2]);
  }

  const labelEntries = Array.from(context.labels.entries()).sort((left, right) => right[0].length - left[0].length);
  for (const [label, ms] of labelEntries) {
    if (text === label) return safeMs(ms, fallback);
    if (text.startsWith(`${label}+=`)) return applySignedOffset(ms, "+=", text.slice(label.length + 2));
    if (text.startsWith(`${label}-=`)) return applySignedOffset(ms, "-=", text.slice(label.length + 2));
  }

  return fallback;
}

function targetLabel(item: PanaMotionItem | null, step: PanaMotionTimelineStep): string {
  if (step.type === "callback") return "Callback";
  if (step.type === "label") return "Label";
  if (!item) return step.type;
  if (item.type === "animation") {
    if (item.target.dataAnim) return item.target.dataAnim;
    if (item.target.selector) return item.target.selector;
  }
  return item.name || item.type;
}

function stepLabel(item: PanaMotionItem | null, step: PanaMotionTimelineStep): string {
  if (step.label) return step.label;
  if (item) return item.name || item.type;
  return step.type;
}

function stepDuration(item: PanaMotionItem | null, step: PanaMotionTimelineStep): number {
  if (step.duration > 0) return Math.max(MOTION_TIMELINE_MIN_DURATION_MS, safeMs(step.duration));
  if (item?.type === "animation") return Math.max(MOTION_TIMELINE_MIN_DURATION_MS, safeMs(item.playback.duration, 600));
  if (item?.type === "timer") return Math.max(MOTION_TIMELINE_MIN_DURATION_MS, safeMs(item.playback.duration, 600));
  if (item?.type === "waapi") return Math.max(MOTION_TIMELINE_MIN_DURATION_MS, safeMs(item.playback.duration, 600));
  if (step.type === "callback" || step.type === "label" || step.type === "set") return MOTION_TIMELINE_MIN_DURATION_MS;
  return 600;
}

function stepTrigger(item: PanaMotionItem | null, step: PanaMotionTimelineStep): string {
  if (item?.type === "animation") return item.trigger || "load";
  return step.type;
}

function propsLabel(item: PanaMotionItem | null): string {
  if (item?.type === "animation" || item?.type === "waapi" || item?.type === "animatable") {
    return item.properties.map((prop) => prop.property).join(", ");
  }
  return "";
}

function timelineTrackFallback(index = 0): PanaMotionTimelineTrack {
  return {
    id: index === 0 ? MOTION_TIMELINE_DEFAULT_TRACK_ID : `track-${index + 1}`,
    name: index === 0 ? "Principal" : `Track ${index + 1}`,
    collapsed: false,
    height: 38,
    color: "#168a72",
  };
}

function laneFromTrack(track: PanaMotionTimelineTrack): MotionTimelineLane {
  return {
    id: track.id,
    label: track.name || track.id,
    color: track.color || "#168a72",
    collapsed: Boolean(track.collapsed),
    height: Math.max(26, safeMs(track.height, 38)),
    rowCount: 1,
    clips: [],
  };
}

function visualRowKey(clip: MotionTimelineClip): string {
  return clip.targetItem?.id || clip.step.targetItemId || `${clip.type}:${clip.label}`;
}

function laneHeightForRows(rowCount: number, fallback: number): number {
  return Math.max(fallback, rowCount * MOTION_TIMELINE_ROW_HEIGHT + MOTION_TIMELINE_LANE_VERTICAL_PADDING);
}

function assignClipRows(clips: MotionTimelineClip[]): { clips: MotionTimelineClip[]; rowCount: number } {
  if (clips.length === 0) return { clips: [], rowCount: 1 };
  const rowKeys: string[] = [];
  const rowEnds: number[] = [];
  const rowByClipId = new Map<string, number>();
  const sorted = [...clips].sort((left, right) => {
    if (left.startMs !== right.startMs) return left.startMs - right.startMs;
    if (left.endMs !== right.endMs) return left.endMs - right.endMs;
    return left.id.localeCompare(right.id);
  });

  for (const clip of sorted) {
    const key = visualRowKey(clip);
    let row = rowKeys.findIndex((rowKey, index) => rowKey === key && clip.startMs >= rowEnds[index]);
    if (row < 0) {
      row = rowKeys.length;
      rowKeys.push(key);
    }
    rowEnds[row] = Math.max(rowEnds[row] ?? 0, clip.endMs);
    rowByClipId.set(clip.id, row);
  }

  const rowCount = Math.max(1, rowKeys.length);
  return {
    rowCount,
    clips: clips.map((clip) => ({ ...clip, rowIndex: rowByClipId.get(clip.id) ?? 0 })),
  };
}

function timelineLabelMap(timeline: PanaMotionTimelineItem | null): {
  labels: Array<{ id: string; name: string; ms: number }>;
  map: Map<string, number>;
} {
  const labels: Array<{ id: string; name: string; ms: number }> = [];
  const map = new Map<string, number>();
  let previousStart = 0;
  let previousEnd = 0;
  for (const label of timeline?.labels ?? []) {
    const ms = resolveMotionTimelinePosition(label.position, { previousStart, previousEnd, labels: map }, previousEnd);
    labels.push({ id: label.id, name: label.name, ms });
    if (label.name) map.set(label.name, ms);
    previousStart = ms;
    previousEnd = ms;
  }
  return { labels, map };
}

export function buildMotionTimelineComposer(
  timeline: PanaMotionTimelineItem | null,
  items: PanaMotionItem[],
): MotionTimelineModel {
  const lanesById = new Map<string, MotionTimelineLane>();
  let maxEnd = safeMs(timeline?.duration, MOTION_TIMELINE_DEFAULT_MS);
  const tracks = timeline?.tracks?.length ? timeline.tracks : [timelineTrackFallback()];

  for (const [index, track] of tracks.entries()) {
    const normalizedTrack = {
      ...timelineTrackFallback(index),
      ...track,
    };
    lanesById.set(normalizedTrack.id, laneFromTrack(normalizedTrack));
  }

  const itemIndexById = new Map(items.map((item, index) => [item.id, index]));
  const itemById = new Map(items.map((item) => [item.id, item]));
  const labelState = timelineLabelMap(timeline);
  let previousStart = 0;
  let previousEnd = 0;

  for (const [stepIndex, step] of (timeline?.steps ?? []).entries()) {
    const targetItem = step.targetItemId ? itemById.get(step.targetItemId) ?? null : null;
    const laneId = step.lane || tracks[0]?.id || MOTION_TIMELINE_DEFAULT_TRACK_ID;
    const lane = lanesById.get(laneId) ?? {
      id: laneId,
      label: laneId,
      color: "#168a72",
      collapsed: false,
      height: 38,
      rowCount: 1,
      clips: [] as MotionTimelineClip[],
    };
    const startMs = resolveMotionTimelinePosition(step.position, {
      previousStart,
      previousEnd,
      labels: labelState.map,
    }, previousEnd);
    const durationMs = stepDuration(targetItem, step);
    const endMs = startMs + durationMs;
    maxEnd = Math.max(maxEnd, endMs + 500);
    lane.clips.push({
      step,
      stepIndex,
      laneIndex: lanesById.size,
      targetItem,
      targetItemIndex: step.targetItemId ? itemIndexById.get(step.targetItemId) ?? null : null,
      id: step.id,
      type: step.type,
      lane: laneId,
      rowIndex: 0,
      label: stepLabel(targetItem, step),
      trigger: stepTrigger(targetItem, step),
      startMs,
      durationMs,
      endMs,
      propsLabel: propsLabel(targetItem),
    });
    lanesById.set(laneId, lane);
    previousStart = startMs;
    previousEnd = endMs;
  }

  const labels = labelState.labels;

  for (const label of labels) {
    maxEnd = Math.max(maxEnd, label.ms + 500);
  }

  const maxMs = Math.min(MOTION_TIMELINE_MAX_VISUAL_MS, Math.max(
    MOTION_TIMELINE_DEFAULT_MS,
    Math.ceil(maxEnd / 500) * 500,
  ));

  return {
    lanes: Array.from(lanesById.values()).map((lane, laneIndex) => {
      if (lane.collapsed) {
        return {
          ...lane,
          rowCount: 1,
          clips: [],
        };
      }
      const packed = assignClipRows(lane.clips);
      return {
        ...lane,
        rowCount: packed.rowCount,
        height: laneHeightForRows(packed.rowCount, lane.height),
        clips: packed.clips.map((clip) => ({ ...clip, laneIndex })),
      };
    }),
    labels,
    maxMs,
    widthPx: Math.max(720, Math.ceil(maxMs * MOTION_TIMELINE_PX_PER_MS)),
    ticks: buildTimelineTicks(maxMs),
  };
}

export function firstTimelineItem(items: PanaMotionItem[], activeItemId: string | null): PanaMotionTimelineItem | null {
  const active = items.find((item) => item.id === activeItemId && item.type === "timeline");
  if (active?.type === "timeline") return active;
  const parent = items.find((item) => item.type === "timeline" && item.steps.some((step) => step.id === activeItemId));
  if (parent?.type === "timeline") return parent;
  return (items.find((item) => item.type === "timeline") as PanaMotionTimelineItem | undefined) ?? null;
}

export function timelineStepFromAnimation(animation: PanaMotionAnimationItem, index: number): PanaMotionTimelineStep {
  return {
    id: `step-${Math.random().toString(36).slice(2, 9)}`,
    type: "animation",
    label: animation.name || `Animation ${index + 1}`,
    position: "0",
    duration: safeMs(animation.playback.duration, 600),
    lane: MOTION_TIMELINE_DEFAULT_TRACK_ID,
    targetItemId: animation.id,
    callback: { enabled: false, label: "Timeline callback", code: "" },
  };
}

export function timelineStepFromItem(
  item: PanaMotionItem | null,
  type: PanaMotionTimelineStep["type"],
  position: string,
  index: number,
): PanaMotionTimelineStep {
  const label = item?.name || `${type} ${index + 1}`;
  const duration = item?.type === "animation" || item?.type === "timer" || item?.type === "waapi"
    ? safeMs(item.playback.duration, 600)
    : type === "callback" || type === "set" || type === "sync" || type === "label"
      ? MOTION_TIMELINE_MIN_DURATION_MS
      : 600;
  return {
    id: `step-${Math.random().toString(36).slice(2, 9)}`,
    type,
    label,
    position,
    duration,
    lane: MOTION_TIMELINE_DEFAULT_TRACK_ID,
    targetItemId: item?.id ?? "",
    callback: { enabled: false, label: "Timeline callback", code: "" },
  };
}

function timelineTickStep(maxMs: number): number {
  const steps = [250, 500, 1_000, 2_000, 5_000, 10_000, 15_000, 30_000];
  return steps.find((step) => Math.ceil(maxMs / step) <= MOTION_TIMELINE_MAX_TICKS) ?? 30_000;
}

function timelineMajorStep(maxMs: number, tickStep: number): number {
  if (maxMs <= 10_000) return 1_000;
  if (maxMs <= 60_000) return Math.max(5_000, tickStep);
  return Math.max(10_000, tickStep);
}

function buildTimelineTicks(maxMs: number): Array<{ ms: number; major: boolean; label: string }> {
  const ticks: Array<{ ms: number; major: boolean; label: string }> = [];
  const tickStep = timelineTickStep(maxMs);
  const majorStep = timelineMajorStep(maxMs, tickStep);
  for (let ms = 0; ms <= maxMs; ms += tickStep) {
    const major = ms % majorStep === 0;
    ticks.push({
      ms,
      major,
      label: major ? `${ms / 1000}s` : "",
    });
  }
  return ticks;
}
