<script lang="ts">
  import {
    IconBolt,
    IconArrowRight,
    IconChevronRight,
    IconCode,
    IconCopy,
    IconGripVertical,
    IconLayoutGrid,
    IconPlus,
    IconTimeline,
    IconTrash,
  } from "@tabler/icons-svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { MotionWorkspaceState } from "$lib/state/motion-workspace.svelte";
  import {
    actionTargetsDataAnim,
    actionSpan,
    behaviorTouchesDataAnim,
    createAnimateAction,
    createMotionInteraction,
    defaultMotionTrigger,
    interactionDuration,
    interactionTargetsDataAnim,
    interactionTouchesDataAnim,
    interactionTriggeredByDataAnim,
    motionId,
    motionValue,
    targetForDataAnim,
    triggerTarget,
    type MotionPreset,
  } from "$lib/js/motion-v2";
  import type {
    MotionAction,
    MotionAnimateAction,
    MotionBehavior,
    MotionInteraction,
    MotionKeyframe,
    MotionProperty,
    MotionSetAction,
    MotionTargetKind,
    MotionTrigger,
    MotionTriggerCommand,
  } from "$lib/types";

  let {
    workspace,
    dataAnim,
  }: {
    workspace: MotionWorkspaceState;
    dataAnim: string;
  } = $props();

  let creating = $state(false);
  let createTrigger = $state<MotionTrigger["type"]>("inView");
  let createPreset = $state<MotionPreset>("fade");
  let expandedAdvanced = $state(false);
  let customOpen = $state(false);
  let newActionType = $state<MotionAction["type"]>("animate");

  const triggeredInteractions = $derived.by(() => workspace.interactions.filter(
    (interaction) => interactionTriggeredByDataAnim(interaction, dataAnim),
  ));
  const targetedInteractions = $derived.by(() => workspace.interactions.filter(
    (interaction) => interactionTargetsDataAnim(interaction, dataAnim),
  ));
  const interactions = $derived.by(() => workspace.interactions.filter(
    (interaction) => interactionTouchesDataAnim(interaction, dataAnim),
  ));
  const selected = $derived.by(() => {
    const current = workspace.selectedInteraction;
    return current && (
      workspace.timelineOpen
      || interactionTouchesDataAnim(current, dataAnim)
    )
      ? current
      : interactions[0] ?? null;
  });
  const selectedOutsideElementContext = $derived(
    Boolean(selected && !interactionTouchesDataAnim(selected, dataAnim)),
  );
  const behaviors = $derived.by(() => (workspace.config.motion?.behaviors ?? []).filter(
    (behavior) => behaviorTouchesDataAnim(behavior, dataAnim),
  ));
  const customCode = $derived(workspace.config.motion?.customCode ?? []);

  $effect(() => {
    if (!selected && interactions[0]) {
      workspace.selectInteraction(interactions[0].id);
    } else if (selected && workspace.selectedInteractionId !== selected.id) {
      workspace.selectInteraction(selected.id);
    }
  });

  async function createInteraction() {
    const interaction = createMotionInteraction(dataAnim, createTrigger, createPreset);
    await workspace.mutate({ command: "createInteraction", interaction });
    workspace.openTimeline(interaction.id, interaction.actions[0]?.id ?? null);
    creating = false;
  }

  function contextualActionId(interaction: MotionInteraction) {
    return interaction.actions.find((action) =>
      actionTargetsDataAnim(action, dataAnim, interaction.triggerTarget)
    )?.id ?? interaction.actions[0]?.id ?? null;
  }

  function openContextInteraction(interaction: MotionInteraction) {
    workspace.openTimeline(interaction.id, contextualActionId(interaction));
  }

  function updateInteraction(interaction: MotionInteraction) {
    return workspace.mutate({ command: "updateInteraction", interaction });
  }

  function patchInteraction(
    interaction: MotionInteraction,
    patch: Partial<MotionInteraction>,
  ) {
    return updateInteraction({ ...structuredClone(interaction), ...patch });
  }

  function changeTrigger(interaction: MotionInteraction, type: MotionTrigger["type"]) {
    const currentDuration = interactionDuration(interaction);
    const nextTrigger = defaultMotionTrigger(type);
    const progress = type === "pointer"
      || (nextTrigger.type === "scroll" && nextTrigger.mode === "scrub");
    const nextDomain = progress ? "progress" : "time";
    let actions = structuredClone(interaction.actions);
    if (nextDomain !== interaction.domain) {
      actions = actions.map((action) => {
        const scale = nextDomain === "progress"
          ? 100 / Math.max(1, currentDuration)
          : 10;
        const next = { ...action, start: Math.round(action.start * scale * 100) / 100 };
        return (
          next.type === "animate" || next.type === "nested"
        ) ? { ...next, duration: Math.max(1, Math.round(next.duration * scale * 100) / 100) } : next;
      });
    }
    if (nextDomain === "progress" && !canUseProgressDomain(interaction)) return;
    return patchInteraction(interaction, {
      trigger: nextTrigger,
      domain: nextDomain,
      actions,
      playback: playbackForDomain(interaction, nextDomain),
    });
  }

  function updateTrigger(
    interaction: MotionInteraction,
    trigger: MotionTrigger,
  ) {
    const domain = trigger.type === "pointer"
      || (trigger.type === "scroll" && trigger.mode === "scrub")
      ? "progress"
      : "time";
    let actions = structuredClone(interaction.actions);
    if (domain === "progress" && !canUseProgressDomain(interaction)) return;
    if (domain !== interaction.domain) {
      const scale = domain === "progress"
        ? 100 / Math.max(1, interactionDuration(interaction))
        : 10;
      actions = actions.map((action) => {
        const next = { ...action, start: Math.round(action.start * scale * 100) / 100 };
        return next.type === "animate" || next.type === "nested"
          ? { ...next, duration: Math.max(1, Math.round(next.duration * scale * 100) / 100) }
          : next;
      });
    }
    return patchInteraction(interaction, {
      trigger,
      domain,
      actions,
      playback: playbackForDomain(interaction, domain),
    });
  }

  function canUseProgressDomain(interaction: MotionInteraction) {
    const incompatible = interaction.actions.find((action) => (
      action.type === "media"
      || action.type === "call"
      || (
        action.type === "animate"
        && (
          action.repeat.count > 0
          || action.repeat.infinite
          || action.repeat.alternate
          || action.repeat.delayMs > 0
        )
      )
      || (
        action.type === "set"
        && action.values.some((value) => value.type !== "property")
      )
      || (
        action.type === "nested"
        && interactionHasProgressSideEffects(action.interactionId, new Set())
      )
    ));
    if (!incompatible) return true;
    workspace.error = t("motion-error-progress-side-effects", { name: incompatible.name });
    return false;
  }

  function interactionHasProgressSideEffects(
    interactionId: string,
    visited: Set<string>,
  ): boolean {
    if (visited.has(interactionId)) return false;
    visited.add(interactionId);
    const interaction = workspace.interactions.find((candidate) => candidate.id === interactionId);
    return interaction?.actions.some((action) => (
      action.type === "media"
      || action.type === "call"
      || (
        action.type === "set"
        && action.values.some((value) => value.type !== "property")
      )
      || (
        action.type === "nested"
        && interactionHasProgressSideEffects(action.interactionId, visited)
      )
    )) ?? false;
  }

  function playbackForDomain(
    interaction: MotionInteraction,
    domain: MotionInteraction["domain"],
  ) {
    if (domain !== "progress") return structuredClone(interaction.playback);
    return {
      ...structuredClone(interaction.playback),
      delayMs: 0,
      repeat: 0,
      infinite: false,
      loopDelayMs: 0,
      alternate: false,
    };
  }

  function updateAction(interactionId: string, action: MotionAction) {
    return workspace.mutate({ command: "updateAction", interactionId, action });
  }

  function currentSelectionActionTarget(interaction: MotionInteraction) {
    return (
      interaction.triggerTarget.kind === "element"
      && interaction.triggerTarget.dataAnim === dataAnim
    )
      ? triggerTarget()
      : targetForDataAnim(dataAnim);
  }

  function newAction(interaction: MotionInteraction): MotionAction | null {
    if (
      interaction.domain === "progress"
      && (newActionType === "media" || newActionType === "call")
    ) {
      workspace.error = t("motion-error-action-progress-side-effects");
      return null;
    }
    const target = currentSelectionActionTarget(interaction);
    if (newActionType === "animate") return createAnimateAction("custom", target);
    if (newActionType === "set") {
      return {
        type: "set",
        id: motionId("action"),
        name: t("motion-default-set-name"),
        enabled: true,
        target,
        start: 0,
        values: [{
          type: "property",
          name: "opacity",
          value: motionValue("1"),
        }],
      };
    }
    if (newActionType === "media") {
      return {
        type: "media",
        id: motionId("action"),
        name: t("motion-default-media-name"),
        enabled: true,
        target,
        start: 0,
        command: "play",
      };
    }
    if (newActionType === "call") {
      return {
        type: "call",
        id: motionId("action"),
        name: t("motion-default-call-name"),
        enabled: true,
        start: 0,
        code: "",
      };
    }
    const nested = workspace.interactions.find((candidate) => candidate.id !== interaction.id);
    if (!nested) {
      workspace.error = t("motion-error-nested-required");
      return null;
    }
    return {
      type: "nested",
      id: motionId("action"),
      name: t("motion-default-nested-name", { name: nested.name }),
      enabled: true,
      start: 0,
      duration: interaction.domain === "progress" ? 20 : 600,
      interactionId: nested.id,
    };
  }

  async function addAction(interaction: MotionInteraction) {
    const action = newAction(interaction);
    if (!action) return;
    const last = interaction.actions.at(-1);
    action.start = last
      ? last.start + actionSpan(last)
      : 0;
    if (interaction.domain === "progress" && (action.type === "animate" || action.type === "nested")) {
      action.start = Math.min(80, action.start);
      action.duration = Math.min(20, 100 - action.start);
    }
    await workspace.mutate({
      command: "insertAction",
      interactionId: interaction.id,
      index: interaction.actions.length,
      action,
    });
    workspace.selectInteraction(interaction.id, action.id);
    if (interaction.actions.length >= 1) workspace.openTimeline(interaction.id, action.id);
  }

  async function duplicateAction(interaction: MotionInteraction, action: MotionAction) {
    const copy = structuredClone(action);
    copy.id = motionId("action");
    copy.name = `${action.name} copy`;
    copy.start += action.type === "animate" || action.type === "nested" ? actionSpan(action) : 100;
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

  function deleteAction(interaction: MotionInteraction, action: MotionAction) {
    if (interaction.actions.length === 1) {
      return workspace.mutate({ command: "deleteInteraction", interactionId: interaction.id });
    }
    return workspace.mutate({
      command: "deleteAction",
      interactionId: interaction.id,
      actionId: action.id,
    });
  }

  function patchAnimate(
    interaction: MotionInteraction,
    action: MotionAnimateAction,
    patch: Partial<MotionAnimateAction>,
  ) {
    return updateAction(interaction.id, {
      ...structuredClone(action),
      ...patch,
    });
  }

  function updateProperty(
    interaction: MotionInteraction,
    action: MotionAnimateAction,
    property: MotionProperty,
  ) {
    return patchAnimate(interaction, action, {
      properties: action.properties.map((item) => item.id === property.id ? property : item),
    });
  }

  function addProperty(interaction: MotionInteraction, action: MotionAnimateAction) {
    return patchAnimate(interaction, action, {
      properties: [
        ...action.properties,
        {
          id: motionId("property"),
          name: "opacity",
          category: "style",
          from: motionValue("0"),
          to: motionValue("1"),
        },
      ],
    });
  }

  function addKeyframe(interaction: MotionInteraction, action: MotionAnimateAction) {
    const lastOffset = action.keyframes.at(-1)?.offset ?? 0;
    const frame: MotionKeyframe = {
      id: motionId("keyframe"),
      offset: Math.min(100, lastOffset + 25),
      ease: "",
      properties: action.properties.map((property) => ({
        ...structuredClone(property),
        id: motionId("property"),
        from: null,
      })),
    };
    return patchAnimate(interaction, action, {
      keyframes: [...action.keyframes, frame],
    });
  }

  function updateKeyframe(
    interaction: MotionInteraction,
    action: MotionAnimateAction,
    frame: MotionKeyframe,
  ) {
    return patchAnimate(interaction, action, {
      keyframes: action.keyframes.map((candidate) =>
        candidate.id === frame.id ? frame : candidate
      ),
    });
  }

  function actionTargetFor(kind: MotionTargetKind) {
    if (kind === "element") return targetForDataAnim(dataAnim);
    if (kind === "selector") return {
      ...triggerTarget(),
      kind,
      selector: ".element",
    };
    if (kind === "relative") return {
      ...triggerTarget(),
      kind,
      relation: "children" as const,
      selector: "*",
    };
    return { ...triggerTarget(), kind };
  }

  function patchActionTarget(
    interaction: MotionInteraction,
    action: Exclude<MotionAction, { type: "call" | "nested" }>,
    patch: Partial<typeof action.target>,
  ) {
    return updateAction(interaction.id, {
      ...structuredClone(action),
      target: { ...action.target, ...patch },
    });
  }

  function updateSetValue(
    interaction: MotionInteraction,
    action: MotionSetAction,
    index: number,
    patch: Partial<Extract<MotionSetAction["values"][number], { type: "property" }>>,
  ) {
    const values = structuredClone(action.values);
    const value = values[index];
    if (!value || value.type !== "property") return;
    values[index] = { ...value, ...patch };
    return updateAction(interaction.id, { ...structuredClone(action), values });
  }

  function replaceSetValue(
    interaction: MotionInteraction,
    action: MotionSetAction,
    index: number,
    value: MotionSetAction["values"][number],
  ) {
    const values = structuredClone(action.values);
    values[index] = value;
    return updateAction(interaction.id, { ...structuredClone(action), values });
  }

  function addSetValue(
    interaction: MotionInteraction,
    action: MotionSetAction,
    type: MotionSetAction["values"][number]["type"],
  ) {
    const value: MotionSetAction["values"][number] = type === "property"
      ? { type, name: "opacity", value: motionValue("1") }
      : type === "attribute"
        ? { type, name: "aria-hidden", value: "true" }
        : { type, name: "class" };
    return updateAction(interaction.id, {
      ...structuredClone(action),
      values: [...action.values, value],
    });
  }

  function addMediaCondition(interaction: MotionInteraction) {
    return patchInteraction(interaction, {
      conditions: {
        ...interaction.conditions,
        mediaQueries: [
          ...interaction.conditions.mediaQueries,
          {
            id: motionId("media"),
            query: "(min-width: 768px)",
            enabled: true,
          },
        ],
      },
    });
  }

  function patchMediaCondition(
    interaction: MotionInteraction,
    id: string,
    patch: Partial<MotionInteraction["conditions"]["mediaQueries"][number]>,
  ) {
    return patchInteraction(interaction, {
      conditions: {
        ...interaction.conditions,
        mediaQueries: interaction.conditions.mediaQueries.map((condition) =>
          condition.id === id ? { ...condition, ...patch } : condition
        ),
      },
    });
  }

  function createBehavior(type: MotionBehavior["type"]) {
    const behavior: MotionBehavior = type === "draggable"
      ? {
          type,
          id: motionId("behavior"),
          name: t("motion-add-drag"),
          enabled: true,
          target: targetForDataAnim(dataAnim),
          axis: "both",
          container: "",
          snap: 0,
          friction: 0.8,
          cursor: true,
        }
      : {
          type,
          id: motionId("behavior"),
          name: t("motion-default-layout-name"),
          enabled: true,
          target: targetForDataAnim(dataAnim),
          childrenSelector: "",
          properties: [],
          durationMs: 600,
          ease: "out(3)",
        };
    return workspace.mutate({ command: "upsertBehavior", behavior });
  }

  function updateBehavior(behavior: MotionBehavior) {
    return workspace.mutate({ command: "upsertBehavior", behavior });
  }

  function localizedTriggerLabel(trigger: MotionTrigger) {
    switch (trigger.type) {
      case "load": return t("motion-trigger-load");
      case "inView": return t("motion-trigger-in-view");
      case "click": return t("motion-trigger-click");
      case "hover": return t("motion-trigger-hover");
      case "scroll": return trigger.mode === "scrub"
        ? t("motion-trigger-scroll-scrub")
        : t("motion-trigger-scroll");
      case "pointer": return t("motion-trigger-pointer-move");
      case "custom": return t("motion-trigger-custom-label", {
        event: trigger.event || t("motion-preset-custom").toLocaleLowerCase(),
      });
    }
  }

  function addCustomCode() {
    const customCode = {
      id: motionId("custom"),
      name: t("motion-custom-code"),
      enabled: true,
      code: "",
    };
    customOpen = true;
    return workspace.mutate({ command: "upsertCustomCode", customCode });
  }
</script>

<section class="motion-inspector" aria-label={t("motion-aria-interactions")}>
  <header class="section-head">
    <div>
      <strong>{t("motion-interactions")}</strong>
      <span>{t("motion-flow-summary")}</span>
    </div>
    <button class="primary-icon" type="button" title={t("motion-add-interaction")} onclick={() => { creating = !creating; }}>
      <IconPlus size={15} stroke={2.2} />
    </button>
  </header>

  {#if workspace.error}
    <div class="motion-error" role="alert">{workspace.error}</div>
  {/if}

  {#if creating}
    <div class="create-card">
      <label>
        <span>{t("motion-when")}</span>
        <select bind:value={createTrigger}>
          <option value="load">{t("motion-trigger-load")}</option>
          <option value="inView">{t("motion-trigger-in-view")}</option>
          <option value="click">{t("motion-trigger-click")}</option>
          <option value="hover">{t("motion-trigger-hover")}</option>
          <option value="scroll">{t("motion-trigger-scroll-scrub")}</option>
          <option value="pointer">{t("motion-trigger-pointer")}</option>
        </select>
      </label>
      <div class="preset-grid" aria-label={t("motion-preset-label")}>
        {#each [
          ["fade", t("motion-preset-fade")],
          ["slideUp", t("motion-preset-slide-up")],
          ["scale", t("motion-preset-scale")],
          ["custom", t("motion-preset-custom")],
        ] as [id, label]}
          <button
            type="button"
            class:active={createPreset === id}
            onclick={() => { createPreset = id as MotionPreset; }}
          >{label}</button>
        {/each}
      </div>
      <div class="create-actions">
        <button type="button" onclick={() => { creating = false; }}>{t("motion-cancel")}</button>
        <button class="primary" type="button" disabled={workspace.pendingCount > 0} onclick={() => { void createInteraction(); }}>
          {t("motion-create")}
        </button>
      </div>
    </div>
  {/if}

  {#if triggeredInteractions.length > 0 || targetedInteractions.length > 0}
    <div class="element-context">
      {#if triggeredInteractions.length > 0}
        <section class="context-group">
          <header>
            <strong>{t("motion-triggers")}</strong>
            <span>{triggeredInteractions.length}</span>
          </header>
          {#each triggeredInteractions as item (item.id)}
            <button
              type="button"
              class="ui-entity-selectable"
              data-ui-selected={selected?.id === item.id ? "true" : undefined}
              aria-pressed={selected?.id === item.id}
              onclick={() => openContextInteraction(item)}
            >
              <span>
                <strong>{item.name}</strong>
                <small>{localizedTriggerLabel(item.trigger)}</small>
              </span>
              <IconTimeline size={14} />
            </button>
          {/each}
        </section>
      {/if}

      {#if targetedInteractions.length > 0}
        <section class="context-group">
          <header>
            <strong>{t("motion-animated-by")}</strong>
            <span>{targetedInteractions.length}</span>
          </header>
          {#each targetedInteractions as item (item.id)}
            {@const targetActionCount = item.actions.filter((action) =>
              actionTargetsDataAnim(action, dataAnim, item.triggerTarget)
            ).length}
            <button
              type="button"
              class="ui-entity-selectable"
              data-ui-selected={selected?.id === item.id ? "true" : undefined}
              aria-pressed={selected?.id === item.id}
              onclick={() => openContextInteraction(item)}
            >
              <span>
                <strong>{item.name}</strong>
                <small>{t("motion-action-count", { count: targetActionCount })}</small>
              </span>
              <IconTimeline size={14} />
            </button>
          {/each}
        </section>
      {/if}
    </div>
  {/if}

  {#if selectedOutsideElementContext && selected}
    <div class="active-timeline-context">
      <strong>{t("motion-active-timeline", { name: selected.name })}</strong>
      <span>{t("motion-outside-context")}</span>
    </div>
  {/if}

  {#if selected}
    <article class="interaction-card">
      <div class="interaction-title">
        <button
          type="button"
          class="enabled-dot"
          class:off={!selected.enabled}
          title={selected.enabled ? t("motion-disable") : t("motion-enable")}
          onclick={() => { void patchInteraction(selected, { enabled: !selected.enabled }); }}
        ></button>
        <input
          aria-label={t("motion-interaction-name")}
          value={selected.name}
          onchange={(event) => { void patchInteraction(selected, { name: event.currentTarget.value }); }}
        />
        <button type="button" title={t("motion-delete-interaction")} onclick={() => {
          void workspace.mutate({ command: "deleteInteraction", interactionId: selected.id });
        }}>
          <IconTrash size={14} stroke={1.9} />
        </button>
      </div>

      <div class="flow-card">
        <div class="flow-step">
          <span class="step-index">1</span>
          <label>
            <small>{t("motion-trigger")}</small>
            <select
              value={selected.trigger.type}
              onchange={(event) => {
                void changeTrigger(selected, event.currentTarget.value as MotionTrigger["type"]);
              }}
            >
              <option value="load">{t("motion-trigger-load")}</option>
              <option value="inView">{t("motion-trigger-in-view")}</option>
              <option value="click">{t("motion-trigger-click")}</option>
              <option value="hover">{t("motion-trigger-hover")}</option>
              <option value="scroll">{t("motion-trigger-scroll")}</option>
              <option value="pointer">{t("motion-trigger-pointer")}</option>
              <option value="custom">{t("motion-trigger-custom")}</option>
            </select>
          </label>
        </div>

        {#if selected.trigger.type === "load"}
          <div class="sub-grid">
            <label><span>{t("motion-moment")}</span><select value={selected.trigger.phase} onchange={(event) => {
              if (selected.trigger.type === "load") void updateTrigger(selected, {
                ...selected.trigger,
                phase: event.currentTarget.value as "domReady" | "windowLoad",
              });
            }}><option value="domReady">{t("motion-dom-ready")}</option><option value="windowLoad">{t("motion-window-loaded")}</option></select></label>
          </div>
        {:else if selected.trigger.type === "inView"}
          <div class="sub-grid">
            <label><span>{t("motion-threshold")}</span><input type="number" min="0" max="1" step="0.05" value={selected.trigger.threshold} onchange={(event) => {
              if (selected.trigger.type === "inView") void updateTrigger(selected, {
                ...selected.trigger,
                threshold: Number(event.currentTarget.value),
              });
            }} /></label>
            <label class="check"><input type="checkbox" checked={selected.trigger.once} onchange={(event) => {
              if (selected.trigger.type === "inView") void updateTrigger(selected, {
                ...selected.trigger,
                once: event.currentTarget.checked,
              });
            }} /> {t("motion-once")}</label>
          </div>
        {:else if selected.trigger.type === "click"}
          <div class="sub-grid">
            <label><span>{t("motion-first-click")}</span><select value={selected.trigger.firstClick} onchange={(event) => {
              if (selected.trigger.type === "click") void updateTrigger(selected, {
                ...selected.trigger,
                firstClick: event.currentTarget.value as MotionTriggerCommand,
              });
            }}>
              {#each ["restart", "play", "pause", "reverse", "toggle", "reset", "none"] as command}
                <option value={command}>{command}</option>
              {/each}
            </select></label>
            <label><span>{t("motion-second-click")}</span><select value={selected.trigger.secondClick} onchange={(event) => {
              if (selected.trigger.type === "click") void updateTrigger(selected, {
                ...selected.trigger,
                secondClick: event.currentTarget.value as MotionTriggerCommand,
              });
            }}>
              {#each ["restart", "play", "pause", "reverse", "toggle", "reset", "none"] as command}
                <option value={command}>{command}</option>
              {/each}
            </select></label>
            <label class="check"><input type="checkbox" checked={selected.trigger.preventDefault} onchange={(event) => {
              if (selected.trigger.type === "click") void updateTrigger(selected, {
                ...selected.trigger,
                preventDefault: event.currentTarget.checked,
              });
            }} /> {t("motion-prevent-default")}</label>
          </div>
        {:else if selected.trigger.type === "hover"}
          <div class="sub-grid">
            <label><span>{t("motion-enter")}</span><select value={selected.trigger.enter} onchange={(event) => {
              if (selected.trigger.type === "hover") void updateTrigger(selected, {
                ...selected.trigger,
                enter: event.currentTarget.value as MotionTriggerCommand,
              });
            }}>
              {#each ["restart", "play", "pause", "reverse", "toggle", "reset", "none"] as command}
                <option value={command}>{command}</option>
              {/each}
            </select></label>
            <label><span>{t("motion-leave")}</span><select value={selected.trigger.leave} onchange={(event) => {
              if (selected.trigger.type === "hover") void updateTrigger(selected, {
                ...selected.trigger,
                leave: event.currentTarget.value as MotionTriggerCommand,
              });
            }}>
              {#each ["restart", "play", "pause", "reverse", "toggle", "reset", "none"] as command}
                <option value={command}>{command}</option>
              {/each}
            </select></label>
          </div>
        {:else if selected.trigger.type === "scroll"}
          <div class="sub-grid">
            <label><span>{t("motion-mode")}</span><select value={selected.trigger.mode} onchange={(event) => {
              if (selected.trigger.type === "scroll") void updateTrigger(selected, {
                ...selected.trigger,
                mode: event.currentTarget.value as "trigger" | "scrub",
              });
            }}><option value="scrub">{t("motion-progress")}</option><option value="trigger">{t("motion-start-scroll")}</option></select></label>
            <label><span>{t("motion-smoothing-ms")}</span><input type="number" min="0" value={selected.trigger.smoothMs} onchange={(event) => {
              if (selected.trigger.type === "scroll") void updateTrigger(selected, {
                ...selected.trigger,
                smoothMs: Number(event.currentTarget.value),
              });
            }} /></label>
            <label><span>{t("motion-anime-start")}</span><input value={selected.trigger.start} onchange={(event) => {
              if (selected.trigger.type === "scroll") void updateTrigger(selected, {
                ...selected.trigger,
                start: event.currentTarget.value,
              });
            }} /></label>
            <label><span>{t("motion-anime-end")}</span><input value={selected.trigger.end} onchange={(event) => {
              if (selected.trigger.type === "scroll") void updateTrigger(selected, {
                ...selected.trigger,
                end: event.currentTarget.value,
              });
            }} /></label>
            {#if selected.trigger.mode === "trigger"}
              <label class="check"><input type="checkbox" checked={selected.trigger.once} onchange={(event) => {
                if (selected.trigger.type === "scroll") void updateTrigger(selected, {
                  ...selected.trigger,
                  once: event.currentTarget.checked,
                });
              }} /> {t("motion-once")}</label>
            {/if}
          </div>
        {:else if selected.trigger.type === "pointer"}
          <div class="sub-grid">
            <label><span>{t("motion-axis")}</span><select value={selected.trigger.axis} onchange={(event) => {
              if (selected.trigger.type === "pointer") void updateTrigger(selected, {
                ...selected.trigger,
                axis: event.currentTarget.value as "x" | "y" | "both",
              });
            }}><option value="x">X</option><option value="y">Y</option><option value="both">X + Y</option></select></label>
            <label><span>{t("motion-smoothing-ms")}</span><input type="number" min="0" value={selected.trigger.smoothMs} onchange={(event) => {
              if (selected.trigger.type === "pointer") void updateTrigger(selected, {
                ...selected.trigger,
                smoothMs: Number(event.currentTarget.value),
              });
            }} /></label>
            <label><span>{t("motion-rest")}</span><input type="number" min="0" max="1" step="0.05" value={selected.trigger.rest} onchange={(event) => {
              if (selected.trigger.type === "pointer") void updateTrigger(selected, {
                ...selected.trigger,
                rest: Number(event.currentTarget.value),
              });
            }} /></label>
          </div>
        {:else if selected.trigger.type === "custom"}
          <div class="sub-grid">
            <label><span>{t("motion-dom-event")}</span><input value={selected.trigger.event} onchange={(event) => {
              if (selected.trigger.type === "custom") void updateTrigger(selected, {
                ...selected.trigger,
                event: event.currentTarget.value,
              });
            }} /></label>
            <label class="check"><input type="checkbox" checked={selected.trigger.preventDefault} onchange={(event) => {
              if (selected.trigger.type === "custom") void updateTrigger(selected, {
                ...selected.trigger,
                preventDefault: event.currentTarget.checked,
              });
            }} /> {t("motion-prevent-default")}</label>
          </div>
        {/if}

        <div class="flow-connector"><IconChevronRight size={14} /></div>
        <div class="flow-step">
          <span class="step-index">2</span>
          <div class="trigger-target-editor">
            <small>{t("motion-trigger-target")}</small>
            <select value={selected.triggerTarget.kind} onchange={(event) => {
              void patchInteraction(selected, {
                triggerTarget: actionTargetFor(event.currentTarget.value as MotionTargetKind),
              });
            }}>
              <option value="element">{t("motion-target-selected")}</option>
              <option value="selector">{t("motion-target-selector")}</option>
              <option value="document">{t("motion-target-document")}</option>
              <option value="viewport">{t("motion-target-viewport")}</option>
            </select>
            {#if selected.triggerTarget.kind === "selector"}
              <input value={selected.triggerTarget.selector} placeholder=".selector" onchange={(event) => {
                void patchInteraction(selected, {
                  triggerTarget: { ...selected.triggerTarget, selector: event.currentTarget.value },
                });
              }} />
            {:else if selected.triggerTarget.kind === "element"}
              <strong>[data-anim="{selected.triggerTarget.dataAnim}"]</strong>
            {/if}
          </div>
        </div>
      </div>

      <div class="actions-head">
        <div>
          <strong>{t("motion-actions")}</strong>
          <span>{selected.domain === "progress" ? t("motion-progress") : t("motion-domain-time")}</span>
        </div>
        <div class="add-action-control">
          <select bind:value={newActionType} aria-label={t("motion-new-action-type")}>
            <option value="animate">{t("motion-action-animate")}</option>
            <option value="set">{t("motion-action-set")}</option>
            <option value="media" disabled={selected.domain === "progress"}>{t("motion-action-media-advanced")}</option>
            <option value="call" disabled={selected.domain === "progress"}>{t("motion-action-call-advanced")}</option>
            <option value="nested">{t("motion-action-nested")}</option>
          </select>
          <button type="button" onclick={() => { void addAction(selected); }}><IconPlus size={14} /> {t("motion-add")}</button>
        </div>
      </div>

      <div class="action-list">
        {#each selected.actions as action, index (action.id)}
          <details
            class="action-card"
            open={workspace.selectedActionId === action.id}
            ontoggle={(event) => {
              if (event.currentTarget.open) workspace.selectInteraction(selected.id, action.id);
            }}
          >
            <summary>
              <IconGripVertical size={14} />
              <span class="action-index">{index + 1}</span>
              <strong>{action.name}</strong>
              <small>{action.start}{selected.domain === "progress" ? "%" : " ms"}</small>
            </summary>
            <div class="action-editor">
              <div class="field-grid">
                <label><span>{t("motion-action-name")}</span><input value={action.name} onchange={(event) => {
                  void updateAction(selected.id, { ...structuredClone(action), name: event.currentTarget.value });
                }} /></label>
                <label><span>{t("motion-action-target")}</span><select
                  value={"target" in action ? action.target.kind : "trigger"}
                  disabled={!("target" in action)}
                  onchange={(event) => {
                    if (!("target" in action)) return;
                    void updateAction(selected.id, {
                      ...structuredClone(action),
                      target: actionTargetFor(event.currentTarget.value as MotionTargetKind),
                    });
                  }}
                >
                  <option value="trigger">{t("motion-target-trigger")}</option>
                  <option value="element">{t("motion-target-selected")}</option>
                  <option value="relative">{t("motion-target-relative")}</option>
                  <option value="selector">{t("motion-target-selector")}</option>
                  <option value="viewport">{t("motion-target-viewport")}</option>
                  <option value="document">{t("motion-target-document")}</option>
                </select></label>
                <label><span>{t("motion-start")} {selected.domain === "progress" ? "%" : "ms"}</span><input type="number" min="0" value={action.start} onchange={(event) => {
                  void workspace.mutate({
                    command: "setActionTiming",
                    interactionId: selected.id,
                    actionId: action.id,
                    start: Number(event.currentTarget.value),
                  });
                }} /></label>
                {#if action.type === "animate" || action.type === "nested"}
                  <label><span>{t("motion-duration", { unit: selected.domain === "progress" ? "%" : "ms" })}</span><input type="number" min="1" value={action.duration} onchange={(event) => {
                    void workspace.mutate({
                      command: "setActionTiming",
                      interactionId: selected.id,
                      actionId: action.id,
                      duration: Number(event.currentTarget.value),
                    });
                  }} /></label>
                {/if}
              </div>

              {#if "target" in action && (action.target.kind === "selector" || action.target.kind === "relative")}
                <div class="field-grid">
                  <label><span>{t("motion-target-selector")}</span><input value={action.target.selector} placeholder=".element" onchange={(event) => {
                    if ("target" in action) void patchActionTarget(selected, action, {
                      selector: event.currentTarget.value,
                    });
                  }} /></label>
                  {#if action.target.kind === "relative"}
                    <label><span>{t("motion-target-relation")}</span><select value={action.target.relation} onchange={(event) => {
                      if ("target" in action) void patchActionTarget(selected, action, {
                        relation: event.currentTarget.value as typeof action.target.relation,
                      });
                    }}>
                      <option value="children">{t("motion-relation-children")}</option>
                      <option value="descendants">{t("motion-relation-descendants")}</option>
                      <option value="parent">{t("motion-relation-parent")}</option>
                      <option value="ancestors">{t("motion-relation-ancestors")}</option>
                      <option value="siblings">{t("motion-relation-siblings")}</option>
                      <option value="nextSibling">{t("motion-relation-next")}</option>
                      <option value="previousSibling">{t("motion-relation-previous")}</option>
                    </select></label>
                  {/if}
                  <label><span>{t("motion-application")}</span><select value={action.target.scope} onchange={(event) => {
                    if ("target" in action) void patchActionTarget(selected, action, {
                      scope: event.currentTarget.value as typeof action.target.scope,
                    });
                  }}><option value="all">{t("motion-all")}</option><option value="first">{t("motion-first")}</option><option value="each">{t("motion-each")}</option></select></label>
                </div>
              {/if}

              {#if action.type === "animate"}
                <div class="field-grid">
                  <label><span>{t("motion-animation-model")}</span><select value={action.mode} onchange={(event) => {
                    void patchAnimate(selected, action, {
                      mode: event.currentTarget.value as MotionAnimateAction["mode"],
                    });
                  }}><option value="fromTo">{t("motion-model-from-to")}</option><option value="to">{t("motion-model-to")}</option><option value="from">{t("motion-model-from")}</option></select></label>
                  <label><span>{t("motion-easing")} Anime.js</span><input value={action.ease} onchange={(event) => {
                    void patchAnimate(selected, action, { ease: event.currentTarget.value });
                  }} /></label>
                </div>

                <div class="property-list">
                  {#each action.properties as property (property.id)}
                    <div class="property-row">
                      <select value={property.name} onchange={(event) => {
                        const name = event.currentTarget.value;
                        const category = ["opacity", "color", "backgroundColor"].includes(name)
                          ? "style"
                          : "transform";
                        void updateProperty(selected, action, { ...property, name, category });
                      }}>
                        <option value="opacity">opacity</option>
                        <option value="translateX">translateX</option>
                        <option value="translateY">translateY</option>
                        <option value="scale">scale</option>
                        <option value="rotate">rotate</option>
                        <option value="color">color</option>
                        <option value="backgroundColor">backgroundColor</option>
                        <option value="width">width</option>
                        <option value="height">height</option>
                      </select>
                      <input aria-label={t("motion-initial-value")} value={property.from?.value ?? ""} placeholder={t("motion-from-placeholder")} onchange={(event) => {
                        void updateProperty(selected, action, {
                          ...property,
                          from: { ...(property.from ?? motionValue("")), value: event.currentTarget.value },
                        });
                      }} />
                      <IconArrowRight class="value-arrow" size={13} stroke={1.8} aria-hidden="true" />
                      <input aria-label={t("motion-final-value")} value={property.to.value} placeholder={t("motion-to-placeholder")} onchange={(event) => {
                        void updateProperty(selected, action, {
                          ...property,
                          to: { ...property.to, value: event.currentTarget.value },
                        });
                      }} />
                      <input class="unit" aria-label={t("motion-unit")} value={property.to.unit} placeholder={t("motion-unit-short")} onchange={(event) => {
                        const unit = event.currentTarget.value;
                        void updateProperty(selected, action, {
                          ...property,
                          from: property.from ? { ...property.from, unit } : null,
                          to: { ...property.to, unit },
                        });
                      }} />
                      <button type="button" title={t("motion-delete-property")} onclick={() => {
                        void patchAnimate(selected, action, {
                          properties: action.properties.filter((item) => item.id !== property.id),
                        });
                      }}><IconTrash size={13} /></button>
                    </div>
                  {/each}
                  <button class="add-row" type="button" onclick={() => { void addProperty(selected, action); }}>
                    <IconPlus size={13} /> {t("motion-add-property")}
                  </button>
                </div>

                <details class="advanced-action">
                  <summary>{t("motion-advanced")}</summary>
                  <div class="field-grid">
                    <label><span>{t("motion-stagger")}</span><input type="number" min="0" value={action.stagger?.amount ?? 0} onchange={(event) => {
                      const amount = Number(event.currentTarget.value);
                      void patchAnimate(selected, action, {
                        stagger: amount > 0 ? {
                          amount,
                          mode: action.stagger?.mode ?? "each",
                          from: action.stagger?.from ?? "first",
                          reversed: action.stagger?.reversed ?? false,
                          ease: action.stagger?.ease ?? "",
                        } : null,
                      });
                    }} /></label>
                    {#if action.stagger}
                      <label><span>{t("motion-stagger-calculation")}</span><select value={action.stagger.mode} onchange={(event) => {
                        if (action.stagger) void patchAnimate(selected, action, {
                          stagger: { ...action.stagger, mode: event.currentTarget.value as "each" | "total" },
                        });
                      }}><option value="each">{t("motion-between-elements")}</option><option value="total">{t("motion-total-duration")}</option></select></label>
                      <label><span>{t("motion-from")}</span><input value={action.stagger.from} placeholder={t("motion-stagger-from-placeholder")} onchange={(event) => {
                        if (action.stagger) void patchAnimate(selected, action, {
                          stagger: { ...action.stagger, from: event.currentTarget.value },
                        });
                      }} /></label>
                      <label><span>{t("motion-stagger-easing")}</span><input value={action.stagger.ease} placeholder="linear" onchange={(event) => {
                        if (action.stagger) void patchAnimate(selected, action, {
                          stagger: { ...action.stagger, ease: event.currentTarget.value },
                        });
                      }} /></label>
                      <label class="check"><input type="checkbox" checked={action.stagger.reversed} onchange={(event) => {
                        if (action.stagger) void patchAnimate(selected, action, {
                          stagger: { ...action.stagger, reversed: event.currentTarget.checked },
                        });
                      }} /> {t("motion-reverse-order")}</label>
                    {/if}
                    <label><span>{t("motion-repeats")}</span><input type="number" min="0" value={action.repeat.count} disabled={selected.domain === "progress"} onchange={(event) => {
                      void patchAnimate(selected, action, {
                        repeat: { ...action.repeat, count: Number(event.currentTarget.value) },
                      });
                    }} /></label>
                    <label><span>{t("motion-repeat-pause")}</span><input type="number" min="0" value={action.repeat.delayMs} disabled={selected.domain === "progress"} onchange={(event) => {
                      void patchAnimate(selected, action, {
                        repeat: { ...action.repeat, delayMs: Number(event.currentTarget.value) },
                      });
                    }} /></label>
                    <label class="check"><input type="checkbox" checked={action.repeat.infinite} disabled={selected.domain === "progress"} onchange={(event) => {
                      void patchAnimate(selected, action, {
                        repeat: { ...action.repeat, infinite: event.currentTarget.checked },
                      });
                    }} /> {t("motion-repeat-infinite")}</label>
                    <label class="check"><input type="checkbox" checked={action.repeat.alternate} disabled={selected.domain === "progress"} onchange={(event) => {
                      void patchAnimate(selected, action, {
                        repeat: { ...action.repeat, alternate: event.currentTarget.checked },
                      });
                    }} /> {t("motion-alternate")}</label>
                    <label><span>{t("motion-specialization")}</span><select value={action.specialization?.type ?? ""} onchange={(event) => {
                      const type = event.currentTarget.value;
                      void patchAnimate(selected, action, {
                        specialization: type === "splitText"
                          ? { type, mode: "chars" }
                          : type === "svgDraw"
                            ? { type }
                            : type === "svgPath"
                              ? { type, path: "path", autoRotate: true }
                              : type === "svgMorph"
                                ? { type, source: "path", precision: 0.33 }
                                : null,
                      });
                    }}>
                      <option value="">{t("motion-none")}</option>
                      <option value="splitText">{t("motion-split-text")}</option>
                      <option value="svgDraw">{t("motion-svg-draw")}</option>
                      <option value="svgPath">{t("motion-svg-path")}</option>
                      <option value="svgMorph">{t("motion-svg-morph")}</option>
                    </select></label>
                    {#if action.specialization?.type === "splitText"}
                      <label><span>{t("motion-segmentation")}</span><select value={action.specialization.mode} onchange={(event) => {
                        if (action.specialization?.type === "splitText") void patchAnimate(selected, action, {
                          specialization: { ...action.specialization, mode: event.currentTarget.value as "lines" | "words" | "chars" },
                        });
                      }}><option value="chars">{t("motion-characters")}</option><option value="words">{t("motion-words")}</option><option value="lines">{t("motion-lines")}</option></select></label>
                    {:else if action.specialization?.type === "svgPath"}
                      <label><span>{t("motion-path-selector")}</span><input value={action.specialization.path} onchange={(event) => {
                        if (action.specialization?.type === "svgPath") void patchAnimate(selected, action, {
                          specialization: { ...action.specialization, path: event.currentTarget.value },
                        });
                      }} /></label>
                      <label class="check"><input type="checkbox" checked={action.specialization.autoRotate} onchange={(event) => {
                        if (action.specialization?.type === "svgPath") void patchAnimate(selected, action, {
                          specialization: { ...action.specialization, autoRotate: event.currentTarget.checked },
                        });
                      }} /> {t("motion-rotate-along-path")}</label>
                    {:else if action.specialization?.type === "svgMorph"}
                      <label><span>{t("motion-source-shape-selector")}</span><input value={action.specialization.source} onchange={(event) => {
                        if (action.specialization?.type === "svgMorph") void patchAnimate(selected, action, {
                          specialization: { ...action.specialization, source: event.currentTarget.value },
                        });
                      }} /></label>
                      <label><span>{t("motion-precision")}</span><input type="number" min="0" step="0.05" value={action.specialization.precision} onchange={(event) => {
                        if (action.specialization?.type === "svgMorph") void patchAnimate(selected, action, {
                          specialization: { ...action.specialization, precision: Number(event.currentTarget.value) },
                        });
                      }} /></label>
                    {/if}
                  </div>
                  <div class="keyframes-editor">
                    <div class="media-conditions-head">
                      <strong>{t("motion-keyframes")}</strong>
                      <button type="button" onclick={() => { void addKeyframe(selected, action); }}>
                        <IconPlus size={12} stroke={1.9} aria-hidden="true" />
                        {t("motion-add-keyframe")}
                      </button>
                    </div>
                    {#each action.keyframes as frame (frame.id)}
                      <div class="keyframe-row">
                        <label><span>{t("motion-position-percent")}</span><input type="number" min="0" max="100" value={frame.offset} onchange={(event) => {
                          void updateKeyframe(selected, action, {
                            ...frame,
                            offset: Number(event.currentTarget.value),
                          });
                        }} /></label>
                        <label><span>{t("motion-easing")}</span><input value={frame.ease} placeholder={t("motion-inherited")} onchange={(event) => {
                          void updateKeyframe(selected, action, {
                            ...frame,
                            ease: event.currentTarget.value,
                          });
                        }} /></label>
                        <div class="keyframe-properties">
                          {#each frame.properties as property (property.id)}
                            <label><span>{property.name}</span><input value={property.to.value} onchange={(event) => {
                              void updateKeyframe(selected, action, {
                                ...frame,
                                properties: frame.properties.map((candidate) =>
                                  candidate.id === property.id
                                    ? { ...candidate, to: { ...candidate.to, value: event.currentTarget.value } }
                                    : candidate
                                ),
                              });
                            }} /></label>
                          {/each}
                        </div>
                        <button type="button" title={t("motion-delete-keyframe")} onclick={() => {
                          void patchAnimate(selected, action, {
                            keyframes: action.keyframes.filter((candidate) => candidate.id !== frame.id),
                          });
                        }}><IconTrash size={13} /></button>
                      </div>
                    {/each}
                  </div>
                </details>
              {:else if action.type === "set"}
                <div class="set-values">
                  {#each action.values as value, valueIndex}
                    <div class="set-value-row">
                      <select value={value.type} onchange={(event) => {
                        const type = event.currentTarget.value as MotionSetAction["values"][number]["type"];
                        const replacement: MotionSetAction["values"][number] = type === "property"
                          ? { type, name: "opacity", value: motionValue("1") }
                          : type === "attribute"
                            ? { type, name: "aria-hidden", value: "true" }
                            : { type, name: "class" };
                        void replaceSetValue(selected, action, valueIndex, replacement);
                      }}>
                        <option value="property">{t("motion-set-property")}</option>
                        <option value="attribute" disabled={selected.domain === "progress"}>{t("motion-set-attribute")}</option>
                        <option value="addClass" disabled={selected.domain === "progress"}>{t("motion-set-add-class")}</option>
                        <option value="removeClass" disabled={selected.domain === "progress"}>{t("motion-set-remove-class")}</option>
                        <option value="toggleClass" disabled={selected.domain === "progress"}>{t("motion-set-toggle-class")}</option>
                      </select>
                      <input value={value.name} placeholder={t("motion-name-placeholder")} onchange={(event) => {
                        void replaceSetValue(selected, action, valueIndex, {
                          ...value,
                          name: event.currentTarget.value,
                        });
                      }} />
                      {#if value.type === "property"}
                        <input value={value.value.value} placeholder={t("motion-value-placeholder")} onchange={(event) => {
                          void updateSetValue(selected, action, valueIndex, {
                            value: { ...value.value, value: event.currentTarget.value },
                          });
                        }} />
                      {:else if value.type === "attribute"}
                        <input value={value.value} placeholder={t("motion-value-placeholder")} onchange={(event) => {
                          void replaceSetValue(selected, action, valueIndex, {
                            ...value,
                            value: event.currentTarget.value,
                          });
                        }} />
                      {/if}
                      <button type="button" title={t("motion-delete-value")} onclick={() => {
                        void updateAction(selected.id, {
                          ...structuredClone(action),
                          values: action.values.filter((_, index) => index !== valueIndex),
                        });
                      }}><IconTrash size={13} /></button>
                    </div>
                  {/each}
                  <div class="set-add-buttons">
                    <button type="button" onclick={() => { void addSetValue(selected, action, "property"); }}>
                      <IconPlus size={12} stroke={1.9} aria-hidden="true" />
                      {t("motion-set-property")}
                    </button>
                    <button type="button" disabled={selected.domain === "progress"} onclick={() => { void addSetValue(selected, action, "attribute"); }}>
                      <IconPlus size={12} stroke={1.9} aria-hidden="true" />
                      {t("motion-set-attribute")}
                    </button>
                    <button type="button" disabled={selected.domain === "progress"} onclick={() => { void addSetValue(selected, action, "addClass"); }}>
                      <IconPlus size={12} stroke={1.9} aria-hidden="true" />
                      {t("motion-add-class")}
                    </button>
                  </div>
                </div>
              {:else if action.type === "media"}
                <label><span>{t("motion-media-command")}</span><select value={action.command} onchange={(event) => {
                  void updateAction(selected.id, {
                    ...structuredClone(action),
                    command: event.currentTarget.value as typeof action.command,
                  });
                }}><option value="play">{t("motion-action-play")}</option><option value="pause">{t("motion-action-pause")}</option><option value="toggle">{t("motion-action-toggle")}</option><option value="reset">{t("motion-action-reset")}</option></select></label>
              {:else if action.type === "nested"}
                <label><span>{t("motion-included-interaction")}</span><select value={action.interactionId} onchange={(event) => {
                  const interactionId = event.currentTarget.value;
                  const nested = workspace.interactions.find((candidate) => candidate.id === interactionId);
                  void updateAction(selected.id, {
                    ...structuredClone(action),
                    interactionId,
                    name: nested ? `Include ${nested.name}` : action.name,
                  });
                }}>
                  {#each workspace.interactions.filter((candidate) => candidate.id !== selected.id) as candidate}
                    <option value={candidate.id}>{candidate.name}</option>
                  {/each}
                </select></label>
              {:else if action.type === "call"}
                <label class="code-field"><span>{t("motion-isolated-code")}</span><textarea value={action.code} onchange={(event) => {
                  void updateAction(selected.id, { ...action, code: event.currentTarget.value });
                }}></textarea></label>
              {/if}

              <div class="action-buttons">
                <button type="button" onclick={() => { void duplicateAction(selected, action); }}><IconCopy size={13} /> {t("motion-duplicate")}</button>
                <button type="button" onclick={() => workspace.openTimeline(selected.id, action.id)}><IconTimeline size={13} /> {t("motion-sequence")}</button>
                <button class="danger" type="button" onclick={() => { void deleteAction(selected, action); }}><IconTrash size={13} /> {t("motion-delete")}</button>
              </div>
            </div>
          </details>
        {/each}
      </div>

      <button class="timeline-button" type="button" onclick={() => workspace.openTimeline(selected.id, workspace.selectedActionId)}>
        <IconTimeline size={15} />
        {t("motion-edit-sequence")}
        <span>{t("motion-action-count", { count: selected.actions.length })}</span>
      </button>

      <details bind:open={expandedAdvanced} class="interaction-advanced">
        <summary>{t("motion-conditions-playback")}</summary>
        <div class="field-grid">
          <label><span>{t("motion-reduced-motion")}</span><select value={selected.conditions.reducedMotion} onchange={(event) => {
            void patchInteraction(selected, {
              conditions: {
                ...selected.conditions,
                reducedMotion: event.currentTarget.value as MotionInteraction["conditions"]["reducedMotion"],
              },
            });
          }}><option value="reduce">{t("motion-reduced-reduce")}</option><option value="skipToEnd">{t("motion-reduced-skip")}</option><option value="disable">{t("motion-reduced-disable")}</option></select></label>
          <label><span>{t("motion-speed")}</span><input type="number" min="0.1" max="4" step="0.1" value={selected.playback.playbackRate} onchange={(event) => {
            void patchInteraction(selected, {
              playback: { ...selected.playback, playbackRate: Number(event.currentTarget.value) },
            });
          }} /></label>
          <label class="check"><input type="checkbox" checked={selected.playback.infinite} disabled={selected.domain === "progress"} onchange={(event) => {
            void patchInteraction(selected, {
              playback: { ...selected.playback, infinite: event.currentTarget.checked },
            });
          }} /> {t("motion-loop")}</label>
          <label class="check"><input type="checkbox" checked={selected.playback.alternate} disabled={selected.domain === "progress"} onchange={(event) => {
            void patchInteraction(selected, {
              playback: { ...selected.playback, alternate: event.currentTarget.checked },
            });
          }} /> {t("motion-alternate")}</label>
        </div>
        <div class="media-conditions">
          <div class="media-conditions-head">
            <strong>{t("motion-responsive-conditions")}</strong>
            <button type="button" onclick={() => { void addMediaCondition(selected); }}>
              <IconPlus size={12} stroke={1.9} aria-hidden="true" />
              {t("motion-add-media-query")}
            </button>
          </div>
          {#each selected.conditions.mediaQueries as condition (condition.id)}
            <div class="media-condition-row">
              <input
                type="checkbox"
                checked={condition.enabled}
                aria-label={t("motion-enable-media-condition")}
                onchange={(event) => {
                  void patchMediaCondition(selected, condition.id, {
                    enabled: event.currentTarget.checked,
                  });
                }}
              />
              <input value={condition.query} placeholder="(min-width: 768px)" onchange={(event) => {
                void patchMediaCondition(selected, condition.id, {
                  query: event.currentTarget.value,
                });
              }} />
              <button type="button" title={t("motion-delete-media-condition")} onclick={() => {
                void patchInteraction(selected, {
                  conditions: {
                    ...selected.conditions,
                    mediaQueries: selected.conditions.mediaQueries.filter(
                      (candidate) => candidate.id !== condition.id,
                    ),
                  },
                });
              }}><IconTrash size={13} /></button>
            </div>
          {/each}
          <small>{t("motion-media-alternatives")}</small>
        </div>
      </details>
    </article>
  {:else if !creating}
    <div class="empty-state">
      <IconBolt size={24} stroke={1.5} />
      <strong>{t("motion-empty-title")}</strong>
      <span>{t("motion-empty-description")}</span>
      <button type="button" onclick={() => { creating = true; }}>
        <IconPlus size={12} stroke={1.9} aria-hidden="true" />
        {t("motion-interactions")}
      </button>
    </div>
  {/if}

  <section class="secondary-section">
    <header>
      <div><IconLayoutGrid size={15} /><strong>{t("motion-behaviors")}</strong></div>
      <div>
        <button type="button" onclick={() => { void createBehavior("draggable"); }}>
          <IconPlus size={12} stroke={1.9} aria-hidden="true" />
          {t("motion-add-drag")}
        </button>
        <button type="button" onclick={() => { void createBehavior("layout"); }}>
          <IconPlus size={12} stroke={1.9} aria-hidden="true" />
          {t("motion-add-layout")}
        </button>
      </div>
    </header>
    {#each behaviors as behavior (behavior.id)}
      <div class="behavior-card">
        <div class="behavior-row">
          <button
            class="enabled-dot"
            class:off={!behavior.enabled}
            type="button"
            aria-label={behavior.enabled ? t("motion-disable-behavior") : t("motion-enable-behavior")}
            onclick={() => {
              void updateBehavior({ ...behavior, enabled: !behavior.enabled });
            }}
          ></button>
          <input value={behavior.name} aria-label={t("motion-behavior-name")} onchange={(event) => {
            void updateBehavior({ ...behavior, name: event.currentTarget.value });
          }} />
          <strong>{behavior.type === "draggable" ? t("motion-behavior-draggable") : t("motion-behavior-auto-layout")}</strong>
          <button type="button" title={t("motion-delete-behavior")} onclick={() => {
            void workspace.mutate({ command: "deleteBehavior", behaviorId: behavior.id });
          }}><IconTrash size={13} /></button>
        </div>
        {#if behavior.type === "draggable"}
          <div class="field-grid behavior-fields">
            <label><span>{t("motion-axes")}</span><select value={behavior.axis} onchange={(event) => {
              void updateBehavior({ ...behavior, axis: event.currentTarget.value as "x" | "y" | "both" });
            }}><option value="both">X + Y</option><option value="x">X</option><option value="y">Y</option></select></label>
            <label><span>{t("motion-css-container")}</span><input value={behavior.container} placeholder=".container" onchange={(event) => {
              void updateBehavior({ ...behavior, container: event.currentTarget.value });
            }} /></label>
            <label><span>{t("motion-snap-px")}</span><input type="number" min="0" value={behavior.snap} onchange={(event) => {
              void updateBehavior({ ...behavior, snap: Number(event.currentTarget.value) });
            }} /></label>
            <label><span>{t("motion-friction")}</span><input type="number" min="0" max="1" step="0.05" value={behavior.friction} onchange={(event) => {
              void updateBehavior({ ...behavior, friction: Number(event.currentTarget.value) });
            }} /></label>
            <label class="check"><input type="checkbox" checked={behavior.cursor} onchange={(event) => {
              void updateBehavior({ ...behavior, cursor: event.currentTarget.checked });
            }} /> {t("motion-grab-cursor")}</label>
          </div>
        {:else}
          <div class="field-grid behavior-fields">
            <label><span>{t("motion-tracked-children")}</span><input value={behavior.childrenSelector} placeholder="> *" onchange={(event) => {
              void updateBehavior({ ...behavior, childrenSelector: event.currentTarget.value });
            }} /></label>
            <label><span>{t("motion-duration-ms")}</span><input type="number" min="1" value={behavior.durationMs} onchange={(event) => {
              void updateBehavior({ ...behavior, durationMs: Number(event.currentTarget.value) });
            }} /></label>
            <label><span>{t("motion-easing")}</span><input value={behavior.ease} onchange={(event) => {
              void updateBehavior({ ...behavior, ease: event.currentTarget.value });
            }} /></label>
            <label><span>{t("motion-extra-properties")}</span><input value={behavior.properties.join(", ")} placeholder="opacity, borderRadius" onchange={(event) => {
              void updateBehavior({
                ...behavior,
                properties: event.currentTarget.value.split(",").map((value) => value.trim()).filter(Boolean),
              });
            }} /></label>
          </div>
          <p class="behavior-note">
            {t("motion-layout-note")}
            <code>registry.updateLayout("{behavior.id}", fn)</code>.
          </p>
        {/if}
      </div>
    {/each}
  </section>

  <details class="secondary-section custom-section" bind:open={customOpen}>
    <summary><span><IconCode size={15} />{t("motion-custom-code")}</span><small>{customCode.length}</small></summary>
    {#each customCode as custom (custom.id)}
      <div class="custom-card">
        <input value={custom.name} aria-label={t("motion-custom-code-name")} onchange={(event) => {
          void workspace.mutate({
            command: "upsertCustomCode",
            customCode: { ...custom, name: event.currentTarget.value },
          });
        }} />
        <textarea value={custom.code} placeholder={t("motion-custom-code-placeholder")} onchange={(event) => {
          void workspace.mutate({
            command: "upsertCustomCode",
            customCode: { ...custom, code: event.currentTarget.value },
          });
        }}></textarea>
        <button type="button" onclick={() => {
          void workspace.mutate({ command: "deleteCustomCode", customCodeId: custom.id });
        }}><IconTrash size={13} /> {t("motion-delete")}</button>
      </div>
    {/each}
    <button class="add-row" type="button" onclick={() => { void addCustomCode(); }}>
      <IconPlus size={12} stroke={1.9} aria-hidden="true" />
      {t("motion-add-code-block")}
    </button>
  </details>
</section>

<style>
  .motion-inspector { display:flex; flex-direction:column; gap:0; padding:0; color:var(--text); }
  button, input, select, textarea { font:inherit; color:inherit; }
  button { cursor:pointer; }
  .section-head, .actions-head, .secondary-section header, .interaction-title, .action-buttons, .create-actions {
    display:flex; align-items:center; justify-content:space-between; gap:8px;
  }
  .section-head { min-height:38px; padding:6px 10px; border-bottom:1px solid var(--border-subtle); }
  .section-head > div, .actions-head > div { display:flex; flex-direction:column; gap:2px; }
  .section-head strong, .actions-head strong { font-size:12px; }
  .section-head span, .actions-head span { font-size:11px; color:var(--text-muted); }
  .primary-icon { width:28px; height:28px; display:grid; place-items:center; border:1px solid color-mix(in srgb,var(--brand) 36%,var(--border-subtle)); border-radius:var(--radius-control); background:var(--material-control); box-shadow:var(--shadow-control); color:var(--brand-strong); }
  .motion-error { margin:8px 10px; padding:8px; border:1px solid var(--danger); border-radius:var(--radius-control); color:var(--danger); font-size:11px; overflow-wrap:anywhere; }
  .create-card, .interaction-card, .secondary-section { border:0; border-bottom:1px solid var(--border-subtle); border-radius:0; background:transparent; }
  .create-card { display:flex; flex-direction:column; gap:9px; padding:10px; }
  label { display:flex; flex-direction:column; gap:4px; min-width:0; font-size:11px; color:var(--text-muted); }
  input, select, textarea { min-width:0; min-height:29px; padding:5px 7px; border:1px solid var(--border-2); border-radius:var(--radius-control); background:var(--material-inset); box-shadow:var(--shadow-inset); font-size:11px; }
  textarea { min-height:92px; resize:vertical; font-family:"JetBrains Mono",monospace; line-height:1.45; }
  .preset-grid { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:6px; }
  .preset-grid button, .create-actions button, .actions-head button, .action-buttons button, .secondary-section button, .add-row {
    min-height:28px; border:1px solid var(--border-2); border-radius:var(--radius-control); background:var(--material-control); box-shadow:var(--shadow-control); font-size:11px;
  }
  .preset-grid button.active, button.primary { border-color:var(--brand); background:var(--brand-soft); color:var(--brand-strong); font-weight:800; }
  .element-context { display:flex; flex-direction:column; gap:6px; max-height:220px; padding:8px 10px; overflow:auto; border-bottom:1px solid var(--border-subtle); }
  .context-group { overflow:hidden; border:1px solid var(--border-2); border-radius:7px; background:var(--surface-2); }
  .context-group header { display:flex; align-items:center; justify-content:space-between; min-height:28px; padding:0 8px; border-bottom:1px solid var(--border-3); }
  .context-group header strong { font-size:11px; }
  .context-group header span { color:var(--text-muted); font:11px "JetBrains Mono",monospace; }
  .context-group > button { --ui-entity-background:var(--surface); --ui-entity-border-color:var(--border-3); display:grid; grid-template-columns:minmax(0,1fr) auto; align-items:center; gap:7px; width:100%; min-height:40px; padding:5px 8px; border:0; border-bottom:1px solid var(--border-3); border-radius:0; background:var(--surface); text-align:left; }
  .context-group > button:last-child { border-bottom:0; }
  .context-group > button > span { display:flex; flex-direction:column; min-width:0; gap:2px; }
  .context-group > button strong { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:11px; }
  .context-group > button small { overflow:hidden; color:var(--text-muted); text-overflow:ellipsis; white-space:nowrap; font-size:11px; }
  .active-timeline-context { display:flex; flex-direction:column; gap:3px; padding:8px; border:1px solid var(--brand); border-radius:7px; background:var(--brand-soft); }
  .active-timeline-context strong { color:var(--brand-strong); font-size:11px; }
  .active-timeline-context span { color:var(--text-muted); font-size:11px; line-height:1.4; }
  .interaction-card { overflow:hidden; }
  .interaction-title { padding:7px; border-bottom:1px solid var(--border-3); }
  .interaction-title input { flex:1; border-color:transparent; background:transparent; font-weight:800; }
  .interaction-title button { border:0; background:transparent; color:var(--text-muted); }
  .interaction-title button.enabled-dot { flex:0 0 auto; width:10px; height:10px; min-height:10px; padding:0; border:2px solid var(--surface); border-radius:50%; background:var(--brand); box-shadow:0 0 0 1px var(--brand); }
  .interaction-title button.enabled-dot.off { background:var(--text-muted); box-shadow:0 0 0 1px var(--text-muted); }
  .flow-card { padding:9px; border-bottom:1px solid var(--border-3); }
  .flow-step { display:flex; align-items:center; gap:8px; }
  .flow-step label, .flow-step > div { flex:1; }
  .flow-step small { display:block; color:var(--brand-strong); font-size:11px; font-weight:900; letter-spacing:.06em; }
  .flow-step strong { display:block; margin-top:3px; font:11px "JetBrains Mono",monospace; overflow-wrap:anywhere; }
  .step-index, .action-index { display:grid; place-items:center; width:20px; height:20px; border-radius:50%; background:var(--brand-soft); color:var(--brand-strong); font-size:11px; font-weight:900; }
  .flow-connector { height:13px; margin-left:10px; padding-left:8px; border-left:1px solid var(--border-2); color:var(--text-muted); }
  .sub-grid, .field-grid { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:7px; margin:8px 0 0 28px; }
  .check { flex-direction:row; align-items:center; align-self:end; min-height:29px; }
  .check input { min-height:auto; }
  .actions-head { padding:8px; border-bottom:1px solid var(--border-3); }
  .add-action-control { display:flex; align-items:center; gap:5px; }
  .add-action-control select { max-width:128px; }
  .actions-head button, .action-buttons button { display:flex; align-items:center; gap:4px; padding:0 7px; }
  .action-list { display:flex; flex-direction:column; }
  .action-card { border-bottom:1px solid var(--border-3); }
  .action-card summary { display:grid; grid-template-columns:auto auto minmax(0,1fr) auto; align-items:center; gap:6px; min-height:34px; padding:0 8px; cursor:pointer; list-style:none; }
  .action-card summary::-webkit-details-marker { display:none; }
  .action-card summary strong { font-size:11px; overflow:hidden; text-overflow:ellipsis; }
  .action-card summary small { color:var(--text-muted); font:11px "JetBrains Mono",monospace; }
  .action-editor { display:flex; flex-direction:column; gap:9px; padding:8px; border-top:1px solid var(--border-3); background:color-mix(in srgb,var(--surface) 55%,transparent); }
  .action-editor .field-grid { margin:0; }
  .property-list { display:flex; flex-direction:column; gap:5px; }
  .property-row { display:grid; grid-template-columns:1.25fr .7fr auto .7fr .45fr auto; align-items:center; gap:4px; }
  .property-row input, .property-row select { min-width:0; width:100%; }
  :global(.value-arrow) { color:var(--text-muted); }
  .property-row button { border:0; background:transparent; color:var(--text-muted); }
  .unit { text-align:center; }
  .add-row { width:100%; display:flex; align-items:center; justify-content:center; gap:4px; color:var(--brand-strong); }
  .trigger-target-editor { display:flex; flex-direction:column; gap:4px; }
  .trigger-target-editor select, .trigger-target-editor input { width:100%; }
  .set-values { display:flex; flex-direction:column; gap:5px; }
  .set-value-row { display:grid; grid-template-columns:1fr 1fr 1fr auto; gap:4px; align-items:center; }
  .set-value-row input, .set-value-row select { width:100%; }
  .set-value-row button { border:0; background:transparent; color:var(--text-muted); }
  .set-add-buttons { display:flex; gap:5px; }
  .set-add-buttons button { display:inline-flex; align-items:center; justify-content:center; gap:4px; min-height:27px; padding:0 6px; border:1px solid var(--border-2); border-radius:5px; background:var(--surface); font-size:11px; }
  .advanced-action summary, .interaction-advanced summary { cursor:pointer; color:var(--text-muted); font-size:11px; font-weight:800; }
  .advanced-action .field-grid { grid-template-columns:1fr; margin-top:7px; }
  .keyframes-editor { display:flex; flex-direction:column; gap:6px; margin-top:9px; padding-top:8px; border-top:1px solid var(--border-3); }
  .keyframe-row { display:grid; grid-template-columns:72px 1fr auto; gap:5px; padding:6px; border:1px solid var(--border-3); border-radius:6px; background:var(--surface); }
  .keyframe-properties { grid-column:1 / -1; display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:5px; }
  .keyframe-row > button { border:0; background:transparent; color:var(--text-muted); }
  .action-buttons { justify-content:flex-end; }
  .action-buttons .danger { color:var(--danger); }
  .timeline-button { display:grid; grid-template-columns:auto 1fr auto; align-items:center; gap:7px; width:calc(100% - 16px); margin:8px; min-height:32px; padding:0 9px; border:1px solid var(--brand); border-radius:7px; background:var(--brand-soft); color:var(--brand-strong); text-align:left; font-weight:800; }
  .timeline-button span { font-size:11px; color:var(--text-muted); }
  .interaction-advanced { padding:0 8px 9px; }
  .interaction-advanced .field-grid { margin:8px 0 0; }
  .media-conditions { display:flex; flex-direction:column; gap:5px; margin-top:9px; padding-top:8px; border-top:1px solid var(--border-3); }
  .media-conditions-head { display:flex; align-items:center; justify-content:space-between; gap:6px; }
  .media-conditions-head strong { font-size:11px; }
  .media-conditions-head button { display:inline-flex; align-items:center; justify-content:center; gap:4px; min-height:26px; border:1px solid var(--border-2); border-radius:5px; background:var(--surface); font-size:11px; }
  .media-condition-row { display:grid; grid-template-columns:auto minmax(0,1fr) auto; align-items:center; gap:5px; }
  .media-condition-row input[type="checkbox"] { min-height:auto; }
  .media-condition-row button { border:0; background:transparent; color:var(--text-muted); }
  .media-conditions > small { color:var(--text-muted); font-size:11px; line-height:1.4; }
  .empty-state { display:flex; flex-direction:column; align-items:center; gap:6px; padding:20px 12px; border:0; border-bottom:1px solid var(--border-subtle); border-radius:0; text-align:center; color:var(--text-muted); }
  .empty-state strong { color:var(--text); font-size:11px; }
  .empty-state span { font-size:11px; line-height:1.4; }
  .empty-state button { display:inline-flex; align-items:center; justify-content:center; gap:4px; min-height:29px; padding:0 10px; border:1px solid var(--brand); border-radius:6px; background:var(--brand-soft); color:var(--brand-strong); font-weight:800; }
  .secondary-section { padding:10px; }
  .secondary-section header > div { display:flex; align-items:center; gap:5px; }
  .secondary-section header strong { font-size:11px; }
  .secondary-section header button { display:inline-flex; align-items:center; justify-content:center; gap:4px; padding:0 6px; }
  .behavior-card { margin-top:7px; padding-top:7px; border-top:1px solid var(--border-3); }
  .behavior-row { display:grid; grid-template-columns:auto minmax(0,1fr) auto auto; align-items:center; gap:6px; }
  .behavior-row > input { border-color:transparent; background:transparent; font-weight:800; }
  .behavior-row strong { font-size:11px; overflow:hidden; text-overflow:ellipsis; }
  .behavior-row input { width:100%; }
  .behavior-fields { margin:7px 0 0 16px; }
  .behavior-note { margin:7px 0 0 16px; color:var(--text-muted); font-size:11px; line-height:1.45; }
  .behavior-note code { color:var(--brand-strong); font-family:"JetBrains Mono",monospace; overflow-wrap:anywhere; }
  .custom-section summary { display:flex; justify-content:space-between; cursor:pointer; list-style:none; }
  .custom-section summary span { display:flex; align-items:center; gap:5px; font-size:11px; font-weight:800; }
  .custom-card { display:flex; flex-direction:column; gap:6px; margin-top:8px; padding-top:8px; border-top:1px solid var(--border-3); }
  .custom-card button { align-self:flex-end; padding:0 7px; color:var(--danger); }
  @media (max-width:340px) {
    .sub-grid, .field-grid { grid-template-columns:1fr; }
    .property-row { grid-template-columns:1fr 1fr auto; }
    .set-value-row { grid-template-columns:1fr 1fr auto; }
    .property-row :global(.value-arrow) { display:none; }
  }
</style>
