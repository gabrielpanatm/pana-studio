<script lang="ts">
  import { MOTION_FAMILIES, emptyExpression } from "$lib/js/motion-config";
  import AnimationEditor from "$lib/components/inspector/js/AnimationEditor.svelte";
  import AnimatableEditor from "$lib/components/inspector/js/AnimatableEditor.svelte";
  import DraggableEditor from "$lib/components/inspector/js/DraggableEditor.svelte";
  import EasingEditor from "$lib/components/inspector/js/EasingEditor.svelte";
  import EngineEditor from "$lib/components/inspector/js/EngineEditor.svelte";
  import LayoutEditor from "$lib/components/inspector/js/LayoutEditor.svelte";
  import ScrollEditor from "$lib/components/inspector/js/ScrollEditor.svelte";
  import ScopeEditor from "$lib/components/inspector/js/ScopeEditor.svelte";
  import SvgEditor from "$lib/components/inspector/js/SvgEditor.svelte";
  import TextEditor from "$lib/components/inspector/js/TextEditor.svelte";
  import TimerEditor from "$lib/components/inspector/js/TimerEditor.svelte";
  import UtilityEditor from "$lib/components/inspector/js/UtilityEditor.svelte";
  import WaapiEditor from "$lib/components/inspector/js/WaapiEditor.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type {
    PanaMotionAnimationItem,
    PanaMotionAnimatableItem,
    PanaMotionDraggableItem,
    PanaMotionEasingItem,
    PanaMotionExpression,
    PanaMotionItem,
    PanaMotionLayoutItem,
    PanaMotionScopeItem,
    PanaMotionScrollItem,
    PanaMotionSvgItem,
    PanaMotionTextItem,
    PanaMotionTimerItem,
    PanaMotionUtilitiesItem,
    PanaMotionWaapiItem,
  } from "$lib/types";

  let {
    item,
    onChange = undefined as ((item: PanaMotionItem) => void) | undefined,
    onDelete = undefined as ((id: string) => void) | undefined,
  }: {
    item: PanaMotionItem;
    onChange?: (item: PanaMotionItem) => void;
    onDelete?: (id: string) => void;
  } = $props();

  const expressionPlaceholder = "(targets, utils, anime) => { ... }";
  const enabledOptions = [
    { value: "yes", label: "activ" },
    { value: "no", label: "oprit" },
  ];
  const targetModeOptions = [
    "selected",
    { value: "dataAnim", label: "data-anim" },
    "selector",
    { value: "dom", label: "DOM" },
    "array",
    "object",
    "scope",
    "expression",
  ];
  const family = $derived(MOTION_FAMILIES.find((entry) => entry.type === item.type));
  const itemUsesTarget = $derived([
    "animation",
    "animatable",
    "draggable",
    "layout",
    "scroll",
    "svg",
    "text",
    "waapi",
    "interaction",
  ].includes(item.type));

  function patch(patchValue: Partial<PanaMotionItem>) {
    onChange?.({ ...item, ...patchValue } as PanaMotionItem);
  }

  function patchTarget(field: string, value: string) {
    onChange?.({ ...item, target: { ...item.target, [field]: value } } as PanaMotionItem);
  }

  function patchAdvanced(index: number, patchValue: Partial<PanaMotionExpression>) {
    const advanced = item.advanced.map((entry, i) => i === index ? { ...entry, ...patchValue } : entry);
    patch({ advanced } as Partial<PanaMotionItem>);
  }

  function addAdvanced() {
    patch({ advanced: [...item.advanced, emptyExpression("Advanced Expression")] } as Partial<PanaMotionItem>);
  }

</script>

