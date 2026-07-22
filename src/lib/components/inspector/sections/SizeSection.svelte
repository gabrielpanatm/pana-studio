<script lang="ts">
  import type { ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import { variablesForProperty } from "$lib/editor/controls";
  import { IconArrowsMaximize } from "@tabler/icons-svelte";
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

  const BASIC_PROPS = ["width","height","min-width","min-height","max-width","max-height"];
  const ADVANCED_PROPS = [
    "aspect-ratio","object-fit","object-position",
    "scroll-snap-type","scroll-snap-align",
    "touch-action",
    "scrollbar-width","scrollbar-color",
  ];
  const hasValues = $derived([...BASIC_PROPS, ...ADVANCED_PROPS].some((p) => getValue(p) !== ""));
  const hasAdvancedValues = $derived(ADVANCED_PROPS.some((p) => getValue(p) !== ""));

  let showAdvanced = $state(false);
  $effect(() => { if (hasAdvancedValues) showAdvanced = true; });
</script>

<InspectorSection title="Size" {hasValues}>
  {#snippet icon()}<IconArrowsMaximize size={13} stroke={1.7} />{/snippet}

  <!-- W / H -->
  <div class="row-2">
    <PropInput label="W" value={getValue("width")} suggestions={variablesForProperty("width", scssVariables)} placeholder="auto" {...edit.continuous("width")} />
    <PropInput label="H" value={getValue("height")} suggestions={variablesForProperty("height", scssVariables)} placeholder="auto" {...edit.continuous("height")} />
  </div>

  <!-- Min W / Min H -->
  <div class="row-labels">
    <span class="row-label">Min W</span>
    <span class="row-label">Min H</span>
  </div>
  <div class="row-2">
    <PropInput label="mW" value={getValue("min-width")} suggestions={variablesForProperty("min-width", scssVariables)} placeholder="0" {...edit.continuous("min-width")} />
    <PropInput label="mH" value={getValue("min-height")} suggestions={variablesForProperty("min-height", scssVariables)} placeholder="0" {...edit.continuous("min-height")} />
  </div>

  <!-- Max W / Max H -->
  <div class="row-labels">
    <span class="row-label">Max W</span>
    <span class="row-label">Max H</span>
  </div>
  <div class="row-2">
    <PropInput label="MW" value={getValue("max-width")} suggestions={variablesForProperty("max-width", scssVariables)} placeholder="none" {...edit.continuous("max-width")} />
    <PropInput label="MH" value={getValue("max-height")} suggestions={variablesForProperty("max-height", scssVariables)} placeholder="none" {...edit.continuous("max-height")} />
  </div>

  <!-- Advanced toggle -->
  <button type="button" class="advanced-toggle" onclick={() => (showAdvanced = !showAdvanced)}>
    <span class="adv-chevron" class:open={showAdvanced}>›</span>
    Advanced options
  </button>

  {#if showAdvanced}
    <!-- Aspect Ratio / Object Fit -->
    <div class="row-labels">
      <span class="row-label">Aspect Ratio</span>
      <span class="row-label">Object Fit</span>
    </div>
    <div class="row-2">
      <TextWithOptions
        value={getValue("aspect-ratio")}
        placeholder="auto"
        options={["auto","1/1","4/3","16/9","3/2","2/1","9/16"]}
        {...edit.continuous("aspect-ratio")}
      />
      <TextWithOptions
        value={getValue("object-fit")}
        placeholder="auto"
        options={["fill","contain","cover","none","scale-down"]}
        {...edit.continuous("object-fit")}
      />
    </div>

    <!-- Object Position -->
    <div class="row-label">Poziția obiectului</div>
    <TextWithOptions
      value={getValue("object-position")}
      placeholder="auto"
      options={["center","top","bottom","left","right","top left","top right","bottom left","bottom right","50% 50%","0 0"]}
      {...edit.continuous("object-position")}
    />

    <!-- Scroll Snap / Snap Align -->
    <div class="row-labels">
      <span class="row-label">Scroll Snap</span>
      <span class="row-label">Snap Align</span>
    </div>
    <div class="row-2">
      <TextWithOptions
        value={getValue("scroll-snap-type")}
        placeholder="auto"
        options={["none","x mandatory","y mandatory","both mandatory","x proximity","y proximity","both proximity"]}
        {...edit.continuous("scroll-snap-type")}
      />
      <TextWithOptions
        value={getValue("scroll-snap-align")}
        placeholder="auto"
        options={["none","start","end","center","start end","center end"]}
        {...edit.continuous("scroll-snap-align")}
      />
    </div>

    <!-- Touch Action -->
    <div class="row-label">Acțiune la atingere</div>
    <TextWithOptions
      value={getValue("touch-action")}
      placeholder="auto"
      options={["auto","none","pan-x","pan-y","pan-left","pan-right","pan-up","pan-down","pinch-zoom","manipulation"]}
      {...edit.continuous("touch-action")}
    />

    <!-- Scrollbar Width / Color -->
    <div class="row-labels">
      <span class="row-label">Scrollbar Width</span>
      <span class="row-label">Scrollbar Color</span>
    </div>
    <div class="row-2">
      <TextWithOptions
        value={getValue("scrollbar-width")}
        placeholder="auto"
        options={["auto","thin","none"]}
        {...edit.continuous("scrollbar-width")}
      />
      <PropInput
        value={getValue("scrollbar-color")}
        placeholder="auto"
        {...edit.continuous("scrollbar-color")}
      />
    </div>
  {/if}
</InspectorSection>

<style>
  .row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .row-labels {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .row-label {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }

  /* ── Advanced toggle ─────────────────────────────────────────────────── */

  .advanced-toggle {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 2px 0;
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: color 80ms;
    margin-top: 2px;
  }

  .advanced-toggle:hover {
    color: var(--text);
  }

  .adv-chevron {
    display: inline-block;
    font-size: 13px;
    line-height: 1;
    transform: rotate(0deg);
    transition: transform 150ms ease;
    color: var(--text-muted);
  }

  .adv-chevron.open {
    transform: rotate(90deg);
  }
</style>
