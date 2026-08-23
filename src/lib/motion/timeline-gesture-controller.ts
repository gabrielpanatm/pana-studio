import type { MotionAction, MotionInteraction } from "$lib/js/contracts";
import {
  startPointerSession,
  type PointerCaptureTarget,
  type PointerSession,
  type PointerSessionCancelReason,
  type PointerSessionEnvironment,
} from "$lib/ui/pointer-session";

export type MotionTimelineTimingDraft = {
  start: number;
  duration: number;
};

export type MotionTimelineTimingCommit = {
  interactionId: string;
  actionId: string;
  start: number;
  duration?: number;
};

export type MotionTimelineGestureHost = {
  selectedInteractionId: () => string | null;
  selectAction: (interactionId: string, actionId: string) => void;
  setTimingDraft: (actionId: string, draft: MotionTimelineTimingDraft | null) => void;
  setPlayhead: (value: number) => void;
  requestSeek: (interactionId: string, value: number) => void;
  commitTiming: (commit: MotionTimelineTimingCommit) => Promise<unknown>;
  setGestureActive: (gesture: "drag" | "seek", active: boolean) => void;
};

export type BeginMotionActionDrag = {
  event: PointerEvent;
  captureTarget: PointerCaptureTarget;
  action: MotionAction;
  interactionId: string;
  domain: MotionInteraction["domain"];
  mode: "move" | "resize";
  canvasWidth: number;
  timelineDuration: number;
  snap: boolean;
  original: MotionTimelineTimingDraft;
  safetyTimeoutMs?: number;
};

export type BeginMotionSeek = {
  event: PointerEvent;
  captureTarget: PointerCaptureTarget;
  interactionId: string;
  domain: MotionInteraction["domain"];
  canvasLeft: number;
  canvasWidth: number;
  timelineDuration: number;
  snap: boolean;
  safetyTimeoutMs?: number;
};

type PendingTimingCommit = {
  serial: number;
  draft: MotionTimelineTimingDraft;
};

type ActionDragState = {
  action: MotionAction;
  interactionId: string;
  domain: MotionInteraction["domain"];
  mode: "move" | "resize";
  startX: number;
  latestX: number;
  canvasWidth: number;
  timelineDuration: number;
  snap: boolean;
  original: MotionTimelineTimingDraft;
  session: PointerSession;
};

type SeekState = {
  interactionId: string;
  domain: MotionInteraction["domain"];
  latestX: number;
  canvasLeft: number;
  canvasWidth: number;
  timelineDuration: number;
  snap: boolean;
  lastPublishedValue: number | null;
  session: PointerSession;
};

export class MotionTimelineGestureController {
  private readonly host: MotionTimelineGestureHost;
  private readonly environment?: PointerSessionEnvironment;
  private actionDrag: ActionDragState | null = null;
  private seek: SeekState | null = null;
  private timingCommitSerial = 0;
  private readonly pendingTimingCommits = new Map<string, PendingTimingCommit>();

  constructor(host: MotionTimelineGestureHost, environment?: PointerSessionEnvironment) {
    this.host = host;
    this.environment = environment;
  }

  get dragging() {
    return this.actionDrag !== null;
  }

  get seeking() {
    return this.seek !== null;
  }

  isSeekingInteraction(interactionId: string) {
    return this.seek?.interactionId === interactionId;
  }

  beginActionDrag(options: BeginMotionActionDrag): boolean {
    if (options.event.button !== 0 || this.seek) return false;
    this.cancelActionDrag("replaced");
    options.event.preventDefault();
    options.event.stopPropagation();
    this.host.selectAction(options.interactionId, options.action.id);

    const state = {
      action: options.action,
      interactionId: options.interactionId,
      domain: options.domain,
      mode: options.mode,
      startX: options.event.clientX,
      latestX: options.event.clientX,
      canvasWidth: options.canvasWidth,
      timelineDuration: options.timelineDuration,
      snap: options.snap,
      original: options.original,
      session: null as unknown as PointerSession,
    } satisfies ActionDragState;
    this.actionDrag = state;
    this.host.setGestureActive("drag", true);
    state.session = startPointerSession({
      pointerId: options.event.pointerId,
      captureTarget: options.captureTarget,
      safetyTimeoutMs: options.safetyTimeoutMs ?? 8_000,
      environment: this.environment,
      onMove: (event, session) => {
        if (this.actionDrag !== state) return;
        event.preventDefault();
        state.latestX = event.clientX;
        session.requestFrame(() => this.publishActionDraft(state));
      },
      onCommit: (event, session) => this.commitActionDrag(state, event, session),
      onCancel: () => this.cancelActionDragState(state),
    });
    return true;
  }

