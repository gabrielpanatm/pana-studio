<script lang="ts">
  import {
    IconAlertTriangle,
    IconDeviceFloppy,
    IconEdit,
    IconExternalLink,
    IconFileTypeCss,
    IconPalette,
    IconPlus,
    IconSearch,
    IconTags,
    IconTypography,
    IconX,
  } from "@tabler/icons-svelte";
  import {
    createProjectTextFile,
    downloadGoogleFontFamily,
    getFontInventory,
    readThemeStyleCatalog,
    semanticRenameProjectEntry,
  } from "$lib/project/io";
  import ThemeStylesWorkspace from "./ThemeStylesWorkspace.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    FileBufferRequestIdentity,
    FontInventory,
    ScssVariable,
    SourceGraphStyle,
    ThemeStyleCatalogSnapshot,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type DesignView = "global-styles" | "tokens" | "classes" | "styles" | "fonts";
  type DetailMode = "info" | "create" | "edit";
  type TokenCategory = "all" | "color" | "type" | "space" | "breakpoint" | "other";

  let activeView = $state<DesignView>("global-styles");
  let category = $state<TokenCategory>("all");
  let styleCategory = $state("all");
  let query = $state("");
  let selectedTokenKey = $state("");
  let selectedStyleId = $state("");
  let selectedClassName = $state("");
  let selectedFontKey = $state("");
  let detailMode = $state<DetailMode>("info");
  let fontInventory = $state<FontInventory | null>(null);
  let fontError = $state("");
  let formName = $state("");
  let formValue = $state("");
  let formPath = $state("");
  let formWeights = $state("400, 700");
  let formVariableFont = $state(false);
  let formError = $state("");
  let mutating = $state(false);
  let themeStyleCatalog = $state<ThemeStyleCatalogSnapshot | null>(null);
  let themeStyleLoading = $state(false);
  let themeStyleError = $state("");
  let themeStyleLoadSequence = 0;

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
  const visibleFonts = $derived(
    (fontInventory?.families ?? []).filter((family) => (
      !normalizedQuery
      || `${family.family} ${family.directory}`.toLocaleLowerCase("ro").includes(normalizedQuery)
    )),
  );
  const selectedFont = $derived(
    (fontInventory?.families ?? []).find(
      (family) => `${family.origin}:${family.directory}` === selectedFontKey,
    )
      ?? visibleFonts[0]
      ?? null,
  );
  const formReady = $derived.by(() => {
    if (activeView === "global-styles") return false;
    if (activeView === "styles") return Boolean(formPath.trim());
    if (activeView === "tokens" || activeView === "classes") {
      return Boolean(formName.trim() && formPath.trim());
    }
    return Boolean(formName.trim());
  });
  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    void app.refreshDesignClassInventory();
  });

  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    void reloadThemeStyleCatalog();
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

  function selectView(view: DesignView) {
    activeView = view;
    resetPanel();
  }

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: app.sessionProjectRoot,
      expectedSessionId: app.kernelProjectSessionId,
    };
  }

  async function reloadThemeStyleCatalog() {
    const requestId = ++themeStyleLoadSequence;
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    if (!projectRoot || !runtimeSessionId) return;
    themeStyleLoading = true;
    themeStyleError = "";
    try {
      const snapshot = await readThemeStyleCatalog(identity());
      if (
        requestId !== themeStyleLoadSequence
        || app.sessionProjectRoot !== projectRoot
        || app.kernelProjectSessionId !== runtimeSessionId
      ) return;
      themeStyleCatalog = snapshot;
      if (
        styleCategory !== "all"
        && !snapshot.categories.some((entry) => entry.id === styleCategory)
      ) styleCategory = "all";
    } catch (cause) {
      if (requestId !== themeStyleLoadSequence) return;
      themeStyleCatalog = null;
      themeStyleError = errorMessage(cause);
    } finally {
      if (requestId === themeStyleLoadSequence) themeStyleLoading = false;
    }
  }

  function resetPanel() {
    detailMode = "info";
    formName = "";
    formValue = "";
    formPath = "";
    formWeights = "400, 700";
    formVariableFont = false;
    formError = "";
  }

  function selectToken(variable: ScssVariable) {
    selectedTokenKey = tokenKey(variable);
    resetPanel();
  }

  function selectClass(name: string) {
    selectedClassName = name;
    resetPanel();
  }

  function selectStyle(id: string) {
    selectedStyleId = id;
    resetPanel();
  }

  function selectFont(origin: string, directory: string) {
    selectedFontKey = `${origin}:${directory}`;
    resetPanel();
  }

  function defaultStylePath() {
    return selectedStyle?.file
      ?? app.sourceGraph?.styles.find((style) => style.file.endsWith(".scss"))?.file
      ?? "sass/css-framework/_variabile.scss";
  }

  function beginCreate() {
    resetPanel();
    detailMode = "create";
    if (activeView === "tokens") {
      formName = "token-nou";
      formValue = "0";
      formPath = selectedToken?.file ?? defaultStylePath();
    } else if (activeView === "classes") {
      formName = "clasa-noua";
      formPath = selectedClass?.files.find((file) => /\.(?:s?css)$/i.test(file)) ?? defaultStylePath();
    } else if (activeView === "styles") {
      formName = "stil-nou.scss";
      formPath = "sass/pagini/stil-nou.scss";
    }
  }

  function beginEdit() {
    resetPanel();
    detailMode = "edit";
    if (activeView === "tokens" && selectedToken) {
      formName = selectedToken.name;
      formValue = selectedToken.value;
      formPath = selectedToken.file;
    } else if (activeView === "classes" && selectedClass) {
      formName = selectedClass.name;
    } else if (activeView === "styles" && selectedStyle) {
      formName = selectedStyle.file.split("/").at(-1) ?? selectedStyle.file;
      formPath = selectedStyle.file;
    } else {
      detailMode = "info";
    }
  }

  async function createResource() {
    if (mutating) return;
    formError = "";
    mutating = true;
    try {
      if (activeView === "tokens") {
        const created = await app.createDesignSystemVariable(formPath, formName, formValue);
        if (created) {
          selectedTokenKey = `${formPath}\u0000${formName.replace(/^\$/, "")}`;
        }
      } else if (activeView === "classes") {
        const created = await app.createDesignSystemClass(formName, formPath);
        if (created) selectedClassName = formName.replace(/^\./, "");
      } else if (activeView === "styles") {
        const receipt = await createProjectTextFile(
          formPath,
          "/* Stil nou — Pană Studio */\n",
          identity(),
        );
        await app.rescanCurrentProject(receipt.relativePath, { strict: true });
        selectedStyleId = app.sourceGraph?.styles.find((style) => style.file === receipt.relativePath)?.id ?? "";
        app.setGlobalStatus(`Stylesheet creat: ${formPath} — Ctrl+S persistă pe disc`, "unsaved");
      } else {
        const weights = formWeights
          .split(",")
          .map((weight) => Number.parseInt(weight.trim(), 10))
          .filter((weight) => Number.isInteger(weight) && weight >= 100 && weight <= 900);
        if (!formName.trim()) throw new Error("Familia fontului este obligatorie.");
        if (!formVariableFont && weights.length === 0) {
          throw new Error("Adaugă cel puțin o greutate între 100 și 900.");
        }
        await downloadGoogleFontFamily(formName.trim(), weights, formVariableFont);
        await app.rescanCurrentProject(null, { strict: true });
        fontInventory = await getFontInventory();
        app.setGlobalStatus(`Fontul ${formName.trim()} este pregătit în sesiune.`, "unsaved");
      }
      resetPanel();
    } catch (error) {
      formError = errorMessage(error);
    } finally {
      mutating = false;
    }
  }

  async function saveEdit() {
    if (mutating) return;
    formError = "";
    mutating = true;
    try {
      if (activeView === "tokens" && selectedToken) {
        await app.updateDesignSystemVariable(selectedToken, formValue);
      } else if (activeView === "classes" && selectedClass) {
        const changed = await app.renameDesignSystemClass(selectedClass.name, formName);
        if (changed) selectedClassName = formName.replace(/^\./, "");
      } else if (activeView === "styles" && selectedStyle) {
        const receipt = await semanticRenameProjectEntry(selectedStyle.file, formName, identity());
        await app.rescanCurrentProject(receipt.relativePath, { strict: true });
        selectedStyleId = app.sourceGraph?.styles.find((style) => style.file === receipt.relativePath)?.id ?? "";
        app.setGlobalStatus(`Stylesheet redenumit: ${receipt.relativePath}`, "unsaved");
      }
      resetPanel();
    } catch (error) {
      formError = errorMessage(error);
    } finally {
      mutating = false;
    }
  }

  const designViews: { id: DesignView; label: string }[] = [
    { id: "global-styles", label: "Stiluri" },
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
      <div><dt>Stiluri</dt><dd>{themeStyleCatalog?.targets.length ?? 0}</dd></div>
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
    {#if activeView === "global-styles"}
      <label class="category-field">
        <span class="sr-only">Categorie stil</span>
        <select bind:value={styleCategory} aria-label="Categorie stil">
          <option value="all">Toate categoriile</option>
          {#each themeStyleCatalog?.categories ?? [] as entry (entry.id)}
            <option value={entry.id}>{entry.label} ({entry.targetCount})</option>
          {/each}
        </select>
      </label>
    {:else if activeView === "tokens"}
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
      <input
        bind:value={query}
        type="search"
        placeholder={activeView === "global-styles" ? "Caută stil sau selector" : "Caută token, fișier sau font"}
      />
    </label>
    {#if activeView !== "global-styles"}
      <button class="toolbar-action" type="button" disabled={mutating} onclick={beginCreate}>
        <IconPlus size={14} stroke={2} /> Adaugă
      </button>
    {/if}
  </div>

  {#if activeView === "global-styles"}
    <ThemeStylesWorkspace
      {app}
      catalog={themeStyleCatalog}
      loading={themeStyleLoading}
      error={themeStyleError}
      {query}
      category={styleCategory}
      reload={reloadThemeStyleCatalog}
      {openWorkspaceSource}
    />
  {:else}
    <div class="workspace-body">
    <div class="resource-list" id={`design-panel-${activeView}`} role="tabpanel" aria-labelledby={`design-tab-${activeView}`}>
      {#if activeView === "tokens"}
        {#each filteredTokens as variable (tokenKey(variable))}
          <button
            type="button"
            class="token-row"
            class:selected={selectedToken && tokenKey(selectedToken) === tokenKey(variable)}
            onclick={() => selectToken(variable)}
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
              onclick={() => selectClass(entry.name)}
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
            onclick={() => selectStyle(style.id)}
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
        {#each visibleFonts as family (`${family.origin}:${family.directory}`)}
          <button
            type="button"
            class="font-row"
            class:selected={selectedFont?.directory === family.directory && selectedFont?.origin === family.origin}
            onclick={() => selectFont(family.origin, family.directory)}
          >
            <span class="resource-icon"><IconTypography size={16} stroke={1.8} /></span>
            <div><strong>{family.family}</strong><small>{family.directory}</small></div>
            <span>{family.files.length} fișiere</span>
            <span>{family.origin === "local" ? "Local" : `Temă · ${family.themeName ?? "activă"}`}</span>
          </button>
        {:else}
          <div class="workspace-state">Nu există fonturi pentru filtrul curent.</div>
        {/each}
      {:else}
        <div class="workspace-state">Se scanează fonturile locale prin Rust…</div>
      {/if}
    </div>

    <aside class="resource-detail" aria-label="Panou contextual sistem de design">
      {#if detailMode === "create"}
        <form class="resource-form" onsubmit={(event) => { event.preventDefault(); void createResource(); }}>
          <header class="detail-heading">
            <div>
              <span class="detail-kicker">Resursă nouă</span>
              <h2>Adaugă {designViews.find((view) => view.id === activeView)?.label.toLocaleLowerCase("ro")}</h2>
              <p>Crearea este validată și jurnalizată în ProjectWorkspace înainte de salvarea pe disc.</p>
            </div>
            <button type="button" aria-label="Renunță la creare" disabled={mutating} onclick={resetPanel}><IconX size={14} /></button>
          </header>

          {#if activeView === "tokens"}
            <label><span>Nume token</span><input bind:value={formName} disabled={mutating} placeholder="color-accent" /></label>
            <label><span>Valoare SCSS</span><input bind:value={formValue} disabled={mutating} placeholder="#16836f" /></label>
            <label><span>Fișier SCSS</span><input bind:value={formPath} disabled={mutating} /></label>
          {:else if activeView === "classes"}
            <label><span>Nume clasă</span><input bind:value={formName} disabled={mutating} placeholder="card-serviciu" /></label>
            <label><span>Stylesheet destinație</span><input bind:value={formPath} disabled={mutating} /></label>
          {:else if activeView === "styles"}
            <label><span>Cale în proiect</span><input bind:value={formPath} disabled={mutating} placeholder="sass/pagini/stil-nou.scss" /></label>
          {:else}
            <label><span>Familie Google Fonts</span><input bind:value={formName} disabled={mutating} placeholder="Space Grotesk" /></label>
            <label><span>Greutăți</span><input bind:value={formWeights} disabled={mutating || formVariableFont} placeholder="400, 600, 700" /></label>
            <label class="check-field"><input bind:checked={formVariableFont} type="checkbox" disabled={mutating} /><span>Descarcă varianta variabilă</span></label>
          {/if}

          {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button type="button" disabled={mutating} onclick={resetPanel}>Renunță</button>
            <button class="primary" type="submit" disabled={mutating || !formReady}>
              <IconPlus size={14} /> {mutating ? "Se creează prin Rust…" : "Creează în sesiune"}
            </button>
          </div>
        </form>
      {:else if detailMode === "edit"}
        <form class="resource-form" onsubmit={(event) => { event.preventDefault(); void saveEdit(); }}>
          <header class="detail-heading">
            <div>
              <span class="detail-kicker">Modificare controlată</span>
              <h2>
                {activeView === "tokens" && selectedToken ? `$${selectedToken.name}`
                  : activeView === "classes" && selectedClass ? `.${selectedClass.name}`
                    : selectedStyle?.file.split("/").at(-1) ?? "Resursă"}
              </h2>
              <p>Modificarea devine o singură tranzacție în istoricul ProjectWorkspace.</p>
            </div>
            <button type="button" aria-label="Renunță la editare" disabled={mutating} onclick={resetPanel}><IconX size={14} /></button>
          </header>

          {#if activeView === "tokens"}
            <label><span>Valoare SCSS</span><input bind:value={formValue} disabled={mutating} /></label>
            <div class="source-card"><span>Sursă</span><code>{formPath}</code></div>
          {:else if activeView === "classes"}
            <label><span>Nume clasă</span><input bind:value={formName} disabled={mutating} /></label>
          {:else}
            <label><span>Nume fișier</span><input bind:value={formName} disabled={mutating} /></label>
            <div class="source-card"><span>Cale curentă</span><code>{formPath}</code></div>
          {/if}

          {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button type="button" disabled={mutating} onclick={resetPanel}>Renunță</button>
            <button class="primary" type="submit" disabled={mutating}>
              <IconDeviceFloppy size={14} /> {mutating ? "Se actualizează prin Rust…" : "Salvează modificările"}
            </button>
          </div>
        </form>
      {:else if activeView === "tokens" && selectedToken}
        <span class="detail-kicker">{categoryLabel(variableCategory(selectedToken))}</span>
        <h2>${selectedToken.name}</h2>
        <p>Valoarea este citită din inventarul SCSS canonic și nu intră în editare la simpla selectare.</p>
        <dl class="info-grid">
          <div><dt>Valoare</dt><dd>{selectedToken.value}</dd></div>
          <div><dt>Categorie</dt><dd>{categoryLabel(variableCategory(selectedToken))}</dd></div>
        </dl>
        <div class="source-card"><span>Sursă</span><code>{selectedToken.file}</code></div>
        <div class="detail-actions">
          <button class="primary-action" type="button" onclick={beginEdit}><IconEdit size={14} /> Editează</button>
          <button class="secondary-action" type="button" onclick={() => { void openWorkspaceSource(selectedToken.file); }}>Deschide sursa <IconExternalLink size={13} /></button>
        </div>
      {:else if activeView === "classes" && selectedClass}
        <span class="detail-kicker">Inventar clase</span>
        <h2>.{selectedClass.name}</h2>
        <p>{selectedClass.markupOccurrences} utilizări în markup și {selectedClass.selectorOccurrences} selectori sunt confirmați de ProjectModel.</p>
        <dl class="info-grid">
          <div><dt>Markup</dt><dd>{selectedClass.markupOccurrences}</dd></div>
          <div><dt>Selectori</dt><dd>{selectedClass.selectorOccurrences}</dd></div>
        </dl>
        <div class="detail-actions">
          <button class="primary-action" type="button" onclick={beginEdit}><IconEdit size={14} /> Editează</button>
        </div>
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
        <div class="detail-actions">
          <button class="primary-action" type="button" onclick={beginEdit}><IconEdit size={14} /> Editează</button>
          <button class="secondary-action" type="button" onclick={() => { void openWorkspaceSource(selectedStyle.file); }}>Deschide în Editor <IconExternalLink size={13} /></button>
        </div>
      {:else if activeView === "fonts" && selectedFont}
        <span class="detail-kicker">Inventar fonturi</span>
        <h2>{selectedFont.family}</h2>
        <p>Familia și fișierele sale provin din inventarul Rust al resurselor binare din sesiune.</p>
        <dl class="info-grid">
          <div><dt>Origine</dt><dd>{selectedFont.origin === "local" ? "Local" : selectedFont.themeName ?? "Temă"}</dd></div>
          <div><dt>Fișiere</dt><dd>{selectedFont.files.length}</dd></div>
        </dl>
        <div class="source-card"><span>Director</span><code>{selectedFont.directory}</code></div>
      {:else}
        <div class="workspace-state">Selectează o resursă.</div>
      {/if}
    </aside>
    </div>
  {/if}
</section>

<style>
  .design-workspace { display: grid; grid-template-rows: auto 42px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-panel); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .eyebrow { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 20px; font-weight: 650; letter-spacing: -.015em; }
  .workspace-header p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; }
  .workspace-header > dl { display: flex; gap: 7px; margin: 0; }
  .workspace-header > dl div { min-width: 82px; padding: 7px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  dt { color: var(--wb-text-muted); font-size: 12px; font-weight: 650; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 650; }
  .workspace-toolbar, .view-tabs, .search-field, .token-row, .class-row, .style-row, .font-row, .primary-action, .secondary-action, .toolbar-action, .detail-heading, .form-error, .form-actions, .detail-actions { display: flex; align-items: center; }
  .workspace-toolbar { justify-content: flex-end; gap: 8px; padding: 5px 9px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .view-tabs { align-self: stretch; gap: 2px; margin-right: auto; }
  .view-tabs button { height: 100%; padding: 0 10px; border: 0; border-bottom: 2px solid transparent; color: var(--wb-text-muted); background: transparent; font-size: 12px; font-weight: 600; }
  .view-tabs button.active { border-bottom-color: var(--wb-accent); color: var(--wb-accent-strong); }
  .search-field { position: relative; width: min(280px, 32vw); }
  .search-field :global(svg) { position: absolute; left: 8px; color: var(--wb-text-muted); }
  .search-field input, .category-field select { height: 28px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .search-field input { width: 100%; padding: 0 8px 0 28px; }
  .category-field select { min-width: 124px; padding: 0 7px; }
  .toolbar-action { flex: 0 0 auto; justify-content: center; gap: 5px; min-height: 28px; padding: 0 10px; border: 1px solid var(--wb-accent); border-radius: var(--radius-control); color: #fff; background: var(--wb-accent); font-size: 12px; font-weight: 650; }
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
  .font-row { display: grid; grid-template-columns: 34px minmax(0, 1fr) auto 100px; gap: 8px; width: 100%; min-height: 52px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .font-row:hover, .font-row.selected { border-color: var(--wb-border-subtle); background: var(--wb-control-hover); }
  .font-row.selected { box-shadow: inset 3px 0 0 var(--wb-accent); }
  .font-row > span { color: var(--wb-text-muted); font-size: 12px; }
  .resource-detail { min-width: 0; min-height: 0; overflow: auto; padding: 17px; background: var(--wb-surface-chrome); }
  .detail-kicker { color: var(--wb-accent-strong); font-size: 12px; font-weight: 850; text-transform: uppercase; }
  h2 { margin: 7px 0 0; color: var(--text-strong); font-size: 19px; }
  .resource-detail > p { margin: 6px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.5; }
  .detail-heading { align-items: flex-start; justify-content: space-between; gap: 12px; }
  .detail-heading h2 { margin-top: 5px; }
  .detail-heading p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.5; }
  .detail-heading > button { display: grid; flex: 0 0 auto; width: 28px; height: 28px; padding: 0; place-items: center; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-muted); background: var(--wb-surface-document); }
  .resource-form { display: grid; gap: 11px; }
  .resource-form > label { display: grid; gap: 5px; color: var(--wb-text-muted); font-size: 12px; font-weight: 700; }
  .resource-form > label > input:not([type="checkbox"]) { width: 100%; height: 34px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--text-strong); background: var(--wb-surface-document); font-size: 12px; }
  .resource-form > label > input:not([type="checkbox"]):focus { border-color: var(--wb-accent); }
  .resource-form .check-field { display: flex; align-items: center; gap: 7px; min-height: 32px; }
  .check-field input { width: 15px; height: 15px; accent-color: var(--wb-accent); }
  .source-card { display: grid; gap: 4px; margin-top: 9px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .source-card span { color: var(--wb-text-muted); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .source-card code { overflow: hidden; color: var(--wb-text-primary); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .form-error { align-items: flex-start; gap: 6px; margin: 0; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger) 36%, var(--wb-border-subtle)); border-radius: 6px; color: var(--danger); background: color-mix(in srgb, var(--danger) 7%, var(--wb-surface-document)); font-size: 12px; line-height: 1.4; }
  .form-error :global(svg) { flex: 0 0 auto; margin-top: 1px; }
  .form-actions { justify-content: flex-end; gap: 7px; padding-top: 3px; }
  .form-actions button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-height: 32px; padding: 0 11px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; font-weight: 650; }
  .form-actions button.primary { border-color: var(--wb-accent); color: #fff; background: var(--wb-accent); }
  .info-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; margin: 13px 0 0; }
  .info-grid div { min-width: 0; padding: 8px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .info-grid dd { overflow: hidden; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
  .detail-actions { align-items: stretch; gap: 7px; margin-top: 10px; }
  .detail-actions .primary-action, .detail-actions .secondary-action { margin-top: 0; }
  .occurrence-list { display: grid; max-height: 270px; margin-top: 10px; overflow: auto; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .occurrence-list button { display: grid; gap: 3px; padding: 7px 8px; border: 0; border-bottom: 1px solid var(--wb-border-subtle); color: var(--wb-text-primary); background: transparent; text-align: left; }
  .occurrence-list button:last-child { border-bottom: 0; }
  .occurrence-list button:hover { background: var(--wb-control-hover); }
  .occurrence-list span { color: var(--wb-accent-strong); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .occurrence-list code { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .primary-action, .secondary-action { justify-content: center; gap: 6px; width: 100%; min-height: 32px; margin-top: 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; font-weight: 600; }
  .primary-action { border-color: var(--wb-accent); color: #fff; background: var(--wb-accent); }
  .secondary-action { border-color: var(--wb-border-subtle); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  button:disabled { opacity: .5; }
  button:not(:disabled) { cursor: pointer; }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .workspace-state { display: grid; min-height: 180px; place-items: center; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .workspace-state.error { color: var(--danger); }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .resource-detail { display: none; } .resource-list { border-right: 0; } }
</style>
