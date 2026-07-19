<script lang="ts">
  import { IconTrash } from "@tabler/icons-svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import {
    defaultLoopDefinition,
    loopDefaultsForSource,
    loopDescription,
    loopLayoutOptions,
    loopSandboxItems,
    loopSnippetForDefinition,
    loopSourceOptions,
    normalizeLoopDefinition,
    type LoopDefinition,
    type LoopLayoutKind,
    type LoopSourceKind,
  } from "$lib/loops/model";

  export let definitions: LoopDefinition[] = [];
  export let onRegisterLoop: (definition: LoopDefinition) => void = () => {};
  export let onRemoveLoop: (id: string) => void = () => {};

  const initial = defaultLoopDefinition();
  let label = initial.label;
  let sourceKind: LoopSourceKind = initial.sourceKind;
  let alias = initial.alias;
  let layout: LoopLayoutKind = initial.layout;
  let sectionPath = initial.sectionPath ?? "_index.md";
  let extraKey = initial.extraKey ?? "services";
  let dataPath = initial.dataPath ?? "data/services.toml";
  let collectionKey = initial.collectionKey ?? "services";
  let customCollection = initial.customCollection ?? "items";
  let titleExpression = initial.titleExpression;
  let descriptionExpression = initial.descriptionExpression ?? "";
  let urlExpression = initial.urlExpression ?? "";

  $: draft = normalizeLoopDefinition({
    id: `loop-${sourceKind}-${label.toLowerCase().replace(/\s+/g, "-")}`,
    label,
    sourceKind,
    alias,
    layout,
    sectionPath,
    extraKey,
    dataPath,
    collectionKey,
    customCollection,
    titleExpression,
    descriptionExpression,
    urlExpression,
    createdAt: Date.now(),
  });

  function applySourceDefaults() {
    const defaults = loopDefaultsForSource(sourceKind);
    label = defaults.label ?? label;
    alias = defaults.alias ?? alias;
    sectionPath = defaults.sectionPath ?? sectionPath;
    extraKey = defaults.extraKey ?? extraKey;
    dataPath = defaults.dataPath ?? dataPath;
    collectionKey = defaults.collectionKey ?? collectionKey;
    customCollection = defaults.customCollection ?? customCollection;
    titleExpression = defaults.titleExpression ?? titleExpression;
    descriptionExpression = defaults.descriptionExpression ?? "";
    urlExpression = defaults.urlExpression ?? "";
  }

  function registerLoop() {
    onRegisterLoop({
      ...draft,
      id: `loop-${Date.now()}`,
      createdAt: Date.now(),
    });
  }
</script>

