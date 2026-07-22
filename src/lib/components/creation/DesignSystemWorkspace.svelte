<script lang="ts">
  import {
    IconExternalLink,
    IconFileTypeCss,
    IconPalette,
    IconSearch,
    IconTags,
    IconTypography,
  } from "@tabler/icons-svelte";
  import { getFontInventory } from "$lib/project/io";
  import type { AppState } from "$lib/state/app.svelte";
  import type { FontInventory, ScssVariable, SourceGraphStyle } from "$lib/types";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type DesignView = "tokens" | "classes" | "styles" | "fonts";
  type TokenCategory = "all" | "color" | "type" | "space" | "breakpoint" | "other";

  let activeView = $state<DesignView>("tokens");
  let category = $state<TokenCategory>("all");
  let query = $state("");
  let selectedTokenKey = $state("");
  let selectedStyleId = $state("");
  let selectedClassName = $state("");
  let draftKey = $state("");
  let draftValue = $state("");
  let saving = $state(false);
  let renameKey = $state("");
  let renameDraft = $state("");
  let renameError = $state("");
  let renaming = $state(false);
  let fontInventory = $state<FontInventory | null>(null);
  let fontError = $state("");

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase("ro"));
  const filteredTokens = $derived(
    app.scssVariables.filter((variable) => {
      const tokenCategory = variableCategory(variable);
      return (category === "all" || tokenCategory === category)
        && (!normalizedQuery || `${variable.name} ${variable.value} ${variable.file}`
          .toLocaleLowerCase("ro")
          .includes(normalizedQuery));
    }),
  );
  const styles = $derived(
    (app.sourceGraph?.styles ?? []).filter((style) => (
      !normalizedQuery
      || `${style.file} ${style.scope}`.toLocaleLowerCase("ro").includes(normalizedQuery)
    )),
  );
  const classes = $derived(
    (app.designClassInventory?.classes ?? []).filter((entry) => (
      !normalizedQuery
      || `${entry.name} ${entry.files.join(" ")}`.toLocaleLowerCase("ro").includes(normalizedQuery)
    )),
  );
  const selectedToken = $derived(
    app.scssVariables.find((variable) => tokenKey(variable) === selectedTokenKey)
      ?? filteredTokens[0]
      ?? null,
  );
  const selectedStyle = $derived(
    (app.sourceGraph?.styles ?? []).find((style) => style.id === selectedStyleId)
      ?? styles[0]
      ?? null,
  );
  const selectedClass = $derived(
    (app.designClassInventory?.classes ?? []).find((entry) => entry.name === selectedClassName)
      ?? classes[0]
      ?? null,
  );
  $effect(() => {
    const token = selectedToken;
    const key = token ? tokenKey(token) : "";
    if (key === draftKey) return;
    draftKey = key;
    draftValue = token?.value ?? "";
  });

  $effect(() => {
    const entry = selectedClass;
    const key = entry?.name ?? "";
    if (key === renameKey) return;
    renameKey = key;
    renameDraft = key;
    renameError = "";
  });

  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    void app.refreshDesignClassInventory();
  });

  $effect(() => {
    const sessionId = app.kernelProjectSessionId;
    if (!sessionId) return;
    fontInventory = null;
    fontError = "";
    void getFontInventory()
      .then((inventory) => {
        if (app.kernelProjectSessionId === sessionId) fontInventory = inventory;
      })
      .catch((error) => {
        if (app.kernelProjectSessionId === sessionId) {
          fontError = error instanceof Error ? error.message : String(error);
        }
      });
  });

  function tokenKey(variable: ScssVariable) {
    return `${variable.file}\u0000${variable.name}`;
  }

  function variableCategory(variable: ScssVariable): Exclude<TokenCategory, "all"> {
    const name = variable.name.toLocaleLowerCase("ro");
    const value = variable.value.toLocaleLowerCase("ro");
    if (/culoare|color|fundal|background|accent|border/.test(name) || /^(#|rgb|hsl|oklch|color-mix)/.test(value)) return "color";
    if (/font|text|linie|line-height|greutate|weight/.test(name)) return "type";
    if (/spatiu|space|gap|padding|margin|radius|raza/.test(name)) return "space";
    if (/^bp-|breakpoint/.test(name)) return "breakpoint";
    return "other";
  }

  function categoryLabel(value: ReturnType<typeof variableCategory>) {
    if (value === "color") return "Culoare";
    if (value === "type") return "Tipografie";
    if (value === "space") return "Spațiere";
    if (value === "breakpoint") return "Breakpoint";
    return "Altele";
  }

  function styleUsageCount(style: SourceGraphStyle) {
    return (app.sourceGraph?.relations ?? []).filter((relation) => (
      relation.to === style.nodeId && relation.kind === "usesStyle"
    )).length;
  }

  async function saveToken() {
    const token = selectedToken;
    if (!token || saving || !draftValue.trim() || draftValue.trim() === token.value) return;
    saving = true;
    try {
      await app.updateDesignSystemVariable(token, draftValue);
    } catch (error) {
      app.setGlobalStatus(
        `Tokenul nu a putut fi actualizat: ${error instanceof Error ? error.message : String(error)}`,
        "error",
      );
      draftValue = token.value;
    } finally {
      saving = false;
    }
  }

  async function renameClass() {
    const entry = selectedClass;
    const nextName = renameDraft.trim().replace(/^\./, "");
    if (!entry || renaming || !nextName || nextName === entry.name) return;
    renameError = "";
    renaming = true;
    try {
      const changed = await app.renameDesignSystemClass(entry.name, nextName);
      if (changed) selectedClassName = nextName;
    } catch (error) {
      renameError = error instanceof Error ? error.message : String(error);
    } finally {
      renaming = false;
    }
  }

  function selectView(view: DesignView) {
    activeView = view;
  }

  const designViews: { id: DesignView; label: string }[] = [
    { id: "tokens", label: "Tokeni" },
    { id: "classes", label: "Clase" },
    { id: "styles", label: "Stylesheets" },
    { id: "fonts", label: "Fonturi" },
  ];

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + designViews.length) % designViews.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % designViews.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = designViews.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const next = designViews[nextIndex];
    if (!next) return;
    selectView(next.id);
    requestAnimationFrame(() => document.getElementById(`design-tab-${next.id}`)?.focus());
  }