<div class="motion-editor">
  <div class="editor-heading">
    <div>
      <span>{family?.label ?? item.type}</span>
      <strong>{item.name || family?.label || item.type}</strong>
    </div>
    <button type="button" class="delete-btn" onclick={() => onDelete?.(item.id)}>Șterge</button>
  </div>

  <div class="field-grid">
    <label>
      <span>Nume</span>
      <input value={item.name} oninput={(event) => patch({ name: event.currentTarget.value })} />
    </label>
    <label>
      <span>Activ</span>
      <SelectControl value={item.enabled ? "yes" : "no"} options={enabledOptions} ariaLabel="Stare item motion" onchange={(value) => patch({ enabled: value === "yes" })} />
    </label>
  </div>

  {#if itemUsesTarget}
    <div class="field-grid">
      <label>
        <span>Target mode</span>
        <SelectControl value={item.target.mode} options={targetModeOptions} ariaLabel="Target motion" onchange={(value) => patchTarget("mode", value)} />
      </label>
      <label>
        <span>Selector</span>
        <input class="mono" value={item.target.selector} oninput={(event) => patchTarget("selector", event.currentTarget.value)} />
      </label>
    </div>

    {#if item.target.mode === "dataAnim"}
      <label>
        <span>Data anim</span>
        <input class="mono" value={item.target.dataAnim} oninput={(event) => patchTarget("dataAnim", event.currentTarget.value)} />
      </label>
    {/if}

    {#if item.target.mode === "expression"}
      <label>
        <span>Target expression</span>
        <textarea value={item.target.expression} oninput={(event) => patchTarget("expression", event.currentTarget.value)}></textarea>
      </label>
    {/if}
  {/if}

  {#if item.type === "animation"}
    {@const animation = item as PanaMotionAnimationItem}
    <AnimationEditor {animation} onChange={(next: PanaMotionAnimationItem) => onChange?.(next)} />
  {:else if item.type === "timer"}
    {@const timer = item as PanaMotionTimerItem}
    <TimerEditor {timer} onChange={(next: PanaMotionTimerItem) => onChange?.(next)} />
  {:else if item.type === "animatable"}
    {@const animatable = item as PanaMotionAnimatableItem}
    <AnimatableEditor {animatable} onChange={(next: PanaMotionAnimatableItem) => onChange?.(next)} />
  {:else if item.type === "draggable"}
    {@const draggable = item as PanaMotionDraggableItem}
    <DraggableEditor {draggable} onChange={(next: PanaMotionDraggableItem) => onChange?.(next)} />
  {:else if item.type === "layout"}
    {@const layout = item as PanaMotionLayoutItem}
    <LayoutEditor {layout} onChange={(next: PanaMotionLayoutItem) => onChange?.(next)} />
  {:else if item.type === "scope"}
    {@const scope = item as PanaMotionScopeItem}
    <ScopeEditor {scope} onChange={(next: PanaMotionScopeItem) => onChange?.(next)} />
  {:else if item.type === "scroll"}
    {@const scroll = item as PanaMotionScrollItem}
    <ScrollEditor {scroll} onChange={(next: PanaMotionScrollItem) => onChange?.(next)} />
  {:else if item.type === "svg"}
    {@const svg = item as PanaMotionSvgItem}
    <SvgEditor {svg} onChange={(next: PanaMotionSvgItem) => onChange?.(next)} />
  {:else if item.type === "text"}
    {@const text = item as PanaMotionTextItem}
    <TextEditor {text} onChange={(next: PanaMotionTextItem) => onChange?.(next)} />
  {:else if item.type === "waapi"}
    {@const waapi = item as PanaMotionWaapiItem}
    <WaapiEditor {waapi} onChange={(next: PanaMotionWaapiItem) => onChange?.(next)} />
  {:else if item.type === "utilities"}
    {@const utility = item as PanaMotionUtilitiesItem}
    <UtilityEditor {utility} onChange={(next: PanaMotionUtilitiesItem) => onChange?.(next)} />
  {:else if item.type === "easing"}
    {@const easing = item as PanaMotionEasingItem}
    <EasingEditor {easing} onChange={(next: PanaMotionEasingItem) => onChange?.(next)} />
  {:else if item.type === "engine"}
    <EngineEditor engine={item} onChange={(next) => onChange?.(next)} />
  {:else if item.type === "custom"}
    <label>
      <span>Custom JS</span>
      <textarea value={item.code} oninput={(event) => patch({ code: event.currentTarget.value } as Partial<PanaMotionItem>)}></textarea>
    </label>
  {/if}

  <div class="advanced-box">
    <div class="advanced-heading">
      <span>Advanced Expression</span>
      <button type="button" onclick={addAdvanced}>+ expresie</button>
    </div>
    {#each item.advanced as expression, index}
      <label class="advanced-row">
        <span>{expression.label}</span>
        <textarea value={expression.code} placeholder={expressionPlaceholder} oninput={(event) => patchAdvanced(index, { code: event.currentTarget.value, enabled: Boolean(event.currentTarget.value.trim()) })}></textarea>
      </label>
    {/each}
  </div>
</div>

<style>
  .motion-editor {
    display: flex;
    flex-direction: column;
    gap: 9px;
    min-width: 0;
  }

  .editor-heading,
  .advanced-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .editor-heading span,
  .advanced-heading span,
  label span {
    display: block;
    font-size: 9px;
    font-weight: 900;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .editor-heading strong {
    display: block;
    font-size: 13px;
    color: var(--text);
  }

  .delete-btn,
  .advanced-heading button {
    min-height: 24px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 10px;
    font-weight: 800;
    cursor: pointer;
  }

  .delete-btn {
    color: var(--danger);
  }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  input,
  textarea {
    width: 100%;
    min-width: 0;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 11px;
    padding: 0 6px;
  }

  input {
    height: 25px;
  }

  textarea {
    min-height: 62px;
    padding-block: 6px;
    resize: vertical;
  }

  .mono {
    font-family: "JetBrains Mono", monospace;
  }

  .advanced-box {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-top: 8px;
    border-top: 1px solid var(--border-3);
  }
</style>
