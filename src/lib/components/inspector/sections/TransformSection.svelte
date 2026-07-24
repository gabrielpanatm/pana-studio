<script lang="ts">
  import { tick } from "svelte";
  import type { ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import { IconPlus, IconTransform } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import PropInput from "../controls/PropInput.svelte";
  import TextWithOptions from "../controls/TextWithOptions.svelte";

  let {
    pendingValues,
    rulesMap,
    scssVariables = [],
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
    scssVariables?: ScssVariable[];
    edit: CssPropertyEditController;
  } = $props();

  function getValue(prop: string): string {
    return pendingValues[prop] ?? rulesMap[prop] ?? "";
  }

  const PROPS = ["transition","transform","transform-origin","transform-style","backface-visibility"];
  const hasValues = $derived(PROPS.some((p) => getValue(p) !== ""));

  // ── Presets & functions ──────────────────────────────────────────────────

  const TRANSITION_PRESETS = [
    { name: "Ease",        value: "all 0.3s ease" },
    { name: "Ease In-Out", value: "all 0.3s ease-in-out" },
    { name: "Linear",      value: "all 0.3s linear" },
    { name: "Quick",       value: "all 0.15s ease" },
    { name: "Smooth",      value: "all 0.5s cubic-bezier(0.4, 0, 0.2, 1)" },
    { name: "Bouncy",      value: "all 0.4s cubic-bezier(0.34, 1.56, 0.64, 1)" },
    { name: "Spring",      value: "all 0.6s cubic-bezier(0.175, 0.885, 0.32, 1.275)" },
    { name: "Snappy",      value: "all 0.2s cubic-bezier(0.4, 0, 0.2, 1)" },
    { name: "Niciuna",        value: "none" },
  ];

  const TRANSFORM_FUNCTIONS = [
    { name: "Translate X",  fn: "translateX(0px)"     },
    { name: "Translate Y",  fn: "translateY(0px)"     },
    { name: "Translate Z",  fn: "translateZ(0px)"     },
    { name: "Translate",    fn: "translate(0px, 0px)" },
    { name: "Rotate",       fn: "rotate(0deg)"        },
    { name: "Rotate X",     fn: "rotateX(0deg)"       },
    { name: "Rotate Y",     fn: "rotateY(0deg)"       },
    { name: "Rotate Z",     fn: "rotateZ(0deg)"       },
    { name: "Scale",        fn: "scale(1)"            },
    { name: "Scale X",      fn: "scaleX(1)"           },
    { name: "Scale Y",      fn: "scaleY(1)"           },
    { name: "Skew X",       fn: "skewX(0deg)"         },
    { name: "Skew Y",       fn: "skewY(0deg)"         },
    { name: "Perspective",  fn: "perspective(500px)"  },
  ];

  const ORIGIN_OPTS = [
    "center","top","bottom","left","right",
    "top left","top center","top right",
    "bottom left","bottom center","bottom right",
    "center left","center right",
    "0 0","50% 50%","100% 0","0 100%","100% 100%",
  ];

  const STYLE_OPTS       = ["flat","preserve-3d"];
  const BACKFACE_OPTS    = ["visible","hidden"];

  // ── Popover state ────────────────────────────────────────────────────────

  let trBtnRef = $state<HTMLButtonElement | null>(null);
  let tfBtnRef = $state<HTMLButtonElement | null>(null);
  let showTr   = $state(false);
  let showTf   = $state(false);
  let trPos    = $state({ top: 0, left: 0, width: 220 });
  let tfPos    = $state({ top: 0, left: 0, width: 220 });

  function calcPos(btn: HTMLButtonElement | null, width = 220) {
    if (!btn) return { top: 0, left: 0, width };
    const rect = btn.getBoundingClientRect();
    const left = Math.max(8, Math.min(rect.right - width, window.innerWidth - width - 8));
    const spaceBelow = window.innerHeight - rect.bottom - 8;
    const spaceAbove = rect.top - 8;
    const top = spaceBelow >= 180 || spaceBelow >= spaceAbove
      ? rect.bottom + 4
      : Math.max(8, rect.top - Math.min(240, spaceAbove) - 4);
    return { top, left, width };
  }

  function openTr() {
    showTf = false;
    if (showTr) { showTr = false; return; }
    trPos = calcPos(trBtnRef);
    showTr = true;
  }

  function openTf() {
    showTr = false;
    if (showTf) { showTf = false; return; }
    tfPos = calcPos(tfBtnRef);
    showTf = true;
  }

  function closeAll() { showTr = false; showTf = false; }

  function applyTransition(value: string) {
    edit.commit("transition", value);
    closeAll();
  }

  function addTransformFn(fn: string) {
    const current = getValue("transform").trim();
    const next = (!current || current === "none")
      ? fn
      : `${current} ${fn}`;
    edit.commit("transform", next);
    closeAll();
  }
</script>

<!-- Backdrop to close popovers on outside click -->
{#if showTr || showTf}
  <div
    class="popover-backdrop"
    role="presentation"
    onmousedown={closeAll}
  ></div>
{/if}

<!-- Transition popover -->
{#if showTr}
  <div
    class="tr-popover"
    role="listbox"
    style="top:{trPos.top}px; left:{trPos.left}px; width:{trPos.width}px;"
  >
    {#each TRANSITION_PRESETS as p}
      <button
        type="button"
        class="tr-option"
        onmousedown={(e) => e.preventDefault()}
        onclick={() => applyTransition(p.value)}
      >
        <span class="tr-opt-name">{p.name}</span>
        <span class="tr-opt-value">{p.value}</span>
      </button>
    {/each}
  </div>
{/if}

<!-- Transform function popover -->
{#if showTf}
  <div
    class="tr-popover"
    role="listbox"
    style="top:{tfPos.top}px; left:{tfPos.left}px; width:{tfPos.width}px;"
  >
    {#each TRANSFORM_FUNCTIONS as f}
      <button
        type="button"
        class="tr-option fn-option"
        onmousedown={(e) => e.preventDefault()}
        onclick={() => addTransformFn(f.fn)}
      >
        {f.name}
      </button>
    {/each}
  </div>
{/if}

<InspectorSection title="Transform" {hasValues}>
  {#snippet icon()}<IconTransform size={13} stroke={1.7} />{/snippet}

  <!-- TRANSITION -->
  <div class="sub-header">
    <span class="sub-label" class:has-value={getValue("transition") !== ""}>TRANSITION</span>
    <button
      bind:this={trBtnRef}
      type="button"
      class="add-btn"
      class:active={showTr}
      title="Preseturi tranziție"
      aria-label="Deschide presetările pentru tranziție"
      onclick={openTr}
    >
      <IconPlus size={13} stroke={1.9} />
    </button>
  </div>
  <PropInput
    value={getValue("transition")}
    placeholder="all 0.3s ease"
    {...edit.continuous("transition")}
  />

  <!-- TRANSFORM -->
  <div class="sub-header" style="margin-top: 4px;">
    <span class="sub-label" class:has-value={getValue("transform") !== ""}>TRANSFORM</span>
    <button
      bind:this={tfBtnRef}
      type="button"
      class="add-btn"
      class:active={showTf}
      title="Adaugă funcție transform"
      aria-label="Adaugă funcție transform"
      onclick={openTf}
    >
      <IconPlus size={13} stroke={1.9} />
    </button>
  </div>

  <div class="row-label">Transform</div>
  <PropInput
    value={getValue("transform")}
    placeholder="none"
    {...edit.continuous("transform")}
  />

  <div class="row-2 label-row">
    <span class="row-label">Transform Origin</span>
    <span class="row-label">Transform Style</span>
  </div>
  <div class="row-2">
    <TextWithOptions
      value={getValue("transform-origin")}
      placeholder="none"
      options={ORIGIN_OPTS}
      {...edit.continuous("transform-origin")}
    />
    <TextWithOptions
      value={getValue("transform-style")}
      placeholder="none"
      options={STYLE_OPTS}
      {...edit.continuous("transform-style")}
    />
  </div>

  <div class="row-label">Backface Visibility</div>
  <TextWithOptions
    value={getValue("backface-visibility")}
    placeholder="none"
    options={BACKFACE_OPTS}
    {...edit.continuous("backface-visibility")}
  />
</InspectorSection>

<style>
  .sub-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .sub-label {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .sub-label.has-value {
    color: var(--brand-strong);
  }

  .add-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: 1px solid var(--border-3);
    border-radius: 4px;
    background: var(--surface-4);
    color: var(--text-muted);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    transition: color 80ms, border-color 80ms, background 80ms;
  }

  .add-btn:hover, .add-btn.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .row-label {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }

  .row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .label-row { align-items: center; }

  /* ── Backdrop ─────────────────────────────────────────────────────────── */

  .popover-backdrop {
    position: fixed;
    inset: 0;
    z-index: 999;
  }

  /* ── Popover ──────────────────────────────────────────────────────────── */

  .tr-popover {
    position: fixed;
    z-index: 1000;
    overflow-y: auto;
    max-height: 280px;
    padding: 4px;
    border: 1px solid var(--border-4);
    border-radius: 8px;
    background: var(--surface-2);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
  }

  .tr-option {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    width: 100%;
    padding: 6px 9px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    cursor: pointer;
    text-align: left;
    transition: background 80ms;
  }

  .tr-option:hover {
    background: var(--brand-soft);
  }

  .tr-opt-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
  }

  .tr-opt-value {
    font-size: 12px;
    font-family: "JetBrains Mono", monospace;
    color: var(--text-muted);
  }

  .fn-option {
    font-size: 12px;
    color: var(--text);
    font-weight: 500;
  }

  .fn-option:hover {
    color: var(--brand-strong);
  }
</style>