</script>

<section class="design-workspace" aria-labelledby="design-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconPalette size={15} stroke={1.9} /> Spațiu de design</span>
      <h1 id="design-title">Sistem de design</h1>
      <p>Tokenii SCSS, foile de stil și fonturile locale rămân în cod și sunt citite din sesiunea proiectului.</p>
    </div>
    <dl>
      <div><dt>Tokeni</dt><dd>{app.scssVariables.length}</dd></div>
      <div><dt>Clase</dt><dd>{app.designClassInventory?.classes.length ?? 0}</dd></div>
      <div><dt>Stylesheets</dt><dd>{app.sourceGraph?.styles.length ?? 0}</dd></div>
      <div><dt>Fonturi</dt><dd>{fontInventory?.families.length ?? 0}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="view-tabs" role="tablist" aria-label="Zonele sistemului de design">
      {#each designViews as view, index (view.id)}
        <button
          id={`design-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          aria-controls={`design-panel-${view.id}`}
          tabindex={activeView === view.id ? 0 : -1}
          class:active={activeView === view.id}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}</button>
      {/each}
    </div>
    {#if activeView === "tokens"}
      <label class="category-field">
        <span class="sr-only">Categorie token</span>
        <select bind:value={category} aria-label="Categorie token">
          <option value="all">Toate categoriile</option>
          <option value="color">Culori</option>
          <option value="type">Tipografie</option>
          <option value="space">Spațiere</option>
          <option value="breakpoint">Breakpoints</option>
          <option value="other">Altele</option>
        </select>
      </label>
    {/if}
    <label class="search-field">
      <span class="sr-only">Caută în sistemul de design</span>
      <IconSearch size={14} stroke={1.9} />
      <input bind:value={query} type="search" placeholder="Caută token, fișier sau font" />
    </label>
  </div>

  <div class="workspace-body">
    <div class="resource-list" id={`design-panel-${activeView}`} role="tabpanel" aria-labelledby={`design-tab-${activeView}`}>
      {#if activeView === "tokens"}
        {#each filteredTokens as variable (tokenKey(variable))}
          <button
            type="button"
            class="token-row"
            class:selected={selectedToken && tokenKey(selectedToken) === tokenKey(variable)}
            onclick={() => { selectedTokenKey = tokenKey(variable); }}
          >
            <span class="token-kind">{categoryLabel(variableCategory(variable))}</span>
            <span><strong>${variable.name}</strong><small>{variable.file}</small></span>
            <code>{variable.value}</code>
          </button>
        {:else}
          <div class="workspace-state">Nu există tokeni pentru filtrul curent.</div>
        {/each}
      {:else if activeView === "classes"}
        {#if app.designClassInventoryError}
          <div class="workspace-state error" role="alert">{app.designClassInventoryError}</div>
        {:else if app.designClassInventoryLoading && !app.designClassInventory}
          <div class="workspace-state">Se construiește inventarul de clase prin Rust…</div>
        {:else}
          {#each classes as entry (entry.name)}
            <button
              type="button"
              class="class-row"
              class:selected={selectedClass?.name === entry.name}
              onclick={() => { selectedClassName = entry.name; }}
            >
              <span class="resource-icon"><IconTags size={16} stroke={1.8} /></span>
              <span><strong>.{entry.name}</strong><small>{entry.files.length} fișiere</small></span>
              <code>{entry.markupOccurrences} markup</code>
              <small>{entry.selectorOccurrences} selectori</small>
            </button>
          {:else}
            <div class="workspace-state">Nu există clase pentru filtrul curent.</div>
          {/each}
        {/if}
      {:else if activeView === "styles"}
        {#each styles as style (style.id)}
          <button
            type="button"
            class="style-row"
            class:selected={selectedStyle?.id === style.id}
            onclick={() => { selectedStyleId = style.id; }}
          >
            <span class="resource-icon"><IconFileTypeCss size={16} stroke={1.8} /></span>
            <span><strong>{style.file.split("/").at(-1)}</strong><small>{style.file}</small></span>
            <code>{style.scope}</code>
            <small>{styleUsageCount(style)} utilizări</small>
          </button>
        {:else}
          <div class="workspace-state">Nu există stylesheet-uri pentru filtrul curent.</div>
        {/each}
      {:else if fontError}
        <div class="workspace-state error" role="alert">{fontError}</div>
      {:else if fontInventory}
        {#each fontInventory.families.filter((family) => !normalizedQuery || `${family.family} ${family.directory}`.toLocaleLowerCase("ro").includes(normalizedQuery)) as family (`${family.origin}:${family.directory}`)}
          <article class="font-row">
            <span class="resource-icon"><IconTypography size={16} stroke={1.8} /></span>
            <div><strong>{family.family}</strong><small>{family.directory}</small></div>
            <span>{family.files.length} fișiere</span>
            <span>{family.origin === "local" ? "Local" : `Temă · ${family.themeName ?? "activă"}`}</span>
          </article>
        {:else}
          <div class="workspace-state">Nu există fonturi pentru filtrul curent.</div>
        {/each}
      {:else}
        <div class="workspace-state">Se scanează fonturile locale prin Rust…</div>
      {/if}
    </div>

    <aside class="resource-detail" aria-label="Detaliile sistemului de design">
      {#if activeView === "tokens" && selectedToken}
        <span class="detail-kicker">{categoryLabel(variableCategory(selectedToken))}</span>
        <h2>${selectedToken.name}</h2>
        <p>Modificarea este validată și aplicată prin comanda CSS Rust, apoi proiectată în previzualizare.</p>
        <label class="value-editor">
          <span>Valoare SCSS</span>
          <input bind:value={draftValue} disabled={saving} onkeydown={(event) => { if (event.key === "Enter") void saveToken(); }} />
        </label>
        <div class="source-card"><span>Sursă</span><code>{selectedToken.file}</code></div>
        <button class="primary-action" type="button" disabled={saving || !draftValue.trim() || draftValue.trim() === selectedToken.value} onclick={() => { void saveToken(); }}>
          {saving ? "Se aplică prin Rust…" : "Actualizează tokenul"}
        </button>
        <button class="secondary-action" type="button" onclick={() => { void openWorkspaceSource(selectedToken.file); }}>Deschide sursa <IconExternalLink size={13} stroke={1.9} /></button>
      {:else if activeView === "classes" && selectedClass}
        <span class="detail-kicker">Class inventory</span>
        <h2>.{selectedClass.name}</h2>
        <p>{selectedClass.markupOccurrences} utilizări în markup și {selectedClass.selectorOccurrences} selectori sunt confirmați de ProjectModel.</p>
        <label class="value-editor">
          <span>Nume nou</span>
          <input bind:value={renameDraft} disabled={renaming} onkeydown={(event) => { if (event.key === "Enter") void renameClass(); }} />
        </label>
        {#if renameError}<p class="rename-error" role="alert">{renameError}</p>{/if}
        <button class="primary-action" type="button" disabled={renaming || !renameDraft.trim() || renameDraft.replace(/^\./, "") === selectedClass.name} onclick={() => { void renameClass(); }}>
          {renaming ? "Se redenumește prin Rust…" : "Redenumește în siguranță"}
        </button>
        <div class="occurrence-list" aria-label="Aparițiile clasei">
          {#each selectedClass.occurrences.slice(0, 40) as occurrence (`${occurrence.file}:${occurrence.range.start}`)}
            <button type="button" onclick={() => { void openWorkspaceSource(occurrence.file); }}>
              <span>{occurrence.kind === "markup" ? "Markup" : "Selector"}</span>
              <code>{occurrence.file}:{occurrence.range.line}:{occurrence.range.column}</code>
            </button>
          {/each}
        </div>
      {:else if activeView === "styles" && selectedStyle}
        <span class="detail-kicker">Stylesheet {selectedStyle.scope}</span>
        <h2>{selectedStyle.file.split("/").at(-1)}</h2>
        <p>{styleUsageCount(selectedStyle)} relații `usesStyle` sunt indexate în harta surselor.</p>
        <div class="source-card"><span>Cale</span><code>{selectedStyle.file}</code></div>
        <button class="primary-action" type="button" onclick={() => { void openWorkspaceSource(selectedStyle.file); }}>Deschide în Code <IconExternalLink size={13} stroke={1.9} /></button>
      {:else if activeView === "fonts"}
        <span class="detail-kicker">Font inventory</span>
        <h2>Fonturi locale</h2>
        <p>Inventarul Rust include resursele binare pregătite în sesiunea proiectului și exclude resursele șterse.</p>
      {:else}
        <div class="workspace-state">Selectează o resursă.</div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .design-workspace { display: grid; grid-template-rows: auto 42px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 10px; color: var(--wb-text-primary); background: var(--wb-surface-document); box-shadow: var(--shadow); }
  .workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: radial-gradient(circle at 18% 0%, var(--wb-accent-soft), transparent 36%), var(--wb-surface-chrome); }
  .eyebrow { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 850; letter-spacing: .06em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 24px; letter-spacing: -.025em; }
  .workspace-header p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; }
  .workspace-header > dl { display: flex; gap: 7px; margin: 0; }
  .workspace-header > dl div { min-width: 82px; padding: 7px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  dt { color: var(--wb-text-muted); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 800; }
  .workspace-toolbar, .view-tabs, .search-field, .token-row, .class-row, .style-row, .font-row, .primary-action, .secondary-action { display: flex; align-items: center; }
  .workspace-toolbar { justify-content: flex-end; gap: 8px; padding: 5px 9px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .view-tabs { align-self: stretch; gap: 2px; margin-right: auto; }
  .view-tabs button { height: 100%; padding: 0 10px; border: 0; border-bottom: 2px solid transparent; color: var(--wb-text-muted); background: transparent; font-size: 12px; font-weight: 800; }
  .view-tabs button.active { border-bottom-color: var(--wb-accent); color: var(--wb-text-primary); }
  .search-field { position: relative; width: min(280px, 32vw); }
  .search-field :global(svg) { position: absolute; left: 8px; color: var(--wb-text-muted); }
  .search-field input, .category-field select { height: 28px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .search-field input { width: 100%; padding: 0 8px 0 28px; }
  .category-field select { min-width: 124px; padding: 0 7px; }
  .workspace-body { display: grid; grid-template-columns: minmax(340px, 1fr) minmax(290px, .58fr); min-width: 0; min-height: 0; }
  .resource-list { min-width: 0; min-height: 0; overflow: auto; padding: 8px; border-right: 1px solid var(--wb-border-subtle); }
  .token-row, .class-row, .style-row { display: grid; grid-template-columns: 82px minmax(0, 1fr) minmax(130px, auto); gap: 9px; width: 100%; min-height: 52px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .class-row, .style-row { grid-template-columns: 34px minmax(0, 1fr) auto 70px; }
  .token-row:hover, .token-row.selected, .class-row:hover, .class-row.selected, .style-row:hover, .style-row.selected { border-color: var(--wb-border-subtle); background: var(--wb-control-hover); }
  .token-row.selected, .class-row.selected, .style-row.selected { box-shadow: inset 3px 0 0 var(--wb-accent); }
  .token-kind { color: var(--wb-accent-strong); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .token-row > span:nth-child(2), .class-row > span:nth-child(2), .style-row > span:nth-child(2), .font-row > div { display: grid; gap: 3px; min-width: 0; }
  .token-row strong, .class-row strong, .style-row strong, .font-row strong { color: var(--text-strong); font-size: 12px; }
  .token-row small, .class-row small, .style-row small, .font-row small { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .token-row code, .class-row code, .style-row code { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-align: right; text-overflow: ellipsis; white-space: nowrap; }
  .resource-icon { display: grid; width: 29px; height: 29px; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .font-row { display: grid; grid-template-columns: 34px minmax(0, 1fr) auto 100px; gap: 8px; min-height: 52px; padding: 7px 9px; border-bottom: 1px solid var(--wb-border-subtle); }
  .font-row > span { color: var(--wb-text-muted); font-size: 12px; }
  .resource-detail { min-width: 0; min-height: 0; overflow: auto; padding: 17px; background: var(--wb-surface-chrome); }
  .detail-kicker { color: var(--wb-accent-strong); font-size: 12px; font-weight: 850; text-transform: uppercase; }
  h2 { margin: 7px 0 0; color: var(--text-strong); font-size: 19px; }
  .resource-detail > p { margin: 6px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.5; }
  .value-editor { display: grid; gap: 5px; margin-top: 15px; color: var(--wb-text-muted); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .value-editor input { height: 34px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--text-strong); background: var(--wb-surface-document); font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 12px; }
  .source-card { display: grid; gap: 4px; margin-top: 9px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .source-card span { color: var(--wb-text-muted); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .source-card code { overflow: hidden; color: var(--wb-text-primary); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .rename-error { margin: 8px 0 0; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger) 36%, var(--wb-border-subtle)); border-radius: 6px; color: var(--danger); background: color-mix(in srgb, var(--danger) 7%, var(--wb-surface-document)); font-size: 12px; line-height: 1.4; }
  .occurrence-list { display: grid; max-height: 270px; margin-top: 10px; overflow: auto; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .occurrence-list button { display: grid; gap: 3px; padding: 7px 8px; border: 0; border-bottom: 1px solid var(--wb-border-subtle); color: var(--wb-text-primary); background: transparent; text-align: left; }
  .occurrence-list button:last-child { border-bottom: 0; }
  .occurrence-list button:hover { background: var(--wb-control-hover); }
  .occurrence-list span { color: var(--wb-accent-strong); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .occurrence-list code { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .primary-action, .secondary-action { justify-content: center; gap: 6px; width: 100%; min-height: 32px; margin-top: 8px; border: 1px solid var(--wb-accent); border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 12px; font-weight: 800; }
  .secondary-action { border-color: var(--wb-border-subtle); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  button:disabled { opacity: .5; }
  button:not(:disabled) { cursor: pointer; }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .workspace-state { display: grid; min-height: 180px; place-items: center; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .workspace-state.error { color: var(--danger); }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .resource-detail { display: none; } .resource-list { border-right: 0; } }
</style>
