<script lang="ts">
  import { IconPlus, IconTrash } from "@tabler/icons-svelte";
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

  let {
    definitions = [],
    register = () => {},
    remove = () => {},
  }: {
    definitions?: LoopDefinition[];
    register?: (definition: LoopDefinition) => void;
    remove?: (id: string) => void;
  } = $props();

  const initial = defaultLoopDefinition();
  let label = $state(initial.label);
  let sourceKind = $state<LoopSourceKind>(initial.sourceKind);
  let alias = $state(initial.alias);
  let layout = $state<LoopLayoutKind>(initial.layout);
  let sectionPath = $state(initial.sectionPath ?? "_index.md");
  let extraKey = $state(initial.extraKey ?? "services");
  let dataPath = $state(initial.dataPath ?? "data/services.toml");
  let collectionKey = $state(initial.collectionKey ?? "services");
  let customCollection = $state(initial.customCollection ?? "items");
  let titleExpression = $state(initial.titleExpression);
  let descriptionExpression = $state(initial.descriptionExpression ?? "");
  let urlExpression = $state(initial.urlExpression ?? "");

  const draft = $derived(normalizeLoopDefinition({
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
  }));

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

  function registerDraft() {
    register({ ...draft, id: `loop-${Date.now()}`, createdAt: Date.now() });
  }
</script>

