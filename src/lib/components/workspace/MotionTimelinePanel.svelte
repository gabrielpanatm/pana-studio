<script lang="ts">
  import { untrack } from "svelte";
  import MotionTimeline from "$lib/components/inspector/js/MotionTimeline.svelte";
  import { createMotionItem, emptyExpression, normalizeMotionConfig } from "$lib/js/motion-config";
  import { emptyPageJsConfig, normalizePageJsConfig } from "$lib/js/page-config";
  import {
    activeOrchestratorTimeline,
    appendOrchestratorTimeline,
    motionActorItems,
    orphanTimelineAnimations,
    replaceOrchestratorTimeline,
    timelineComposerItems,
    timelineStepTargetItems,
  } from "$lib/js/motion-orchestrator";
  import {
    MOTION_TIMELINE_DEFAULT_TRACK_ID,
    timelineStepFromItem,
    timelineStepFromAnimation,
  } from "$lib/js/motion-timeline";
  import { pageConfigWithTimeline } from "$lib/js/motion-timeline-draft";
  import { applyMotionTimelineStepTimingProjection } from "$lib/js/motion-graph-step-timing";
  import {
    createMotionStepTimingQueue,
    type MotionStepTimingTask,
  } from "$lib/js/motion-step-timing-queue";
  import type { MotionTimelineTimingPatch } from "$lib/js/motion-timeline-interaction";
  import { getPageJsWorkspaceState } from "$lib/project/io";
  import { registerEditFlushHandler, type EditFlushReason } from "$lib/session/edit-flush-registry";
  import {
    createPageJsRequestIdentity,
    isPageJsRequestIdentityCurrent,
    pageJsCommandPayload,
  } from "$lib/session/page-js-command-session";
  import { normalizePageJsTemplatePath } from "$lib/js/page-path";
  import { queuePageJsDraftSync } from "$lib/session/page-js-draft-sync";
  import type {
    PageJsConfig,
    PanaMotionConfig,
    PanaMotionTimelineItem,
    PanaMotionTimelineStep,
  } from "$lib/types";

  let {
    activeTemplatePath = null,
    projectRoot = "",
    runtimeSessionId = "",
    refreshToken = 0,
    onPendingChange = undefined as ((pending: boolean) => void) | undefined,
  }: {
    activeTemplatePath?: string | null;
    projectRoot?: string;
    runtimeSessionId?: string;
    refreshToken?: number;
    onPendingChange?: (pending: boolean) => void;
  } = $props();

  const templatePath = $derived.by(() => {
    const canonicalPath = normalizePageJsTemplatePath(activeTemplatePath);
    return canonicalPath || null;
  });

  type TimelineConfigLoadState = "idle" | "loading" | "ready" | "error";

  let config = $state<PageJsConfig>(emptyPageJsConfig());
  let baseConfig = $state<PageJsConfig>(emptyPageJsConfig());
  let timelineConfigLoadState = $state<TimelineConfigLoadState>("idle");
  let readyTemplatePath = "";
  let readyProjectRoot = "";
  let readyRuntimeSessionId = "";
  let readyRefreshToken: number | null = null;
  let lastTplPath = "";
  let lastProjectRoot = "";
  let lastRuntimeSessionId = "";
  let lastHandledRefreshToken: number | null = null;
  let timelineDraftTimer: number | null = null;
  let timelineDraftDirty = $state(false);
  let timelineDraftTarget: string | null = null;
  let loadSerial = 0;
  let templateTransitionSerial = 0;
  let timelineLoadError = $state("");
  let localSelectedStepId = $state<string | null>(null);
  let unregisterFlushHandler: (() => void) | null = null;
  let disposed = false;
  const stepTimingQueue = createMotionStepTimingQueue(commitStepTimingTask);

  const TIMELINE_DRAFT_COMMIT_MS = 320;

  const jsPending = $derived.by(() => {
    return Boolean(templatePath && timelineDraftDirty);
  });

  $effect(() => {
    const pending = jsPending;
    untrack(() => onPendingChange?.(pending));
  });

  $effect(() => {
    const tpl = templatePath;
    const root = projectRoot;
    const sessionId = runtimeSessionId;
    const nextPath = tpl ?? "";
    if (
      nextPath === lastTplPath
      && root === lastProjectRoot
      && sessionId === lastRuntimeSessionId
    ) return;
    beginTemplateTransition(tpl, root, sessionId);
  });

  function beginTemplateTransition(
    tpl: string | null,
    targetProjectRoot = projectRoot,
    targetRuntimeSessionId = runtimeSessionId,
  ) {
    if (disposed) return;
    const transitionSerial = ++templateTransitionSerial;
    loadSerial += 1;
    timelineConfigLoadState = "loading";
    timelineLoadError = "";
    void stepTimingQueue.flush({ throwOnFailure: true }).then(() => {
      if (
        disposed
        || transitionSerial !== templateTransitionSerial
        || templatePath !== tpl
        || projectRoot !== targetProjectRoot
        || runtimeSessionId !== targetRuntimeSessionId
      ) return;
      flushTimelineDraftToSession("template-switch");
      lastTplPath = tpl ?? "";
      lastProjectRoot = targetProjectRoot;
      lastRuntimeSessionId = targetRuntimeSessionId;
      if (tpl && targetProjectRoot && targetRuntimeSessionId) {
        void loadConfig(tpl, targetProjectRoot, targetRuntimeSessionId);
      } else {
        stepTimingQueue.reset();
        readyTemplatePath = "";
        readyProjectRoot = "";
        readyRuntimeSessionId = "";
        readyRefreshToken = null;
        baseConfig = emptyPageJsConfig();
        config = emptyPageJsConfig();
        timelineDraftDirty = false;
        timelineDraftTarget = null;
        timelineConfigLoadState = tpl ? "error" : "idle";
        timelineLoadError = tpl
          ? "ProjectSession nu este disponibilă pentru citirea Page JS."
          : "";
      }
    }).catch((error) => {
      if (
        disposed
        || transitionSerial !== templateTransitionSerial
        || projectRoot !== targetProjectRoot
        || runtimeSessionId !== targetRuntimeSessionId
      ) return;
      timelineLoadError = error instanceof Error ? error.message : String(error);
      timelineConfigLoadState = "error";
    });
  }

  $effect(() => {
    const token = refreshToken;
    const tpl = templatePath;
    if (lastHandledRefreshToken === null) {
      lastHandledRefreshToken = token;
      return;
    }
    if (token === lastHandledRefreshToken) return;
    lastHandledRefreshToken = token;
    if (!tpl || tpl !== lastTplPath) return;
    const targetProjectRoot = projectRoot;
    const targetRuntimeSessionId = runtimeSessionId;
    const targetRefreshToken = token;
    loadSerial += 1;
    timelineConfigLoadState = "loading";
    timelineLoadError = "";
    void stepTimingQueue.flush({ throwOnFailure: true }).then(() => {
      if (
        disposed
        || templatePath !== tpl
        || lastTplPath !== tpl
        || projectRoot !== targetProjectRoot
        || runtimeSessionId !== targetRuntimeSessionId
        || lastProjectRoot !== targetProjectRoot
        || lastRuntimeSessionId !== targetRuntimeSessionId
        || refreshToken !== targetRefreshToken
      ) return;
      flushTimelineDraftToSession("manual");
      void loadConfig(tpl, targetProjectRoot, targetRuntimeSessionId, targetRefreshToken);
    }).catch((error) => {
      if (
        disposed
        || templatePath !== tpl
        || projectRoot !== targetProjectRoot
        || runtimeSessionId !== targetRuntimeSessionId
        || refreshToken !== targetRefreshToken
      ) return;
      timelineLoadError = error instanceof Error ? error.message : String(error);
      timelineConfigLoadState = "error";
    });
  });

  $effect(() => {
    return () => {
      untrack(() => onPendingChange?.(false));
      // Teardown cannot await. Invalidate all pending/in-flight work first,
      // synchronously stage only an already-resolved local draft, then forbid
      // every async continuation from publishing after this instance is gone.
      templateTransitionSerial += 1;
      loadSerial += 1;
      stepTimingQueue.reset();
      flushTimelineDraftToSession("unmount");
      readyTemplatePath = "";
      readyProjectRoot = "";
      readyRuntimeSessionId = "";
      readyRefreshToken = null;
      disposed = true;
      unregisterFlushHandler?.();
      unregisterFlushHandler = null;
    };
  });

  $effect(() => {
    unregisterFlushHandler?.();
    unregisterFlushHandler = registerEditFlushHandler("motion-timeline-panel", async (reason) => {
      await stepTimingQueue.flush({ throwOnFailure: true });
      flushTimelineDraftToSession(reason);
    });
    return () => {
      unregisterFlushHandler?.();
      unregisterFlushHandler = null;
    };
  });

  async function loadConfig(
    tpl: string,
    targetProjectRoot = projectRoot,
    targetRuntimeSessionId = runtimeSessionId,
    targetRefreshToken = refreshToken,
  ) {
    flushTimelineDraftToSession("manual");
    const serial = ++loadSerial;
    timelineConfigLoadState = "loading";
    timelineLoadError = "";
    stepTimingQueue.reset();
    timelineDraftDirty = false;
    timelineDraftTarget = null;
    readyTemplatePath = "";
    readyProjectRoot = "";
    readyRuntimeSessionId = "";
    readyRefreshToken = null;

    try {
      const identity = createPageJsRequestIdentity(targetProjectRoot, targetRuntimeSessionId);
      const receipt = await getPageJsWorkspaceState(tpl, identity);
      if (
        disposed
        || serial !== loadSerial
        || templatePath !== tpl
        || lastTplPath !== tpl
        || projectRoot !== targetProjectRoot
        || runtimeSessionId !== targetRuntimeSessionId
        || lastProjectRoot !== targetProjectRoot
        || lastRuntimeSessionId !== targetRuntimeSessionId
        || refreshToken !== targetRefreshToken
      ) return;
      if (!isPageJsRequestIdentityCurrent(identity, projectRoot, runtimeSessionId)) return;
      const workspaceState = pageJsCommandPayload(
        receipt,
        identity,
        "Citirea Page JS din Timeline",
      );
      baseConfig = normalizePageJsConfig(workspaceState.accepted);
      config = normalizePageJsConfig(workspaceState.current);
      readyTemplatePath = tpl;
      readyProjectRoot = targetProjectRoot;
      readyRuntimeSessionId = targetRuntimeSessionId;
      readyRefreshToken = targetRefreshToken;
      timelineConfigLoadState = "ready";
    } catch (error) {
      if (
        disposed
        || serial !== loadSerial
        || templatePath !== tpl
        || lastTplPath !== tpl
        || projectRoot !== targetProjectRoot
        || runtimeSessionId !== targetRuntimeSessionId
        || lastProjectRoot !== targetProjectRoot
        || lastRuntimeSessionId !== targetRuntimeSessionId
        || refreshToken !== targetRefreshToken
      ) return;
      timelineLoadError = error instanceof Error ? error.message : String(error);
      timelineConfigLoadState = "error";
    }
  }

  function isLoadedConfigTarget(
    target: string | null,
    targetProjectRoot = projectRoot,
    targetRuntimeSessionId = runtimeSessionId,
  ): target is string {
    return Boolean(
      target
      && readyTemplatePath === target
      && readyProjectRoot === targetProjectRoot
      && readyRuntimeSessionId === targetRuntimeSessionId
    );
  }

  function isCurrentConfigReady(): boolean {
    return timelineConfigLoadState === "ready"
      && isLoadedConfigTarget(templatePath, projectRoot, runtimeSessionId)
      && readyRefreshToken === refreshToken
      && lastTplPath === templatePath
      && lastProjectRoot === projectRoot
      && lastRuntimeSessionId === runtimeSessionId;
  }

  function retryTimelineLoad() {
    const tpl = templatePath;
    const targetProjectRoot = projectRoot;
    const targetRuntimeSessionId = runtimeSessionId;
    if (
      tpl
      && tpl === lastTplPath
      && targetProjectRoot === lastProjectRoot
      && targetRuntimeSessionId === lastRuntimeSessionId
    ) {
      void loadConfig(tpl, targetProjectRoot, targetRuntimeSessionId, refreshToken);
      return;
    }
    beginTemplateTransition(tpl, targetProjectRoot, targetRuntimeSessionId);
  }

  function clearTimelineDraftTimer() {
    if (timelineDraftTimer === null) return;
    window.clearTimeout(timelineDraftTimer);
    timelineDraftTimer = null;
  }

  function commitConfigToSession(
    nextConfig: PageJsConfig,
    target: string | null = templatePath,
  ) {
    const targetPath = target ?? "";
    if (!targetPath || !isLoadedConfigTarget(targetPath, projectRoot)) return false;
    queuePageJsDraftSync({
      templatePath: targetPath,
      baseConfig,
      currentConfig: nextConfig,
      cachebustAssets: false,
      source: "motion.timeline",
      coalesceKey: "page_js.timeline",
    });
    return true;
  }

  function stageConfig(nextConfig: PageJsConfig) {
    if (!isCurrentConfigReady()) return;
    clearTimelineDraftTimer();
    timelineDraftDirty = false;
    timelineDraftTarget = null;
    const nextNormalized = normalizePageJsConfig(nextConfig);
    config = nextNormalized;
    commitConfigToSession(nextNormalized);
  }

  function updateMotionConfig(motion: PanaMotionConfig) {
    stageConfig({ ...config, version: 1, motion });
  }

  function commitTimelineDraftToSession(
    timeline: PanaMotionTimelineItem,
    target: string | null = timelineDraftTarget ?? templatePath,
  ) {
    const targetPath = target ?? "";
    if (!targetPath || !isLoadedConfigTarget(targetPath, projectRoot)) return false;
    const nextConfig = pageConfigWithTimeline(config, timeline);
    config = nextConfig;
    return commitConfigToSession(nextConfig, targetPath);
  }

  function flushTimelineDraftToSession(_reason: EditFlushReason) {
    clearTimelineDraftTimer();
    if (!timelineDraftDirty) return false;
    const target = timelineDraftTarget ?? templatePath;
    const timeline = activeOrchestratorTimeline(normalizeMotionConfig(config.motion));
    timelineDraftDirty = false;
    timelineDraftTarget = null;
    if (!target || !timeline) return false;
    return commitTimelineDraftToSession(timeline, target);
  }

  function markTimelineDraftDirty(targetOverride: string | null = templatePath) {
    if (disposed) return;
    const target = targetOverride;
    if (!target || !isLoadedConfigTarget(target, projectRoot)) return;
    timelineDraftDirty = true;
    timelineDraftTarget = target;
    clearTimelineDraftTimer();
    timelineDraftTimer = window.setTimeout(() => {
      timelineDraftTimer = null;
      flushTimelineDraftToSession("manual");
    }, TIMELINE_DRAFT_COMMIT_MS);
  }

  const motionState = $derived.by(() => normalizeMotionConfig(config.motion));
  const activeTimeline = $derived.by(() => activeOrchestratorTimeline(motionState));
  const composerItems = $derived.by(() => timelineComposerItems(motionState.items, activeTimeline?.id ?? null));
  const selectedStepId = $derived.by(() => {
    if (!activeTimeline) return null;
    if (localSelectedStepId && activeTimeline.steps.some((step) => step.id === localSelectedStepId)) {
      return localSelectedStepId;
    }
    const activeStep = activeTimeline.steps.find((step) => step.id === motionState.activeItemId);
    return activeStep?.id ?? null;
  });

  const orphanAnimations = $derived.by(() => {
    return orphanTimelineAnimations(motionState.items);
  });

  function setActiveStep(stepId: string) {
    localSelectedStepId = stepId;
  }

  function updateTimelineItem(
    nextTimeline: PanaMotionTimelineItem,
    activeId: string | null | undefined = undefined,
  ) {
    if (!isCurrentConfigReady()) return;
    const motion = normalizeMotionConfig(config.motion);
    const nextMotion = replaceOrchestratorTimeline(motion, nextTimeline, activeId);
    if (nextMotion === motion) return;
    updateMotionConfig(nextMotion);
  }

  function firstTrackId(timeline: PanaMotionTimelineItem): string {
    return timeline.tracks[0]?.id || MOTION_TIMELINE_DEFAULT_TRACK_ID;
  }

  function createTimeline() {
    if (!isCurrentConfigReady()) return;
    const motion = normalizeMotionConfig(config.motion);
    const timeline = createMotionItem("timeline") as PanaMotionTimelineItem;
    updateMotionConfig(appendOrchestratorTimeline(motion, timeline));
  }

  function updateStepTiming(stepIndex: number, patch: MotionTimelineTimingPatch) {
    if (disposed || !isCurrentConfigReady() || !activeTimeline) return;
    const target = templatePath;
    const step = activeTimeline.steps[stepIndex];
    if (!target || !step) return;
    stepTimingQueue.enqueue({
      projectRoot,
      runtimeSessionId,
      templatePath: target,
      timelineId: activeTimeline.id,
      stepId: step.id,
      stepIndex,
      patch,
    });
  }

  async function commitStepTimingTask(
    task: MotionStepTimingTask,
    context: { isCurrent: () => boolean },
  ) {
    for (let attempt = 0; attempt < 3; attempt += 1) {
      if (
        disposed
        || !context.isCurrent()
        || task.projectRoot !== projectRoot
        || task.runtimeSessionId !== runtimeSessionId
        || task.templatePath !== lastTplPath
        || !isLoadedConfigTarget(task.templatePath, task.projectRoot, task.runtimeSessionId)
      ) return;
      const taskMotion = normalizeMotionConfig(config.motion);
      const taskTimeline = taskMotion.items.find(
        (item): item is PanaMotionTimelineItem => item.type === "timeline" && item.id === task.timelineId,
      );
      if (!taskTimeline?.steps.some((step) => step.id === task.stepId)) return;
      const sourceConfig = config;
      const result = await applyMotionTimelineStepTimingProjection({
        config: sourceConfig,
        timelineId: task.timelineId,
        stepId: task.stepId,
        stepIndex: task.stepIndex,
        patch: task.patch,
      });
      if (
        disposed
        || !context.isCurrent()
        || task.projectRoot !== projectRoot
        || task.runtimeSessionId !== runtimeSessionId
        || task.templatePath !== lastTplPath
        || !isLoadedConfigTarget(task.templatePath, task.projectRoot, task.runtimeSessionId)
      ) return;
      // A different editor may have committed while Rust validated the patch.
      // Retry against that newer projection instead of overwriting it.
      if (config !== sourceConfig) continue;
      if (!result.changed) return;
      if (result.selectedStepId) localSelectedStepId = result.selectedStepId;
      config = result.config;
      markTimelineDraftDirty(task.templatePath);
      return;
    }
    throw new Error("Motion Timeline nu a putut compune stepTiming peste configurația aflată în schimbare.");
  }

  function updateStep(nextStep: PanaMotionTimelineStep) {
    if (!isCurrentConfigReady() || !activeTimeline) return;
    const steps = activeTimeline.steps.map((step) => step.id === nextStep.id ? nextStep : step);
    localSelectedStepId = nextStep.id;
    updateTimelineItem({ ...activeTimeline, steps });
  }

  function deleteStep(stepId: string) {
    if (!isCurrentConfigReady() || !activeTimeline) return;
    const steps = activeTimeline.steps.filter((step) => step.id !== stepId);
    localSelectedStepId = steps[0]?.id ?? null;
    updateTimelineItem({ ...activeTimeline, steps });
  }

  function addAnimationStep(animationId: string) {
    if (!isCurrentConfigReady()) return;
    const motion = normalizeMotionConfig(config.motion);
    const timeline = activeTimeline;
    const animation = motionActorItems(motion.items).find((item) => item.id === animationId && item.type === "animation");
    if (!timeline || !animation || animation.type !== "animation") return;
    const step = { ...timelineStepFromAnimation(animation, timeline.steps.length), lane: firstTrackId(timeline) };
    const nextTimeline = { ...timeline, steps: [...timeline.steps, step] };
    localSelectedStepId = step.id;
    updateMotionConfig(replaceOrchestratorTimeline(motion, nextTimeline));
  }

  function addCallbackStep(position: string) {
    if (!isCurrentConfigReady()) return;
    const motion = normalizeMotionConfig(config.motion);
    const timeline = activeTimeline;
    if (!timeline) return;
    const step: PanaMotionTimelineStep = {
      id: `step-${Math.random().toString(36).slice(2, 9)}`,
      type: "callback",
      label: "Callback",
      position,
      duration: 50,
      lane: firstTrackId(timeline),
      targetItemId: "",
      callback: emptyExpression("Timeline callback"),
    };
    const nextTimeline = { ...timeline, steps: [...timeline.steps, step] };
    localSelectedStepId = step.id;
    updateMotionConfig(replaceOrchestratorTimeline(motion, nextTimeline));
  }

  function addGenericStep(type: PanaMotionTimelineStep["type"], position: string) {
    if (!isCurrentConfigReady()) return;
    const motion = normalizeMotionConfig(config.motion);
    const timeline = activeTimeline;
    if (!timeline) return;
    const target = timelineStepTargetItems(motion.items, timeline.id, type)[0] ?? null;
    const step = { ...timelineStepFromItem(target, type, position, timeline.steps.length), lane: firstTrackId(timeline) };
    const nextTimeline = { ...timeline, steps: [...timeline.steps, step] };
    localSelectedStepId = step.id;
    updateMotionConfig(replaceOrchestratorTimeline(motion, nextTimeline));
  }

  function addLabel(position: string) {
    if (!isCurrentConfigReady()) return;
    const timeline = activeTimeline;
    if (!timeline) return;
    updateTimelineItem({
      ...timeline,
      labels: [
        ...timeline.labels,
        { id: `label-${Math.random().toString(36).slice(2, 9)}`, name: `Label ${timeline.labels.length + 1}`, position },
      ],
    }, timeline.id);
  }