  beginSeek(options: BeginMotionSeek): boolean {
    if (options.event.button !== 0 || this.actionDrag || this.seek) return false;
    options.event.preventDefault();
    const state = {
      interactionId: options.interactionId,
      domain: options.domain,
      latestX: options.event.clientX,
      canvasLeft: options.canvasLeft,
      canvasWidth: options.canvasWidth,
      timelineDuration: options.timelineDuration,
      snap: options.snap,
      lastPublishedValue: null,
      session: null as unknown as PointerSession,
    } satisfies SeekState;
    this.seek = state;
    this.host.setGestureActive("seek", true);
    state.session = startPointerSession({
      pointerId: options.event.pointerId,
      captureTarget: options.captureTarget,
      safetyTimeoutMs: options.safetyTimeoutMs ?? 15_000,
      environment: this.environment,
      onMove: (event, session) => {
        if (this.seek !== state) return;
        event.preventDefault();
        state.latestX = event.clientX;
        session.requestFrame(() => this.publishSeek(state));
      },
      onCommit: (event, session) => {
        if (this.seek !== state) return;
        if (event) state.latestX = event.clientX;
        session.flushFrame();
        this.publishSeek(state);
        this.seek = null;
        this.host.setGestureActive("seek", false);
      },
      onCancel: () => {
        if (this.seek !== state) return;
        this.seek = null;
        this.host.setGestureActive("seek", false);
      },
    });
    this.publishSeek(state);
    return true;
  }

  cancelActionDrag(reason: PointerSessionCancelReason = "programmatic"): boolean {
    return this.actionDrag?.session.cancel(reason) ?? false;
  }

  cancelSeek(reason: PointerSessionCancelReason = "programmatic"): boolean {
    return this.seek?.session.cancel(reason) ?? false;
  }

  reconcileInteraction(interactionId: string | null) {
    if (this.actionDrag && this.actionDrag.interactionId !== interactionId) {
      this.cancelActionDrag("identity-change");
    }
    if (this.seek && this.seek.interactionId !== interactionId) {
      this.cancelSeek("identity-change");
    }
  }

  destroy() {
    this.actionDrag?.session.destroy();
    this.seek?.session.destroy();
  }

  private actionTimingAt(state: ActionDragState, clientX: number): MotionTimelineTimingDraft {
    const delta = (clientX - state.startX)
      / Math.max(1, state.canvasWidth)
      * state.timelineDuration;
    const snap = (value: number) => {
      if (!state.snap) return Math.max(0, value);
      const step = state.domain === "progress" ? 1 : 50;
      return Math.max(0, Math.round(value / step) * step);
    };
    const next = { ...state.original };
    if (state.mode === "move") {
      next.start = snap(
        state.domain === "progress"
          ? Math.min(100 - Math.max(0, next.duration), state.original.start + delta)
          : state.original.start + delta,
      );
    } else {
      next.duration = snap(Math.max(
        state.domain === "progress" ? 1 : 50,
        state.domain === "progress"
          ? Math.min(100 - next.start, state.original.duration + delta)
          : state.original.duration + delta,
      ));
    }
    return next;
  }

  private publishActionDraft(state: ActionDragState) {
    if (this.actionDrag !== state) return;
    this.host.setTimingDraft(
      state.action.id,
      this.actionTimingAt(state, state.latestX),
    );
  }

  private commitActionDrag(
    state: ActionDragState,
    event: PointerEvent | null,
    session: PointerSession,
  ) {
    if (this.actionDrag !== state) return;
    if (event) state.latestX = event.clientX;
    session.flushFrame();
    const next = this.actionTimingAt(state, state.latestX);
    this.actionDrag = null;
    this.host.setGestureActive("drag", false);
    if (this.host.selectedInteractionId() !== state.interactionId) {
      this.restorePendingTimingDraft(state.action.id);
      return;
    }

    const serial = ++this.timingCommitSerial;
    this.pendingTimingCommits.set(state.action.id, { serial, draft: next });
    this.host.setTimingDraft(state.action.id, next);
    let mutation: Promise<unknown>;
    try {
      mutation = this.host.commitTiming({
        interactionId: state.interactionId,
        actionId: state.action.id,
        start: next.start,
        ...(state.action.type === "animate" || state.action.type === "nested"
          ? { duration: next.duration }
          : {}),
      });
    } catch (error) {
      this.settleTimingCommit(state.action.id, serial);
      throw error;
    }
    void mutation.then(
      () => this.settleTimingCommit(state.action.id, serial),
      () => this.settleTimingCommit(state.action.id, serial),
    );
  }

  private cancelActionDragState(state: ActionDragState) {
    if (this.actionDrag !== state) return;
    this.actionDrag = null;
    this.host.setGestureActive("drag", false);
    this.restorePendingTimingDraft(state.action.id);
  }

  private restorePendingTimingDraft(actionId: string) {
    this.host.setTimingDraft(
      actionId,
      this.pendingTimingCommits.get(actionId)?.draft ?? null,
    );
  }

  private settleTimingCommit(actionId: string, serial: number) {
    if (this.pendingTimingCommits.get(actionId)?.serial !== serial) return;
    this.pendingTimingCommits.delete(actionId);
    if (this.actionDrag?.action.id !== actionId) {
      this.host.setTimingDraft(actionId, null);
    }
  }

  private seekValueAt(state: SeekState, clientX: number) {
    const raw = (clientX - state.canvasLeft)
      / Math.max(1, state.canvasWidth)
      * state.timelineDuration;
    const bounded = Math.max(0, Math.min(state.timelineDuration, raw));
    if (!state.snap) return bounded;
    const step = state.domain === "progress" ? 1 : 50;
    return Math.max(
      0,
      Math.min(state.timelineDuration, Math.round(bounded / step) * step),
    );
  }

  private publishSeek(state: SeekState) {
    if (this.seek !== state) return;
    const next = this.seekValueAt(state, state.latestX);
    this.host.setPlayhead(next);
    if (state.lastPublishedValue === next) return;
    state.lastPublishedValue = next;
    this.host.requestSeek(state.interactionId, next);
  }
}