<section class="loop-builder" aria-labelledby="loop-builder-title">
  <header>
    <div>
      <span>Componente Tera</span>
      <h2 id="loop-builder-title">Liste dinamice</h2>
      <p>Configurează o sursă repetabilă, apoi folosește rezultatul din panoul Adaugă element.</p>
    </div>
    <strong>{definitions.length} pregătite</strong>
  </header>

  <div class="builder-grid">
    <form class="loop-form" onsubmit={(event) => { event.preventDefault(); registerDraft(); }}>
      <label><span>Nume</span><input bind:value={label} placeholder="Servicii" /></label>
      <label>
        <span>Sursă de date</span>
        <SelectControl
          value={sourceKind}
          options={loopSourceOptions.map((option) => ({ value: option.kind, label: option.label }))}
          ariaLabel="Sursă de date pentru lista dinamică"
          onchange={(value) => { sourceKind = value as LoopSourceKind; applySourceDefaults(); }}
        />
        <small>{loopSourceOptions.find((option) => option.kind === sourceKind)?.description}</small>
      </label>

      {#if sourceKind === "sectionPages" || sourceKind === "sectionExtra"}
        <label><span>Secțiune</span><input bind:value={sectionPath} placeholder="_index.md sau blog/_index.md" /></label>
      {/if}
      {#if sourceKind === "sectionExtra" || sourceKind === "configExtra"}
        <label><span>Cheie extra</span><input bind:value={extraKey} placeholder="services" /></label>
      {/if}
      {#if sourceKind === "dataFile"}
        <div class="field-pair">
          <label><span>Fișier de date</span><input bind:value={dataPath} placeholder="data/services.toml" /></label>
          <label><span>Colecție</span><input bind:value={collectionKey} placeholder="services" /></label>
        </div>
      {/if}
      {#if sourceKind === "custom"}
        <label><span>Expresie colecție</span><input bind:value={customCollection} placeholder="items" /></label>
      {/if}

      <div class="field-pair">
        <label><span>Alias element</span><input bind:value={alias} placeholder="item" /></label>
        <label>
          <span>Aspect</span>
          <SelectControl
            value={layout}
            options={loopLayoutOptions.map((option) => ({ value: option.kind, label: option.label }))}
            ariaLabel="Aspect listă dinamică"
            onchange={(value) => (layout = value as LoopLayoutKind)}
          />
        </label>
      </div>
      <div class="field-pair">
        <label><span>Titlu</span><input bind:value={titleExpression} placeholder={`${alias}.title`} /></label>
        <label><span>Link</span><input bind:value={urlExpression} placeholder={`${alias}.url`} /></label>
      </div>
      <label><span>Descriere</span><input bind:value={descriptionExpression} placeholder={`${alias}.description`} /></label>
      <button class="primary-action" type="submit"><IconPlus size={16} /> Adaugă în panoul Adaugă element</button>
    </form>

    <div class="preview-column">
      <section class="sandbox" aria-labelledby="loop-preview-title">
        <h3 id="loop-preview-title">Previzualizare date</h3>
        <div class="sandbox-grid">
          {#each loopSandboxItems(draft) as item}
            <article><strong>{item.title}</strong><span>{item.description}</span></article>
          {/each}
        </div>
        <details><summary>Cod Tera generat</summary><pre>{loopSnippetForDefinition(draft)}</pre></details>
      </section>

      <section class="registered" aria-labelledby="registered-loops-title">
        <h3 id="registered-loops-title">Liste pregătite</h3>
        {#each definitions as definition (definition.id)}
          <article>
            <span><strong>{definition.label}</strong><small>{loopDescription(definition)}</small></span>
            <button type="button" title="Șterge lista pregătită" aria-label={`Șterge ${definition.label}`} onclick={() => remove(definition.id)}><IconTrash size={16} /></button>
          </article>
        {:else}
          <p>Nicio listă dinamică pregătită.</p>
        {/each}
      </section>
    </div>
  </div>
</section>

<style>
  .loop-builder { min-width: 0; min-height: 0; overflow: auto; padding: 14px; color: var(--wb-text-primary); }
  .loop-builder > header { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 12px; }
  .loop-builder > header > div { display: grid; gap: 4px; }
  .loop-builder > header span, label > span { color: var(--wb-text-muted); font-size: 12px; font-weight: 800; letter-spacing: .04em; text-transform: uppercase; }
  h2, h3, p { margin: 0; }
  h2 { color: var(--text-strong); font-size: 18px; }
  h3 { color: var(--text-strong); font-size: 13px; }
  .loop-builder > header p, .registered > p { color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  .loop-builder > header > strong { padding: 6px 9px; border-radius: 999px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 12px; }
  .builder-grid { display: grid; grid-template-columns: minmax(340px, .8fr) minmax(380px, 1.2fr); gap: 12px; }
  .loop-form, .sandbox, .registered { display: grid; gap: 10px; min-width: 0; padding: 13px; border-radius: 8px; background: var(--wb-surface-chrome); }
  label { display: grid; gap: 5px; min-width: 0; }
  label small { color: var(--wb-text-muted); font-size: 12px; line-height: 1.4; }
  input { width: 100%; min-width: 0; height: 34px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--wb-radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font: inherit; font-size: 12px; }
  .field-pair { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  .primary-action { display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 34px; border: 1px solid var(--wb-accent); border-radius: var(--wb-radius-control); color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 12px; font-weight: 800; }
  .preview-column { display: grid; align-content: start; gap: 12px; min-width: 0; }
  .sandbox-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 7px; }
  .sandbox-grid article { display: grid; gap: 4px; min-width: 0; padding: 9px; border-radius: 7px; background: var(--wb-surface-document); }
  .sandbox-grid strong, .registered article strong { overflow: hidden; color: var(--text-strong); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .sandbox-grid span, .registered article small { color: var(--wb-text-muted); font-size: 12px; line-height: 1.4; }
  details { overflow: hidden; border-radius: 7px; background: var(--wb-surface-document); }
  summary { min-height: 32px; padding: 8px 10px; color: var(--text-strong); font-size: 12px; font-weight: 750; cursor: pointer; }
  pre { max-height: 220px; margin: 0; padding: 10px; overflow: auto; border-top: 1px solid var(--wb-border-subtle); color: var(--wb-text-primary); font-size: 12px; line-height: 1.45; white-space: pre-wrap; }
  .registered article { display: grid; grid-template-columns: minmax(0, 1fr) 32px; align-items: center; gap: 8px; min-height: 48px; padding: 7px 8px; border-radius: 7px; background: var(--wb-surface-document); }
  .registered article > span { display: grid; gap: 3px; min-width: 0; }
  .registered article button { display: grid; width: 32px; height: 32px; place-items: center; padding: 0; border: 0; border-radius: var(--wb-radius-control); color: var(--wb-text-muted); background: transparent; }
  button:not(:disabled) { cursor: pointer; }
  button:focus-visible, input:focus-visible, summary:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  @media (max-width: 980px) { .builder-grid { grid-template-columns: 1fr; } }
</style>