</script>

<section class="motion-timeline-pane-shell" aria-label="Motion Timeline">
  <div class="motion-design-safe-note" role="status">
    <strong>Design Safe</strong>
    <span>Playback și seek JS sunt oprite în editor; Run extern execută animația completă.</span>
  </div>
  {#if timelineConfigLoadState === "error"}
    <div class="motion-empty-state motion-error-state" role="alert">
      <strong>JS-ul paginii nu a putut fi încărcat</strong>
      <span>{timelineLoadError || "Citirea configurației Motion Timeline a eșuat."}</span>
      <button type="button" onclick={retryTimelineLoad}>Reîncearcă</button>
    </div>
  {:else if !templatePath}
    <div class="motion-empty-state">
      <strong>Motion Timeline</strong>
      <span>Nu există un template activ.</span>
    </div>
  {:else if !isCurrentConfigReady()}
    <div class="motion-empty-state" aria-live="polite">
      <strong>Anime Timeline</strong>
      <span>Se citește JS-ul paginii curente…</span>
    </div>
  {:else if !activeTimeline}
    <div class="motion-empty-state">
      <strong>Anime Timeline</strong>
      <span>Creează un timeline pentru pagina curentă.</span>
      <button type="button" onclick={createTimeline}>Creează timeline</button>
    </div>
  {:else}
    <MotionTimeline
      timelineItem={activeTimeline}
      motionItems={composerItems}
      selectedStepId={selectedStepId}
      emptyMessage={orphanAnimations.length > 0 ? "Adaugă animații ca steps în timeline." : "Creează animații în Efecte."}
      onSelectStep={setActiveStep}
      onTimingChange={updateStepTiming}
      onAddAnimationStep={addAnimationStep}
      onAddCallbackStep={addCallbackStep}
      onAddTimerStep={(position) => addGenericStep("timer", position)}
      onAddSyncStep={(position) => addGenericStep("sync", position)}
      onAddSetStep={(position) => addGenericStep("set", position)}
      onAddLabel={addLabel}
    />
  {/if}
</section>

<style>
  .motion-timeline-pane-shell {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .motion-design-safe-note {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    padding: 5px 8px;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    color: var(--text-muted);
    font-size: 10px;
    background: var(--surface-3);
  }

  .motion-design-safe-note strong {
    flex: 0 0 auto;
    color: var(--brand-strong);
    font-weight: 850;
  }

  .motion-design-safe-note span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .motion-empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 6px;
    height: 100%;
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-7);
    color: var(--text-muted);
    box-shadow: var(--shadow);
    font-size: 12px;
  }

  .motion-empty-state button {
    min-height: 28px;
    border: 1px solid var(--brand);
    border-radius: 6px;
    background: var(--brand-soft);
    color: var(--brand-strong);
    font-size: 11px;
    font-weight: 900;
    cursor: pointer;
  }

  .motion-empty-state strong {
    color: var(--text);
    font-size: 13px;
  }

  :global(.motion-timeline-pane-shell .motion-panel) {
    flex: 1;
  }

</style>
