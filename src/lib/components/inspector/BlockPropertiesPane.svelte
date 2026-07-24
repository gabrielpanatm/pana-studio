<script lang="ts">
  import {
    IconArrowsMaximize,
    IconChevronDown,
    IconChevronUp,
    IconGripHorizontal,
  } from "@tabler/icons-svelte";
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import { readUiBlockGraph } from "$lib/project/io";
  import type {
    BlockOptionDefinition,
    BlockOptionValue,
    FileBufferRequestIdentity,
    NativeBlockOptionState,
    SelectionInfo,
    UiBlockGraphSnapshot,
    UiBlockSourceInstance,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  type ApplyRequest = {
    providerId: string;
    optionId: string;
    value: BlockOptionValue;
    rootSelector: string;
    rootTag: string;
    rootSourceId: string | null;
    rootLocation: UiBlockSourceInstance["rootLocation"];
    rootSessionId: string | null;
  };

  let {
    selectedElement = null,
    projectRoot = "",
    runtimeSessionId = "",
    workspaceRevision = 0,
    previewRevision = "",
    height = 220,
    collapsed = false,
    onLayoutCommit,
    onApply,
  }: {
    selectedElement?: SelectionInfo | null;
    projectRoot?: string;
    runtimeSessionId?: string;
    workspaceRevision?: number;
    previewRevision?: string;
    height?: number;
    collapsed?: boolean;
    onLayoutCommit?: (height: number, collapsed: boolean) => void;
    onApply: (request: ApplyRequest) => Promise<EditorActionOutcome>;
  } = $props();

  const MIN_HEIGHT = 140;
  const MAX_HEIGHT = 520;
  let panelHeight = $state(220);
  let panelCollapsed = $state(false);
  let graph = $state<UiBlockGraphSnapshot | null>(null);
  let loadError = $state("");
  let status = $state("");
  let pendingOption = $state("");
  let draftValues = $state<Record<string, BlockOptionValue>>({});
  let requestKey = "";

  const blockContext = $derived(selectedElement?.blockContext ?? null);
  const sourceInstance = $derived.by(() => {
    if (!graph || !blockContext) return null;
    if (blockContext.rootSourceId) {
      const exact = graph.sourceInstances.find(
        (instance) => instance.rootSourceNodeId === blockContext.rootSourceId,
      );
      if (exact) return exact;
    }
    return null;
  });
  const definition = $derived(
    sourceInstance?.definitionId
      ? graph?.definitions.find((candidate) => candidate.id === sourceInstance.definitionId) ?? null
      : graph?.definitions.find((candidate) => candidate.providerId === blockContext?.providerId) ?? null,
  );

  $effect(() => {
    panelHeight = clampHeight(height);
    panelCollapsed = collapsed;
  });

  $effect(() => {
    const context = blockContext;
    const key = `${projectRoot}\u0000${runtimeSessionId}\u0000${workspaceRevision}\u0000${previewRevision}\u0000${context?.rootSourceId ?? ""}\u0000${context?.providerId ?? ""}`;
    if (!context || !projectRoot || !runtimeSessionId) {
      graph = null;
      loadError = "";
      requestKey = key;
      return;
    }
    if (requestKey === key) return;
    requestKey = key;
    loadError = "";
    const identity: FileBufferRequestIdentity = {
      expectedProjectRoot: projectRoot,
      expectedSessionId: runtimeSessionId,
    };
    void readUiBlockGraph(identity)
      .then((snapshot) => {
        if (requestKey !== key) return;
        graph = snapshot;
      })
      .catch((cause) => {
        if (requestKey === key) loadError = errorMessage(cause);
      });
  });

  $effect(() => {
    const instance = sourceInstance;
    if (!instance) {
      draftValues = {};
      return;
    }
    draftValues = Object.fromEntries(
      instance.options.map((option) => [option.id, cloneValue(option.value)]),
    );
    status = "";
  });

  function clampHeight(value: number) {
    return Math.max(MIN_HEIGHT, Math.min(MAX_HEIGHT, Math.round(value)));
  }

  function cloneValue(value: BlockOptionValue): BlockOptionValue {
    return { ...value } as BlockOptionValue;
  }

  function stateFor(optionId: string): NativeBlockOptionState | null {
    return sourceInstance?.options.find((candidate) => candidate.id === optionId) ?? null;
  }

  function valueFor(option: BlockOptionDefinition): BlockOptionValue {
    return draftValues[option.id] ?? stateFor(option.id)?.value ?? option.defaultValue;
  }

  function textValue(option: BlockOptionDefinition): string {
    const value = valueFor(option);
    return value.kind === "text" ? value.value : "";
  }

  function numberValue(option: BlockOptionDefinition): number {
    const value = valueFor(option);
    return value.kind === "integer" ? value.value : 0;
  }

  function booleanValue(option: BlockOptionDefinition): boolean {
    const value = valueFor(option);
    return value.kind === "boolean" ? value.value : false;
  }

  function setDraft(optionId: string, value: BlockOptionValue) {
    draftValues = { ...draftValues, [optionId]: value };
  }

  function resetDraft(optionId: string) {
    const source = stateFor(optionId);
    if (!source) return;
    setDraft(optionId, cloneValue(source.value));
    status = "Modificarea locală a fost anulată.";
  }

  function valueEquals(left: BlockOptionValue, right: BlockOptionValue) {
    return left.kind === right.kind && left.value === right.value;
  }

  async function commit(option: BlockOptionDefinition) {
    const context = blockContext;
    const instance = sourceInstance;
    const source = stateFor(option.id);
    const value = valueFor(option);
    if (!context || !instance || !source || !instance.editable || pendingOption) return;
    if (valueEquals(value, source.value)) {
      status = "Valoarea nu are modificări de confirmat.";
      return;
    }
    pendingOption = option.id;
    status = "Se validează în nucleul Rust…";
    const outcome = await onApply({
      providerId: instance.providerId,
      optionId: option.id,
      value,
      rootSelector: context.rootSelector,
      rootTag: context.rootTag,
      rootSourceId: instance.rootSourceNodeId,
      rootLocation: instance.rootLocation,
      rootSessionId: context.rootSessionId,
    });
    pendingOption = "";
    if (outcome.status === "committed") {
      status = "Proprietatea este în sesiunea proiectului. Ctrl+S persistă pe disc.";
    } else if (outcome.status === "noop") {
      status = outcome.reason ?? "Valoarea era deja confirmată.";
    } else {
      status = outcome.reason ?? "Proprietatea nu a putut fi aplicată.";
      resetDraft(option.id);
    }
  }

  function handleTextKeydown(event: KeyboardEvent, option: BlockOptionDefinition) {
    if (event.key === "Enter") {
      event.preventDefault();
      void commit(option);
    } else if (event.key === "Escape") {
      event.preventDefault();
      resetDraft(option.id);
      (event.currentTarget as HTMLInputElement).blur();
    }
  }

  function setCollapsed(next: boolean) {
    panelCollapsed = next;
    onLayoutCommit?.(panelHeight, next);
  }

  function maximize() {
    panelHeight = MAX_HEIGHT;
    panelCollapsed = false;
    onLayoutCommit?.(panelHeight, panelCollapsed);
  }

  function startResize(event: PointerEvent) {
    if (panelCollapsed) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    const pointerId = event.pointerId;
    const startY = event.clientY;
    const startHeight = panelHeight;
    handle.setPointerCapture(pointerId);
    const move = (moveEvent: PointerEvent) => {
      panelHeight = clampHeight(startHeight + startY - moveEvent.clientY);
    };
    const finish = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", finish);
      handle.removeEventListener("pointercancel", finish);
      if (handle.hasPointerCapture(pointerId)) handle.releasePointerCapture(pointerId);
      onLayoutCommit?.(panelHeight, panelCollapsed);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", finish);
    handle.addEventListener("pointercancel", finish);
  }

  function resizeFromKeyboard(event: KeyboardEvent) {
    let next = panelHeight;
    if (event.key === "ArrowUp") next += 16;
    else if (event.key === "ArrowDown") next -= 16;
    else if (event.key === "Home") next = MIN_HEIGHT;
    else if (event.key === "End") next = MAX_HEIGHT;
    else return;
    event.preventDefault();
    panelHeight = clampHeight(next);
    onLayoutCommit?.(panelHeight, panelCollapsed);
  }
</script>

{#if blockContext}
  <section
    class="block-properties"
    class:collapsed={panelCollapsed}
    style={`--block-properties-height: ${panelHeight}px`}
    aria-label="Proprietăți bloc"
  >
    {#if !panelCollapsed}
      <button
        class="resize-handle"
        type="button"
        aria-label="Redimensionează proprietățile blocului"
        title={`Înălțime ${panelHeight}px. Săgeți sus/jos pentru reglare.`}
        onpointerdown={startResize}
        onkeydown={resizeFromKeyboard}
      ><IconGripHorizontal size={16} stroke={1.8} /></button>
    {/if}
    <header>
      <div>
        <span>Proprietăți bloc</span>
        <strong>{definition?.displayName ?? blockContext.providerId}</strong>
      </div>
      <div class="panel-actions">
        <button type="button" aria-label="Extinde panoul" title="Extinde" onclick={maximize}>
          <IconArrowsMaximize size={14} stroke={1.8} />
        </button>
        <button
          type="button"
          aria-label={panelCollapsed ? "Deschide proprietățile blocului" : "Pliază proprietățile blocului"}
          aria-expanded={!panelCollapsed}
          onclick={() => setCollapsed(!panelCollapsed)}
        >
          {#if panelCollapsed}<IconChevronUp size={15} />{:else}<IconChevronDown size={15} />{/if}
        </button>
      </div>
    </header>

    {#if !panelCollapsed}
      <div class="properties-body">
        <div class="block-breadcrumb">
          <code>{blockContext.providerId}</code>
          <span>›</span>
          <span>&lt;{selectedElement?.tag}&gt;</span>
        </div>
        {#if loadError}
          <p class="diagnostic" role="alert">{loadError}</p>
        {:else if !graph}
          <p class="empty">Se citește contractul din Rust…</p>
        {:else if !sourceInstance}
          <p class="diagnostic">
            Blocul este vizibil în Canvas, dar rădăcina sa nu a putut fi corelată cu sursa autoritativă.
          </p>
        {:else if sourceInstance.diagnostic}
          <p class:diagnostic={!sourceInstance.editable} class="source-note">
            {sourceInstance.diagnostic}
          </p>
        {/if}

        {#if definition && sourceInstance}
          {#if definition.options.length === 0}
            <p class="empty">Acest provider nu expune proprietăți configurabile.</p>
          {:else}
            <div class="option-list">
              {#each definition.options as option (option.id)}
                <label class="option-row">
                  <span>
                    <strong>{option.label}</strong>
                    <small>{option.description}</small>
                  </span>
                  {#if option.control === "toggle"}
                    <input
                      type="checkbox"
                      checked={booleanValue(option)}
                      disabled={!sourceInstance.editable || Boolean(pendingOption)}
                      onchange={(event) => {
                        setDraft(option.id, { kind: "boolean", value: event.currentTarget.checked });
                        void commit(option);
                      }}
                    />
                  {:else if option.control === "number"}
                    <input
                      class="value-input"
                      type="number"
                      value={numberValue(option)}
                      min={option.constraints.minimum ?? undefined}
                      max={option.constraints.maximum ?? undefined}
                      step={option.constraints.step ?? 1}
                      disabled={!sourceInstance.editable || Boolean(pendingOption)}
                      oninput={(event) => {
                        const next = event.currentTarget.valueAsNumber;
                        if (Number.isFinite(next)) {
                          setDraft(option.id, { kind: "integer", value: next });
                        }
                      }}
                      onblur={() => { void commit(option); }}
                      onkeydown={(event) => handleTextKeydown(event, option)}
                    />
                  {:else if option.control === "select"}
                    <select
                      class="value-input"
                      value={textValue(option)}
                      disabled={!sourceInstance.editable || Boolean(pendingOption)}
                      onchange={(event) => {
                        setDraft(option.id, { kind: "text", value: event.currentTarget.value });
                        void commit(option);
                      }}
                    >
                      {#each option.choices as choice (choice.value)}
                        <option value={choice.value}>{choice.label}</option>
                      {/each}
                    </select>
                  {:else}
                    <input
                      class="value-input"
                      type="text"
                      value={textValue(option)}
                      maxlength={option.constraints.maximumLength ?? undefined}
                      disabled={!sourceInstance.editable || Boolean(pendingOption)}
                      oninput={(event) => setDraft(option.id, { kind: "text", value: event.currentTarget.value })}
                      onblur={() => { void commit(option); }}
                      onkeydown={(event) => handleTextKeydown(event, option)}
                    />
                  {/if}
                </label>
              {/each}
            </div>
          {/if}
        {/if}
        {#if status}<p class="status" aria-live="polite">{status}</p>{/if}
      </div>
    {/if}
  </section>
{/if}

<style>
  .block-properties {
    position: relative;
    display: grid;
    flex: 0 0 var(--block-properties-height);
    grid-template-rows: auto minmax(0, 1fr);
    min-height: 0;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }
  .block-properties.collapsed { flex-basis: 38px; }
  .resize-handle {
    position: absolute;
    z-index: 2;
    top: -7px;
    left: 50%;
    display: grid;
    width: 44px;
    height: 14px;
    padding: 0;
    place-items: center;
    transform: translateX(-50%);
    border: 0;
    color: var(--text-muted);
    background: transparent;
    cursor: ns-resize;
    touch-action: none;
  }
  .resize-handle:focus-visible { outline: 2px solid var(--brand); outline-offset: -2px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 38px; padding: 5px 8px; border-bottom: 1px solid var(--border-subtle); }
  header > div:first-child { display: flex; align-items: baseline; gap: 7px; min-width: 0; }
  header span { color: var(--text-muted); font-size: 11px; font-weight: 750; text-transform: uppercase; }
  header strong { overflow: hidden; color: var(--text-strong); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .panel-actions { display: flex; flex: 0 0 auto; gap: 2px; }
  .panel-actions button { display: grid; width: 26px; height: 26px; padding: 0; place-items: center; border: 1px solid transparent; border-radius: var(--radius-control); color: var(--text-muted); background: transparent; }
  .panel-actions button:hover { border-color: var(--border); background: var(--control-hover); }
  .properties-body { min-height: 0; overflow: auto; padding: 8px; overscroll-behavior: contain; }
  .block-breadcrumb { display: flex; align-items: center; gap: 5px; margin-bottom: 8px; color: var(--text-muted); font-size: 11px; }
  .block-breadcrumb code { color: var(--brand-strong); }
  .option-list { display: grid; gap: 1px; }
  .option-row { display: grid; grid-template-columns: minmax(0, 1fr) minmax(72px, 104px); align-items: center; gap: 8px; min-height: 48px; padding: 6px 2px; border-top: 1px solid var(--border-subtle); }
  .option-row > span { display: grid; gap: 2px; min-width: 0; }
  .option-row strong { color: var(--text-strong); font-size: 11px; }
  .option-row small { color: var(--text-muted); font-size: 11px; line-height: 1.3; }
  .value-input { width: 100%; min-width: 0; height: 28px; padding: 0 7px; border: 1px solid var(--border); border-radius: var(--radius-control); color: var(--text); background: var(--surface-2); font-size: 11px; }
  input[type="checkbox"] { justify-self: end; width: 16px; height: 16px; accent-color: var(--brand); }
  .diagnostic, .source-note, .empty, .status { margin: 6px 0; font-size: 11px; line-height: 1.4; }
  .diagnostic { color: var(--danger); }
  .source-note, .empty, .status { color: var(--text-muted); }
  .status { padding-top: 6px; border-top: 1px solid var(--border-subtle); }
</style>
