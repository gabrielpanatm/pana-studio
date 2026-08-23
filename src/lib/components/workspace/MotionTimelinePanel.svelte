<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    IconArrowsMaximize,
    IconChevronDown,
    IconChevronUp,
    IconCopy,
    IconPlayerPause,
    IconPlayerPlay,
    IconPlayerSkipBack,
    IconRepeat,
    IconRewindBackward10,
    IconTrash,
    IconVolume,
    IconVolumeOff,
    IconX,
    IconZoomIn,
    IconZoomOut,
  } from "@tabler/icons-svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { MotionWorkspaceState } from "$lib/motion/workspace.svelte";
  import {
    actionDuration,
    actionSpan,
    createAnimateAction,
    interactionDuration,
    motionId,
    targetForDataAnim,
    triggerTarget,
  } from "$lib/js/motion-v2";
  import type { InspectorSelectionSummarySnapshot } from "$lib/editor/contracts";
  import type {
    MotionAction,
    MotionInteraction,
    MotionTarget,
    MotionTargetRelation,
  } from "$lib/js/contracts";
  import {
    MotionTimelineGestureController,
    type MotionTimelineTimingDraft,
  } from "$lib/motion/timeline-gesture-controller";

  let {
    workspace,
    selectionSummary = null,
    dataAnim = null,
  }: {
    workspace: MotionWorkspaceState;
    selectionSummary?: InspectorSelectionSummarySnapshot | null;
    dataAnim?: string | null;
  } = $props();

  type TimingDraft = MotionTimelineTimingDraft;
  type TargetPresentation = {
    title: string;
    detail: string;
  };
  type TimelineLane = TargetPresentation & {
    key: string;
    actions: MotionAction[];
  };

  let timingDrafts = $state<Record<string, TimingDraft>>({});
  let clipboard = $state<MotionAction | null>(null);
  let playhead = $state(0);
  let playing = $state(false);
  let targetPresentationRevision = $state(0);
  const targetPresentations = new Map<string, TargetPresentation>();
  const timelineGestures = new MotionTimelineGestureController({
    selectedInteractionId: () => workspace.selectedInteraction?.id ?? null,
    selectAction: (interactionId, actionId) => {
      workspace.selectInteraction(interactionId, actionId);
    },
    setTimingDraft: (actionId, draft) => {
      if (draft) setTimingDraft(actionId, draft);
      else removeTimingDraft(actionId);
    },
    setPlayhead: (value) => {
      playhead = value;
    },
    requestSeek: (interactionId, value) => {
      workspace.requestPreview("seek", interactionId, value);
    },
    commitTiming: (commit) => workspace.mutate({
      command: "setActionTiming",
      ...commit,
    }),
    setGestureActive: (gesture, active) => {
      document.body.classList.toggle(
        gesture === "drag" ? "motion-timeline-dragging" : "motion-timeline-seeking",
        active,
      );
    },
  });

  const interaction = $derived(workspace.selectedInteraction);
  const duration = $derived(interaction ? interactionDuration(interaction) : 1);
  const lanes = $derived.by(() => {
    if (!interaction) return [];
    targetPresentationRevision;
    const groups = new Map<string, TimelineLane>();
    for (const action of interaction.actions) {
      const key = actionTargetKey(action);
      const group = groups.get(key);
      if (group) {
        group.actions.push(action);
        continue;
      }
      groups.set(key, {
        key,
        ...actionTargetPresentation(action, interaction),
        actions: [action],
      });
    }
    return Array.from(groups.values());
  });
  const ticks = $derived.by(() => {
    const count = 10;
    return Array.from({ length: count + 1 }, (_, index) => {
      const value = duration * index / count;
      return { value, left: index * 10 };
    });
  });

  $effect(() => {
    if (!interaction) return;
    playhead = Math.min(playhead, duration);
  });

  $effect(() => {
    const status = workspace.previewStatus;
    if (!interaction || !status || status.interactionId !== interaction.id) return;
    if (timelineGestures.isSeekingInteraction(interaction.id)) return;
    playhead = Math.max(0, Math.min(duration, status.value));
    playing = !status.paused;
  });

  $effect(() => {
    const interactionId = interaction?.id ?? null;
    timelineGestures.reconcileInteraction(interactionId);
  });

  $effect(() => {
    const summary = selectionSummary;
    const selectedDataAnim = dataAnim?.trim() ?? "";
    if (summary?.state !== "resolved" || !selectedDataAnim) return;
    const presentation = {
      title: selectionTitle(summary),
      detail: `[data-anim="${selectedDataAnim}"]`,
    };
    const current = targetPresentations.get(selectedDataAnim);
    if (current?.title === presentation.title && current.detail === presentation.detail) return;
    targetPresentations.set(selectedDataAnim, presentation);
    targetPresentationRevision += 1;
  });

  function selectionTitle(selection: InspectorSelectionSummarySnapshot) {
    const tag = selection.tag?.trim().toLowerCase() || "element";
    if (selection.elementId?.trim()) return `${tag}#${selection.elementId.trim()}`;
    const classes = selection.classes
      .map((name) => name.trim())
      .filter(Boolean)
      .slice(0, 2);
    return `${tag}${classes.map((name) => `.${name}`).join("")}`;
  }

  function targetKey(target: MotionTarget) {
    return [
      target.kind,
      target.dataAnim,
      target.selector,
      target.relation,
      target.scope,
    ].join("\u0000");
  }

  function actionTargetKey(action: MotionAction) {
    return "target" in action ? `target\u0000${targetKey(action.target)}` : "control";
  }

  function technicalTargetDetail(target: MotionTarget) {
    if (target.kind === "element") {
      return target.dataAnim
        ? `[data-anim="${target.dataAnim}"]`
        : t("motion-timeline-target-element");
    }
    if (target.kind === "selector") return target.selector || t("motion-target-selector");
    if (target.kind === "relative") {
      return `${relationLabel(target.relation)}${target.selector ? ` · ${target.selector}` : ""}`;
    }
    if (target.kind === "trigger") return t("motion-trigger-target");
    return target.kind === "viewport"
      ? t("motion-target-viewport")
      : t("motion-target-document");
  }

  function relationLabel(relation: MotionTargetRelation) {
    switch (relation) {
      case "selfElement": return t("motion-target-selected");
      case "children": return t("motion-relation-children");
      case "descendants": return t("motion-relation-descendants");
      case "parent": return t("motion-relation-parent");
      case "ancestors": return t("motion-relation-ancestors");
      case "siblings": return t("motion-relation-siblings");
      case "nextSibling": return t("motion-relation-next");
      case "previousSibling": return t("motion-relation-previous");
    }
  }

  function targetPresentation(target: MotionTarget): TargetPresentation {
    if (target.kind === "element" && target.dataAnim) {
      return targetPresentations.get(target.dataAnim) ?? {
        title: t("motion-timeline-target-element"),
        detail: technicalTargetDetail(target),
      };
    }
    if (target.kind === "selector") {
      return { title: t("motion-timeline-target-selector"), detail: technicalTargetDetail(target) };
    }
    if (target.kind === "relative") {
      return { title: t("motion-timeline-target-relative"), detail: technicalTargetDetail(target) };
    }
    if (target.kind === "viewport") {
      return {
        title: t("motion-target-viewport"),
        detail: t("motion-timeline-page-viewport"),
      };
    }
    if (target.kind === "document") {
      return {
        title: t("motion-target-document"),
        detail: t("motion-timeline-page-document"),
      };
    }
    return {
      title: t("motion-target-trigger"),
      detail: t("motion-trigger-target"),
    };
  }

  function actionTargetPresentation(
    action: MotionAction,
    currentInteraction: MotionInteraction,
  ): TargetPresentation {
    if (!("target" in action)) {
      return {
        title: t("motion-timeline-control-logic"),
        detail: t("motion-timeline-no-visual-target"),
      };
    }
    if (action.target.kind !== "trigger") return targetPresentation(action.target);
    const triggerPresentation = targetPresentation(currentInteraction.triggerTarget);
    return {
      title: triggerPresentation.title === t("motion-timeline-target-element")
        ? t("motion-target-trigger")
        : triggerPresentation.title,
      detail: triggerPresentation.detail,
    };
  }

  function triggerTypeLabel(type: MotionInteraction["trigger"]["type"]) {
    switch (type) {
      case "load": return t("motion-trigger-load");
      case "inView": return t("motion-trigger-in-view");
      case "click": return t("motion-trigger-click");
      case "hover": return t("motion-trigger-hover");
      case "scroll": return t("motion-trigger-scroll");
      case "pointer": return t("motion-trigger-pointer");
      case "custom": return t("motion-trigger-custom");
    }
  }

  function actionTypeLabel(type: MotionAction["type"]) {
    switch (type) {
      case "animate": return t("motion-action-animate");
      case "set": return t("motion-action-set");
      case "media": return t("motion-media-command");
      case "call": return t("motion-isolated-code");
      case "nested": return t("motion-action-nested");
    }
  }

  function draftFor(action: MotionAction): TimingDraft {
    return timingDrafts[action.id] ?? {
      start: action.start,
      duration: actionDuration(action),
    };
  }

  function draftSpan(action: MotionAction, draft: TimingDraft) {
    if (action.type !== "animate" || action.repeat.infinite) return draft.duration;
    return draft.duration * (action.repeat.count + 1)
      + action.repeat.delayMs * action.repeat.count;
  }

  function percent(value: number) {
    return Math.max(0, Math.min(100, value / Math.max(1, duration) * 100));
  }

  function formatValue(value: number) {
    if (interaction?.domain === "progress") {
      return `${l10n.formatNumber(Math.round(value * 10) / 10)}%`;
    }
    if (value >= 1_000) {
      return `${l10n.formatNumber(value / 1_000, {
        maximumFractionDigits: value % 1_000 ? 1 : 0,
      })}s`;
    }
    return `${l10n.formatNumber(Math.round(value))}ms`;
  }

  function snapValue(value: number) {
    if (!workspace.timelineSnap) return Math.max(0, value);
    const step = interaction?.domain === "progress" ? 1 : 50;
    return Math.max(0, Math.round(value / step) * step);
  }

  function removeTimingDraft(actionId: string) {
    if (!(actionId in timingDrafts)) return;
    timingDrafts = Object.fromEntries(
      Object.entries(timingDrafts).filter(([id]) => id !== actionId),
    );
  }

  function setTimingDraft(actionId: string, draft: TimingDraft) {
    timingDrafts = {
      ...timingDrafts,
      [actionId]: draft,
    };
  }

  function beginDrag(event: PointerEvent, action: MotionAction, mode: "move" | "resize") {
    if (!interaction || !(event.currentTarget instanceof HTMLElement)) return;
    const captureTarget = event.currentTarget;
    const canvas = captureTarget.closest<HTMLElement>(".timeline-canvas");
    if (!canvas) return;
    timelineGestures.beginActionDrag({
      event,
      captureTarget,
      action,
      interactionId: interaction.id,
      domain: interaction.domain,
      mode,
      canvasWidth: canvas.getBoundingClientRect().width,
      timelineDuration: duration,
      snap: workspace.timelineSnap,
      original: draftFor(action),
    });
  }

  function beginSeekDrag(event: PointerEvent) {
    if (!interaction || !(event.currentTarget instanceof HTMLElement)) return;
    const captureTarget = event.currentTarget;
    const rect = captureTarget.getBoundingClientRect();
    if (captureTarget.tabIndex >= 0) captureTarget.focus({ preventScroll: true });
    timelineGestures.beginSeek({
      event,
      captureTarget,
      interactionId: interaction.id,
      domain: interaction.domain,
      canvasLeft: rect.left,
      canvasWidth: rect.width,
      timelineDuration: duration,
      snap: workspace.timelineSnap,
    });
  }

  function stopPlayback() {
    playing = false;
  }

  function play(direction = 1) {
    if (!interaction) return;
    if (direction > 0 && playhead >= duration) playhead = 0;
    if (direction < 0 && playhead <= 0) playhead = duration;
    playing = true;
    workspace.requestPreview(direction > 0 ? "play" : "reverse", interaction.id);
  }

  function pause() {
    stopPlayback();
    workspace.requestPreview("pause");
  }

  function restart() {
    stopPlayback();
    playhead = 0;
    workspace.requestPreview("restart");
  }

  function updateInteraction(next: MotionInteraction) {
    return workspace.mutate({ command: "updateInteraction", interaction: next });
  }

  function toggleMute(action: MotionAction) {
    if (!interaction) return;
    return workspace.mutate({
      command: "updateAction",
      interactionId: interaction.id,
      action: { ...structuredClone(action), enabled: !action.enabled },
    });
  }

  async function duplicate(action: MotionAction) {
    if (!interaction) return;
    const copy = structuredClone(action);
    copy.id = motionId("action");
    copy.name = `${action.name} copy`;
    copy.start += action.type === "animate" || action.type === "nested"
      ? actionSpan(action)
      : interaction.domain === "progress" ? 2 : 100;
    if (interaction.domain === "progress") {
      copy.start = Math.max(0, Math.min(100 - actionSpan(copy), copy.start));
    }
    await workspace.mutate({
      command: "insertAction",
      interactionId: interaction.id,
      index: interaction.actions.findIndex((item) => item.id === action.id) + 1,
      action: copy,
    });
    workspace.selectInteraction(interaction.id, copy.id);
  }

  async function pasteAction() {
    if (!interaction || !clipboard) return;
    const action = structuredClone(clipboard);
    action.id = motionId("action");
    action.name = `${clipboard.name} copy`;
    action.start = playhead;
    await workspace.mutate({
      command: "insertAction",
      interactionId: interaction.id,
      index: interaction.actions.length,
      action,
    });
    workspace.selectInteraction(interaction.id, action.id);
  }

  async function addAction() {
    if (!interaction) return;
    const selectedDataAnim = dataAnim?.trim() ?? "";
    const target = selectedDataAnim
      ? (
          interaction.triggerTarget.kind === "element"
          && interaction.triggerTarget.dataAnim === selectedDataAnim
            ? triggerTarget()
            : targetForDataAnim(selectedDataAnim)
        )
      : triggerTarget();
    const action = createAnimateAction("custom", target);
    action.start = playhead;
    action.duration = interaction.domain === "progress" ? 20 : 600;
    await workspace.mutate({
      command: "insertAction",
      interactionId: interaction.id,
      index: interaction.actions.length,
      action,
    });
    workspace.selectInteraction(interaction.id, action.id);
  }

  function addMarker() {
    if (!interaction) return;
    const marker = {
      id: motionId("marker"),
      name: `Marker ${interaction.markers.length + 1}`,
      at: playhead,
    };
    return updateInteraction({
      ...structuredClone(interaction),
      markers: [...interaction.markers, marker],
    });
  }

  function removeMarker(markerId: string) {
    if (!interaction) return;
    return updateInteraction({
      ...structuredClone(interaction),
      markers: interaction.markers.filter((marker) => marker.id !== markerId),
    });
  }

  async function deleteAction(action: MotionAction) {
    if (!interaction) return;
    if (interaction.actions.length === 1) {
      await workspace.mutate({ command: "deleteInteraction", interactionId: interaction.id });
      return;
    }
    await workspace.mutate({
      command: "deleteAction",
      interactionId: interaction.id,
      actionId: action.id,
    });
  }

  function fitTimeline() {
    workspace.timelineZoom = 1;
  }

  function seekFromKeyboard(event: KeyboardEvent) {
    if (!interaction) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "v") {
      event.preventDefault();
      void pasteAction();
      return;
    }
    if (event.key === " ") {
      event.preventDefault();
      if (playing) pause();
      else play();
      return;
    }
    if (event.key === "Home") {
      event.preventDefault();
      restart();
      return;
    }
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const step = interaction.domain === "progress"
      ? event.shiftKey ? 5 : 1
      : event.shiftKey ? 250 : 50;
    playhead = Math.max(
      0,
      Math.min(duration, snapValue(playhead + (event.key === "ArrowRight" ? step : -step))),
    );
    workspace.requestPreview("seek", interaction.id, playhead);
  }

  function editClipFromKeyboard(event: KeyboardEvent, action: MotionAction) {
    if (!interaction) return;
    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      void deleteAction(action);
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "c") {
      event.preventDefault();
      clipboard = structuredClone(action);
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "v") {
      event.preventDefault();
      void pasteAction();
      return;
    }
    if (
      (event.ctrlKey || event.metaKey)
      && (event.key === "ArrowUp" || event.key === "ArrowDown")
    ) {
      event.preventDefault();
      const index = interaction.actions.findIndex((candidate) => candidate.id === action.id);
      void workspace.mutate({
        command: "reorderAction",
        interactionId: interaction.id,
        actionId: action.id,
        index: Math.max(
          0,
          Math.min(interaction.actions.length - 1, index + (event.key === "ArrowDown" ? 1 : -1)),
        ),
      });
      return;
    }
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const direction = event.key === "ArrowRight" ? 1 : -1;
    const step = interaction.domain === "progress"
      ? event.shiftKey ? 5 : 1
      : event.shiftKey ? 250 : 50;
    if (event.altKey && (action.type === "animate" || action.type === "nested")) {
      void workspace.mutate({
        command: "setActionTiming",
        interactionId: interaction.id,
        actionId: action.id,
        duration: Math.max(
          interaction.domain === "progress" ? 1 : 50,
          action.duration + direction * step,
        ),
      });
    } else {
      void workspace.mutate({
        command: "setActionTiming",
        interactionId: interaction.id,
        actionId: action.id,
        start: Math.max(0, action.start + direction * step),
      });
    }
  }

  onDestroy(() => {
    timelineGestures.destroy();
  });
