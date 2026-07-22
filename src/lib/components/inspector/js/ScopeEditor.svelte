<script lang="ts">
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionScopeItem } from "$lib/types";

  type ScopeMediaQuery = PanaMotionScopeItem["mediaQueries"][number];

  let {
    scope,
    onChange,
  }: {
    scope: PanaMotionScopeItem;
    onChange: (item: PanaMotionScopeItem) => void;
  } = $props();

  const defaultsText = $derived(formatDefaults(scope.defaults));
  const reducedMotionOptions: PanaMotionScopeItem["reducedMotion"][] = ["respect", "disable", "ignore"];

  function patch(patchValue: Partial<PanaMotionScopeItem>) {
    onChange({ ...scope, ...patchValue });
  }

  function formatDefaults(defaults: Record<string, string>): string {
    return Object.entries(defaults ?? {}).map(([key, value]) => `${key}=${value}`).join("\n");
  }

  function parseDefaults(value: string): Record<string, string> {
    return Object.fromEntries(
      value
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean)
        .map((line) => {
          const separatorIndex = line.indexOf("=");
          if (separatorIndex === -1) return [line, ""];
          return [line.slice(0, separatorIndex).trim(), line.slice(separatorIndex + 1).trim()];
        })
        .filter(([key]) => key.length > 0),
    );
  }

  function addMediaQuery() {
    patch({
      mediaQueries: [
        ...scope.mediaQueries,
        { id: `mq-${scope.mediaQueries.length + 1}`, query: "(min-width: 768px)", enabled: true },
      ],
    });
  }

  function updateMediaQuery(index: number, patchValue: Partial<ScopeMediaQuery>) {
    patch({
      mediaQueries: scope.mediaQueries.map((query, currentIndex) => currentIndex === index ? { ...query, ...patchValue } : query),
    });
  }

  function removeMediaQuery(index: number) {
    patch({ mediaQueries: scope.mediaQueries.filter((_, currentIndex) => currentIndex !== index) });
  }
</script>

<div class="scope-editor">
  <section class="editor-card">
    <div class="section-head"><span>Scope</span></div>
    <div class="field-grid">
      <label><span>Root</span><input class="mono" value={scope.root} placeholder=".component-root" oninput={(event) => patch({ root: event.currentTarget.value })} /></label>
      <label>
        <span>Reduced motion</span>
        <SelectControl value={scope.reducedMotion} options={reducedMotionOptions} ariaLabel="Reduced motion" onchange={(value) => patch({ reducedMotion: value as PanaMotionScopeItem["reducedMotion"] })} />
      </label>
    </div>
    <div class="toggle-grid">
      <button type="button" class:active={scope.keepTime} onclick={() => patch({ keepTime: !scope.keepTime })}>keepTime</button>
    </div>
  </section>

  <section class="editor-card">
    <div class="section-head"><span>Defaults</span></div>
    <label>
      <span>key=value pe linie</span>
      <textarea class="mono" value={defaultsText} placeholder="duration=600&#10;ease=outQuad" oninput={(event) => patch({ defaults: parseDefaults(event.currentTarget.value) })}></textarea>
    </label>
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Media queries</span>
      <button type="button" onclick={addMediaQuery}>+ query</button>
    </div>
    {#each scope.mediaQueries as query, index}
      <div class="media-row">
        <input class="mono" value={query.id} placeholder="id" oninput={(event) => updateMediaQuery(index, { id: event.currentTarget.value })} />
        <input class="mono" value={query.query} placeholder="(min-width: 768px)" oninput={(event) => updateMediaQuery(index, { query: event.currentTarget.value })} />
        <button type="button" class:active={query.enabled} onclick={() => updateMediaQuery(index, { enabled: !query.enabled })}>{query.enabled ? "on" : "off"}</button>
        <button type="button" class="remove-btn" onclick={() => removeMediaQuery(index)}>×</button>
      </div>
    {/each}
  </section>
</div>

<style>
  .scope-editor,
  .editor-card {
    display: flex;
    flex-direction: column;
  }

  .scope-editor {
    gap: 8px;
  }

  .editor-card {
    gap: 7px;
    padding: 9px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-4);
  }

  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .section-head span,
  label span {
    display: block;
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .toggle-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 5px;
  }

  .media-row {
    display: grid;
    grid-template-columns: 0.8fr 1.4fr 38px 24px;
    gap: 4px;
    align-items: center;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  input,
  textarea,
  button {
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 12px;
  }

  input,
  button {
    min-height: 25px;
  }

  input,
  textarea {
    width: 100%;
    min-width: 0;
    padding: 0 6px;
  }

  textarea {
    min-height: 68px;
    padding-block: 6px;
    resize: vertical;
  }

  button {
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .remove-btn {
    color: var(--danger);
  }

  .mono {
    font-family: "JetBrains Mono", monospace;
  }
</style>