<article class="loop-panel">
  <div class="panel-heading">
    <div>
      <p class="eyebrow">Loop Builder</p>
      <h3>Blocuri repetabile</h3>
      <small>Configurează sursa înainte să apară în panoul Adaugă.</small>
    </div>
  </div>

  <div class="loop-form">
    <label>
      <span>Nume loop</span>
      <input bind:value={label} placeholder="Servicii" />
    </label>

    <label>
      <span>Sursă date</span>
      <SelectControl
        value={sourceKind}
        options={loopSourceOptions.map((option) => ({ value: option.kind, label: option.label }))}
        ariaLabel="Sursă date loop"
        onchange={(value) => {
          sourceKind = value as LoopSourceKind;
          applySourceDefaults();
        }}
      />
      <small>{loopSourceOptions.find((option) => option.kind === sourceKind)?.description}</small>
    </label>

    {#if sourceKind === "sectionPages" || sourceKind === "sectionExtra"}
      <label>
        <span>Secțiune</span>
        <input bind:value={sectionPath} placeholder="_index.md sau blog/_index.md" />
      </label>
    {/if}

    {#if sourceKind === "sectionExtra" || sourceKind === "configExtra"}
      <label>
        <span>Cheie extra</span>
        <input bind:value={extraKey} placeholder="services" />
      </label>
    {/if}

    {#if sourceKind === "dataFile"}
      <label>
        <span>Fișier data</span>
        <input bind:value={dataPath} placeholder="data/services.toml" />
      </label>
      <label>
        <span>Colecție</span>
        <input bind:value={collectionKey} placeholder="services" />
      </label>
    {/if}

    {#if sourceKind === "custom"}
      <label>
        <span>Expresie colecție</span>
        <input bind:value={customCollection} placeholder="items" />
      </label>
    {/if}

    <div class="loop-grid">
      <label>
        <span>Alias item</span>
        <input bind:value={alias} placeholder="item" />
      </label>
      <label>
        <span>Layout</span>
        <SelectControl
          value={layout}
          options={loopLayoutOptions.map((option) => ({ value: option.kind, label: option.label }))}
          ariaLabel="Layout loop"
          onchange={(value) => (layout = value as LoopLayoutKind)}
        />
      </label>
    </div>

    <div class="loop-grid">
      <label>
        <span>Titlu</span>
        <input bind:value={titleExpression} placeholder={`${alias}.title`} />
      </label>
      <label>
        <span>Link</span>
        <input bind:value={urlExpression} placeholder={`${alias}.url`} />
      </label>
    </div>

    <label>
      <span>Descriere</span>
      <input bind:value={descriptionExpression} placeholder={`${alias}.description`} />
    </label>

    <div class="loop-sandbox" aria-label="Sandbox loop">
      {#each loopSandboxItems(draft) as item}
        <div class="sandbox-card">
          <strong>{item.title}</strong>
          <span>{item.description}</span>
        </div>
      {/each}
    </div>

    <details>
      <summary>Cod generat</summary>
      <pre>{loopSnippetForDefinition(draft)}</pre>
    </details>

    <button class="wide-action" type="button" onclick={registerLoop}>Adaugă în panoul Adaugă</button>
  </div>

  <div class="registered-loops">
    <h4>Loop-uri pregătite</h4>
    {#each definitions as definition}
      <div class="registered-loop">
        <div>
          <strong>{definition.label}</strong>
          <span>{loopDescription(definition)}</span>
        </div>
        <button type="button" title="Șterge loop pregătit" onclick={() => onRemoveLoop(definition.id)}>
          <IconTrash size={14} stroke={1.9} />
        </button>
      </div>
    {:else}
      <p class="empty-line">Niciun loop pregătit încă.</p>
    {/each}
  </div>
</article>

<style>
  .loop-panel {
    display: grid;
    gap: 12px;
    min-width: 0;
    padding: 14px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .panel-heading {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 8px;
  }

  .panel-heading h3,
  .panel-heading p {
    margin: 0;
  }

  .panel-heading small,
  label small,
  .registered-loop span,
  .empty-line {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 700;
    line-height: 1.35;
  }

  .eyebrow {
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 900;
    text-transform: uppercase;
  }

  .loop-form,
  .registered-loops {
    display: grid;
    gap: 10px;
  }

  .loop-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 8px;
  }

  label {
    display: grid;
    gap: 5px;
    min-width: 0;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 900;
    text-transform: uppercase;
  }

  input {
    min-width: 0;
    width: 100%;
    height: 34px;
    padding: 6px 9px;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    color: var(--text-strong);
    background: var(--surface);
    font: inherit;
    font-size: 12px;
    font-weight: 800;
    text-transform: none;
  }

  .loop-sandbox {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
    padding: 8px;
    border: 1px dashed var(--border-3);
    border-radius: 8px;
    background: color-mix(in srgb, var(--surface) 72%, var(--surface-3));
  }

  .sandbox-card {
    display: grid;
    gap: 4px;
    min-width: 0;
    padding: 9px;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    background: var(--surface-2);
  }

  .sandbox-card strong,
  .registered-loop strong {
    min-width: 0;
    overflow: hidden;
    color: var(--text-strong);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 12px;
    font-weight: 900;
  }

  .sandbox-card span {
    display: -webkit-box;
    overflow: hidden;
    color: var(--text-muted);
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    font-size: 11px;
    font-weight: 700;
    line-height: 1.35;
  }

  details {
    border: 1px solid var(--border-2);
    border-radius: 7px;
    background: var(--surface);
  }

  summary {
    padding: 8px 10px;
    cursor: pointer;
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 900;
  }

  pre {
    max-height: 220px;
    margin: 0;
    overflow: auto;
    padding: 10px;
    border-top: 1px solid var(--border-2);
    color: var(--text);
    font-size: 11px;
    line-height: 1.45;
    white-space: pre-wrap;
  }

  .wide-action {
    min-height: 34px;
    padding: 8px 10px;
    border: 1px solid var(--brand);
    border-radius: 7px;
    color: var(--brand-strong);
    background: var(--brand-soft);
    font-size: 12px;
    font-weight: 900;
  }

  .registered-loops h4 {
    margin: 0;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 900;
    text-transform: uppercase;
  }

  .registered-loop {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 8px;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    background: var(--surface);
  }

  .registered-loop div {
    display: grid;
    gap: 3px;
    min-width: 0;
  }

  .registered-loop button {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    color: var(--text-muted);
    background: var(--surface-2);
  }

  @media (max-width: 1120px) {
    .loop-grid,
    .loop-sandbox {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