</script>

<section class="motion-timeline-panel" aria-label={t("motion-timeline-aria")}>
  <header class="timeline-header">
    <div class="timeline-identity">
      <span>{t("motion-timeline-heading")}</span>
      {#if interaction}
        <strong>{interaction.name}</strong>
        <small>
          {triggerTypeLabel(interaction.trigger.type)}
          ·
          {interaction.domain === "progress"
            ? t("motion-timeline-domain-progress")
            : t("motion-timeline-domain-time")}
        </small>
      {:else}
        <strong>{t("motion-timeline-none-selected")}</strong>
      {/if}
    </div>
    <div class="timeline-window-actions">
      <button type="button" title={workspace.timelineCollapsed ? t("motion-timeline-expand") : t("motion-timeline-collapse")} onclick={() => {
        workspace.timelineCollapsed = !workspace.timelineCollapsed;
      }}>
        {#if workspace.timelineCollapsed}<IconChevronUp size={15} />{:else}<IconChevronDown size={15} />{/if}
      </button>
      <button class="ui-icon-button ui-close-button" type="button" title={t("motion-timeline-close")} onclick={() => workspace.closeTimeline()}>
        <IconX size={15} />
      </button>
    </div>
  </header>

  {#if !workspace.timelineCollapsed}
    {#if interaction}
      <div class="timeline-toolbar">
        <div class="transport">
          <button type="button" title={t("motion-timeline-restart")} onclick={restart}><IconPlayerSkipBack size={15} /></button>
          <button type="button" title={t("motion-timeline-reverse")} onclick={() => play(-1)}><IconRewindBackward10 size={15} /></button>
          {#if playing}
            <button class="primary" type="button" title={t("motion-timeline-pause")} onclick={pause}><IconPlayerPause size={15} /></button>
          {:else}
            <button class="primary" type="button" title={t("motion-timeline-play")} onclick={() => play(1)}><IconPlayerPlay size={15} /></button>
          {/if}
          <span class="time-readout">{formatValue(playhead)} / {formatValue(duration)}</span>
        </div>
        <div class="timeline-tools">
          <button
            type="button"
            class:active={interaction.playback.infinite}
            disabled={interaction.domain === "progress"}
            title={t("motion-loop")}
            onclick={() => { void updateInteraction({
              ...structuredClone(interaction),
              playback: { ...interaction.playback, infinite: !interaction.playback.infinite },
            }); }}
          ><IconRepeat size={14} /></button>
          <label class="speed-field" title={t("motion-timeline-playback-speed")}>
            <span>{t("motion-speed")}</span>
            <select value={interaction.playback.playbackRate} onchange={(event) => {
              void updateInteraction({
                ...structuredClone(interaction),
                playback: {
                  ...interaction.playback,
                  playbackRate: Number(event.currentTarget.value),
                },
              });
            }}>
              <option value={0.25}>0.25×</option>
              <option value={0.5}>0.5×</option>
              <option value={1}>1×</option>
              <option value={1.5}>1.5×</option>
              <option value={2}>2×</option>
            </select>
          </label>
          <button type="button" class:active={workspace.timelineSnap} title={t("motion-timeline-snap")} onclick={() => {
            workspace.timelineSnap = !workspace.timelineSnap;
          }}>{t("motion-timeline-snap")}</button>
          <button type="button" title={t("motion-timeline-zoom-out")} onclick={() => {
            workspace.timelineZoom = Math.max(1, workspace.timelineZoom - 0.25);
          }}><IconZoomOut size={14} /></button>
          <button type="button" title={t("motion-timeline-zoom-in")} onclick={() => {
            workspace.timelineZoom = Math.min(4, workspace.timelineZoom + 0.25);
          }}><IconZoomIn size={14} /></button>
          <button type="button" title={t("motion-timeline-fit")} onclick={fitTimeline}><IconArrowsMaximize size={14} /></button>
          <button type="button" disabled={!clipboard} title={t("motion-timeline-paste-at-playhead")} onclick={() => { void pasteAction(); }}>{t("motion-timeline-paste")}</button>
          <button type="button" title={t("motion-timeline-add-marker-at-playhead")} onclick={() => { void addMarker(); }}>+ {t("motion-timeline-marker")}</button>
          <button class="add-action" type="button" onclick={() => { void addAction(); }}>+ {t("motion-timeline-action")}</button>
        </div>
      </div>

      <div class="timeline-scroll">
        <div
          class="timeline-grid"
          style={`--timeline-width: ${Math.max(100, workspace.timelineZoom * 100)}%;`}
        >
          <div class="track-label ruler-label">
            <span>{t("motion-timeline-target-track")}</span>
          </div>
          <div
            class="timeline-ruler timeline-canvas"
            role="slider"
            tabindex="0"
            aria-label={t("motion-timeline-playhead")}
            aria-valuemin="0"
            aria-valuemax={duration}
            aria-valuenow={playhead}
            onpointerdown={beginSeekDrag}
            onkeydown={seekFromKeyboard}
          >
            {#each ticks as tick, index (tick.left)}
              <span
                class="ruler-tick"
                class:end={index === ticks.length - 1}
                style={`left:${tick.left}%`}
              >
                <i></i><b>{formatValue(tick.value)}</b>
              </span>
            {/each}
          </div>

          {#each lanes as lane (lane.key)}
            <div
              class="track-label target-group-label"
              style={`grid-row:span ${Math.max(1, lane.actions.length)};`}
              title={`${lane.title} · ${lane.detail}`}
            >
              <strong>{lane.title}</strong>
              <small>{lane.detail}</small>
            </div>
            {#each lane.actions as action (action.id)}
              {@const draft = draftFor(action)}
              {@const instant = actionDuration(action) === 0}
              <div
                class="track timeline-canvas"
                role="group"
                aria-label={t("motion-timeline-action-aria", {
                  target: lane.title,
                  action: action.name,
                })}
                onpointerdown={beginSeekDrag}
              >
                <button
                  type="button"
                  class="action-clip ui-entity-selectable"
                  data-ui-selected={workspace.selectedActionId === action.id ? "true" : undefined}
                  aria-pressed={workspace.selectedActionId === action.id}
                  class:muted={!action.enabled}
                  class:instant
                  data-type={action.type}
                  style={`left:${percent(draft.start)}%;width:${instant ? "8px" : `${Math.max(1.2, percent(draftSpan(action, draft)))}%`};`}
                  title={`${action.name} · ${formatValue(draft.start)}`}
                  onpointerdown={(event) => beginDrag(event, action, "move")}
                  onclick={(event) => {
                    event.stopPropagation();
                    workspace.selectInteraction(interaction.id, action.id);
                  }}
                  onkeydown={(event) => editClipFromKeyboard(event, action)}
                >
                  <span>{action.name}</span>
                  {#if !instant}
                    <i
                      role="slider"
                      aria-label={t("motion-timeline-action-duration", { action: action.name })}
                      aria-valuemin="0"
                      aria-valuemax={duration}
                      aria-valuenow={draft.duration}
                      tabindex="0"
                      onpointerdown={(event) => beginDrag(event, action, "resize")}
                    ></i>
                  {/if}
                </button>
                {#each interaction.markers as marker (marker.id)}
                  <button
                    type="button"
                    class="marker"
                    style={`left:${percent(marker.at)}%`}
                    title={t("motion-timeline-marker-tooltip", { marker: marker.name })}
                    onpointerdown={(event) => event.stopPropagation()}
                    onclick={(event) => {
                      event.stopPropagation();
                      playhead = marker.at;
                      workspace.requestPreview("seek", interaction.id, marker.at);
                    }}
                    ondblclick={(event) => {
                      event.stopPropagation();
                      void removeMarker(marker.id);
                    }}
                  ></button>
                {/each}
              </div>
            {/each}
          {/each}
          <div
            class="timeline-playhead-layer"
            style={`--playhead-position:${percent(playhead)}%;`}
            aria-hidden="true"
          >
            <span
              class="timeline-playhead"
              class:at-start={playhead <= 0}
              class:at-end={playhead >= duration}
            ></span>
          </div>
        </div>
      </div>

      {#if workspace.selectedAction}
        <footer class="selection-toolbar">
          <strong>{workspace.selectedAction.name}</strong>
          <span>{formatValue(draftFor(workspace.selectedAction).start)} · {actionTypeLabel(workspace.selectedAction.type)}</span>
          <button type="button" title={workspace.selectedAction.enabled ? t("motion-timeline-mute") : t("motion-enable")} onclick={() => {
            if (workspace.selectedAction) void toggleMute(workspace.selectedAction);
          }}>
            {#if workspace.selectedAction.enabled}<IconVolume size={14} />{:else}<IconVolumeOff size={14} />{/if}
          </button>
          <button type="button" title={t("motion-timeline-copy")} onclick={() => {
            if (workspace.selectedAction) clipboard = structuredClone(workspace.selectedAction);
          }}><IconCopy size={14} /></button>
          <button type="button" title={t("motion-duplicate")} onclick={() => {
            if (workspace.selectedAction) void duplicate(workspace.selectedAction);
          }}>{t("motion-duplicate")}</button>
          <button class="danger" type="button" title={t("motion-delete")} onclick={() => {
            if (workspace.selectedAction) void deleteAction(workspace.selectedAction);
          }}><IconTrash size={14} /></button>
        </footer>
      {/if}
    {:else}
      <div class="timeline-empty">
        <strong>{t("motion-timeline-contextual")}</strong>
        <span>{t("motion-timeline-contextual-description")}</span>
      </div>
    {/if}
  {/if}
</section>

<style>
  .motion-timeline-panel { display:grid; grid-template-rows:auto auto minmax(0,1fr) auto; min-width:0; min-height:0; overflow:hidden; border:1px solid var(--border-2); border-radius:var(--radius-panel); background:var(--surface-panel); color:var(--text); }
  button, select { color:inherit; font:inherit; }
  button { cursor:pointer; }
  .timeline-header { display:flex; align-items:center; justify-content:space-between; gap:10px; min-height:36px; padding:4px 7px 4px 10px; border-bottom:1px solid var(--border-3); background:var(--surface-2); }
  .timeline-identity { display:flex; align-items:baseline; gap:7px; min-width:0; }
  .timeline-identity > span { color:var(--brand-strong); font-size:11px; font-weight:900; letter-spacing:.08em; }
  .timeline-identity strong { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:11px; }
  .timeline-identity small { color:var(--text-muted); font-size:11px; }
  .timeline-window-actions, .transport, .timeline-tools, .selection-toolbar { display:flex; align-items:center; gap:4px; }
  .timeline-window-actions button, .timeline-toolbar button, .selection-toolbar button { display:grid; place-items:center; min-width:27px; min-height:27px; padding:0 6px; border:1px solid var(--border-2); border-radius:5px; background:var(--surface); font-size:11px; }
  .timeline-toolbar button.active, .timeline-toolbar button.primary { border-color:var(--brand); background:var(--brand-soft); color:var(--brand-strong); }
  .timeline-toolbar { display:flex; align-items:center; justify-content:space-between; gap:10px; min-height:38px; padding:4px 7px; border-bottom:1px solid var(--border-3); }
  .time-readout { min-width:96px; color:var(--text-muted); font:11px "JetBrains Mono",monospace; }
  .speed-field { display:flex; align-items:center; gap:4px; color:var(--text-muted); font-size:11px; }
  .speed-field select { min-height:27px; border:1px solid var(--border-2); border-radius:5px; background:var(--surface); font-size:11px; }
  .timeline-tools { flex-wrap:wrap; justify-content:flex-end; }
  .timeline-tools .add-action { color:var(--brand-strong); font-weight:800; }
  .timeline-scroll {
    min-height:0;
    overflow:auto;
    background:var(--surface);
    scrollbar-color:var(--border-strong) transparent;
    scrollbar-width:thin;
  }
  .timeline-scroll::-webkit-scrollbar { width:8px; height:8px; }
  .timeline-scroll::-webkit-scrollbar-track { background:transparent; }
  .timeline-scroll::-webkit-scrollbar-thumb {
    border:2px solid transparent;
    border-radius:999px;
    background:var(--border-strong);
    background-clip:padding-box;
  }
  .timeline-grid {
    --timeline-label-width:150px;
    position:relative;
    display:grid;
    grid-template-columns:var(--timeline-label-width) minmax(600px,1fr);
    align-content:start;
    width:var(--timeline-width);
    min-width:760px;
    min-height:100%;
    isolation:isolate;
  }
  .timeline-grid::before {
    position:absolute;
    z-index:0;
    top:28px;
    right:0;
    bottom:0;
    left:var(--timeline-label-width);
    background-image:linear-gradient(
      to right,
      var(--border-3) 0 1px,
      transparent 1px
    );
    background-repeat:repeat-x;
    background-size:10% 100%;
    content:"";
    pointer-events:none;
  }
  .track-label { position:sticky; left:0; z-index:5; display:flex; flex-direction:column; justify-content:center; gap:2px; min-height:45px; padding:0 9px; border-right:1px solid var(--border-2); border-bottom:1px solid var(--border-3); background:var(--surface-2); }
  .track-label strong { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:11px; }
  .track-label small, .track-label span { color:var(--text-muted); font-size:11px; }
  .target-group-label { align-self:stretch; min-width:0; border-bottom-color:var(--border-2); }
  .target-group-label small {
    overflow:hidden;
    text-overflow:ellipsis;
    white-space:nowrap;
    font:11px "JetBrains Mono",monospace;
  }
  .ruler-label { min-height:28px; }
  .timeline-canvas { position:relative; z-index:1; min-width:600px; }
  .timeline-ruler { min-height:28px; overflow:hidden; border-bottom:1px solid var(--border-2); background:var(--surface-3); cursor:crosshair; touch-action:none; }
  .ruler-tick { position:absolute; top:0; bottom:0; border-left:1px solid var(--border-3); pointer-events:none; }
  .ruler-tick i { display:block; height:6px; border-left:1px solid var(--text-muted); }
  .ruler-tick b { position:absolute; top:7px; left:4px; color:var(--text-muted); font:11px "JetBrains Mono",monospace; white-space:nowrap; }
  .ruler-tick.end b { right:4px; left:auto; }
  .track { min-height:45px; border-bottom:1px solid var(--border-3); cursor:crosshair; overflow:hidden; touch-action:none; }
  .action-clip { --ui-entity-background:color-mix(in srgb,var(--brand-soft) 78%,var(--surface)); --ui-entity-border-color:color-mix(in srgb,var(--brand) 68%,var(--border)); --ui-entity-color:var(--brand-strong); position:absolute; z-index:2; top:8px; height:29px; min-width:8px; overflow:hidden; padding:0 14px 0 7px; border:1px solid var(--ui-entity-border-color); border-radius:5px; background:var(--ui-entity-background); color:var(--ui-entity-color); text-align:left; font-size:11px; font-weight:800; cursor:grab; touch-action:none; }
  .action-clip[data-ui-selected="true"] { z-index:3; }
  .action-clip[data-type="set"] { --ui-entity-border-color:#b88630; --ui-entity-color:#8b5c13; --ui-entity-background:color-mix(in srgb,#f4d78e 34%,var(--surface)); }
  .action-clip[data-type="media"] { --ui-entity-border-color:#4679b8; --ui-entity-color:#315f96; --ui-entity-background:color-mix(in srgb,#9dc7f4 30%,var(--surface)); }
  .action-clip[data-type="call"], .action-clip[data-type="nested"] { --ui-entity-border-color:#8061ad; --ui-entity-color:#684493; --ui-entity-background:color-mix(in srgb,#c4abe7 28%,var(--surface)); }
  .action-clip.muted { opacity:.42; filter:saturate(.2); }
  .action-clip.instant { padding:0; border-radius:2px; }
  .action-clip span { display:block; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; pointer-events:none; }
  .action-clip i { position:absolute; top:0; right:0; width:8px; height:100%; cursor:ew-resize; background:color-mix(in srgb,var(--brand) 22%,transparent); }
  .timeline-playhead-layer { position:absolute; z-index:4; top:0; right:0; bottom:0; left:var(--timeline-label-width); overflow:clip; pointer-events:none; }
  .timeline-playhead { position:absolute; top:0; bottom:0; left:clamp(0px,calc(var(--playhead-position) - 1px),calc(100% - 1px)); width:1px; background:#d14b65; }
  .timeline-playhead::before { content:""; position:absolute; top:0; left:-4px; border-left:4px solid transparent; border-right:4px solid transparent; border-top:6px solid #d14b65; }
  .timeline-playhead.at-start::before { left:0; border-left-width:0; border-right-width:8px; }
  .timeline-playhead.at-end::before { left:-7px; border-right-width:0; border-left-width:8px; }
  .marker { position:absolute; z-index:3; top:0; bottom:0; width:7px; margin-left:-3px; padding:0; border:0; border-left:1px dashed #d99735; background:transparent; cursor:pointer; }
  .selection-toolbar { min-height:34px; padding:3px 7px; border-top:1px solid var(--border-2); background:var(--surface-2); }
  .selection-toolbar strong { font-size:11px; }
  .selection-toolbar > span { flex:1; color:var(--text-muted); font:11px "JetBrains Mono",monospace; }
  .selection-toolbar .danger { color:var(--danger); }
  .timeline-empty { display:flex; flex-direction:column; align-items:center; justify-content:center; gap:5px; min-height:110px; padding:18px; text-align:center; color:var(--text-muted); }
  .timeline-empty strong { color:var(--text); font-size:11px; }
  .timeline-empty span { max-width:520px; font-size:11px; line-height:1.45; }
  :global(body.motion-timeline-dragging) { cursor:grabbing; user-select:none; }
  :global(body.motion-timeline-seeking) { cursor:ew-resize; user-select:none; }
  @media (max-width:900px) {
    .timeline-toolbar { align-items:flex-start; }
    .timeline-tools { max-width:55%; }
    .timeline-grid { --timeline-label-width:120px; }
  }
</style>
