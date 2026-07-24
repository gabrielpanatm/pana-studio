<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    IconAlertTriangle,
    IconArrowBackUp,
    IconBrush,
    IconDeviceFloppy,
    IconEdit,
    IconExternalLink,
    IconPhoto,
  } from "@tabler/icons-svelte";
  import ColorInput from "$lib/components/inspector/controls/ColorInput.svelte";
  import {
    applyThemeStyleDraft,
    previewThemeStyleDraft,
  } from "$lib/project/io";
  import {
    registerEditFlushHandler,
    type EditFlushReason,
  } from "$lib/session/edit-flush-registry";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    FileBufferRequestIdentity,
    ThemeStyleCatalogSnapshot,
    ThemeStyleDraftPreview,
    ThemeStylePropertyInput,
    ThemeStyleTargetSnapshot,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  const LIVE_STYLE_ID = "pana-theme-style-draft";

  let {
    app,
    catalog,
    loading,
    error,
    query,
    category,
    reload,
    openWorkspaceSource,
  }: {
    app: AppState;
    catalog: ThemeStyleCatalogSnapshot | null;
    loading: boolean;
    error: string;
    query: string;
    category: string;
    reload: () => Promise<void>;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type DetailMode = "info" | "edit";

  let selectedId = $state("");
  let mode = $state<DetailMode>("info");
  let draft = $state<Record<string, string>>({});
  let original = $state<Record<string, string>>({});
  let preview = $state<ThemeStyleDraftPreview | null>(null);
  let previewError = $state("");
  let applying = $state(false);
  let applyError = $state("");
  let editRevision = $state(-1);
  let previewTimer: ReturnType<typeof setTimeout> | null = null;
  let previewSequence = 0;

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase("ro"));
  const visibleTargets = $derived(
    (catalog?.targets ?? []).filter((target) => (
      (category === "all" || target.categoryId === category)
      && (
        !normalizedQuery
        || `${target.label} ${target.description} ${target.selector}`
          .toLocaleLowerCase("ro")
          .includes(normalizedQuery)
      )
    )),
  );
  const selected = $derived(
    visibleTargets.find((target) => target.id === selectedId)
      ?? visibleTargets[0]
      ?? null,
  );
  const dirty = $derived(
    mode === "edit"
    && Object.keys(draft).some((key) => draft[key] !== original[key]),
  );
  const specimenStyle = $derived(
    (preview
      ? preview.properties.map((property) => [property.id, property.value] as const)
      : (selected?.properties ?? []).map(
        (property) => [property.id, property.effectiveValue] as const,
      ))
      .map(([id, value]) => value ? `${id}: ${value}` : "")
      .filter(Boolean)
      .join("; "),
  );

  $effect(() => {
    const next = selected?.id ?? "";
    if (!next || selectedId === next) return;
    if (mode === "edit") cancelEdit();
    selectedId = next;
  });

  $effect(() => {
    const currentRevision = catalog?.workspaceRevision;
    if (
      mode === "edit"
      && editRevision >= 0
      && currentRevision !== undefined
      && currentRevision !== editRevision
    ) cancelEdit();
  });

  const unregisterFlush = registerEditFlushHandler(
    "theme-styles-workspace",
    async (reason: EditFlushReason) => {
      if (mode === "edit" && dirty) await applyDraft(reason);
    },
  );

  onDestroy(() => {
    unregisterFlush();
    clearPreviewTimer();
    app.injectRawCss(LIVE_STYLE_ID, "");
  });

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: app.sessionProjectRoot,
      expectedSessionId: app.kernelProjectSessionId,
    };
  }

  function selectTarget(target: ThemeStyleTargetSnapshot) {
    if (selectedId === target.id) return;
    cancelEdit();
    selectedId = target.id;
  }

  function beginEdit() {
    if (!selected?.editable || applying) return;
    const values = Object.fromEntries(
      selected.properties.map((property) => [property.id, property.value ?? ""]),
    );
    draft = { ...values };
    original = { ...values };
    applyError = "";
    previewError = "";
    editRevision = catalog?.workspaceRevision ?? -1;
    mode = "edit";
    schedulePreview(0);
  }

  function cancelEdit() {
    clearPreviewTimer();
    previewSequence += 1;
    mode = "info";
    draft = {};
    original = {};
    preview = null;
    editRevision = -1;
    previewError = "";
    applyError = "";
    app.injectRawCss(LIVE_STYLE_ID, "");
  }

  function setDraftValue(propertyId: string, value: string) {
    draft = { ...draft, [propertyId]: value };
    applyError = "";
    schedulePreview();
  }

  function clearProperty(propertyId: string) {
    setDraftValue(propertyId, "");
  }

  function inputs(): ThemeStylePropertyInput[] {
    if (!selected) return [];
    return selected.properties.map((property) => ({
      id: property.id,
      value: draft[property.id] ?? "",
    }));
  }

  function schedulePreview(delay = 120) {
    clearPreviewTimer();
    previewTimer = setTimeout(() => {
      previewTimer = null;
      void refreshPreview();
    }, delay);
  }

  function clearPreviewTimer() {
    if (previewTimer !== null) clearTimeout(previewTimer);
    previewTimer = null;
  }

  async function refreshPreview() {
    if (!selected || mode !== "edit") return;
    const requestId = ++previewSequence;
    const sessionId = app.kernelProjectSessionId;
    previewError = "";
    try {
      const next = await previewThemeStyleDraft(
        selected.id,
        inputs(),
        editRevision,
        identity(),
      );
      if (
        requestId !== previewSequence
        || mode !== "edit"
        || app.kernelProjectSessionId !== sessionId
        || selected.id !== next.targetId
      ) return;
      preview = next;
      app.injectRawCss(LIVE_STYLE_ID, next.css);
    } catch (cause) {
      if (requestId !== previewSequence) return;
      preview = null;
      app.injectRawCss(LIVE_STYLE_ID, "");
      previewError = errorMessage(cause);
    }
  }

  async function applyDraft(_reason: EditFlushReason | "button" = "button") {
    if (!selected || !dirty || applying) return;
    clearPreviewTimer();
    applying = true;
    applyError = "";
    const projectRoot = app.sessionProjectRoot;
    const sessionId = app.kernelProjectSessionId;
    try {
      const receipt = await applyThemeStyleDraft(
        selected.id,
        inputs(),
        editRevision,
        identity(),
      );
      if (
        app.sessionProjectRoot !== projectRoot
        || app.kernelProjectSessionId !== sessionId
      ) return;
      await app.projectCommittedInspectorCssMutation(receipt.authority, null);
      if (
        app.sessionProjectRoot !== projectRoot
        || app.kernelProjectSessionId !== sessionId
      ) return;
      app.injectRawCss(LIVE_STYLE_ID, "");
      await reload();
      mode = "info";
      draft = {};
      original = {};
      preview = null;
      editRevision = -1;
      app.setGlobalStatus(
        `Stilul „${receipt.payload.label}” a fost actualizat. Ctrl+S persistă pe disc.`,
        "unsaved",
      );
    } catch (cause) {
      applyError = errorMessage(cause);
      if (_reason !== "button") throw cause;
    } finally {
      applying = false;
    }
  }
</script>

<div class="theme-styles-body">
  <div
    class="style-target-list"
    id="design-panel-global-styles"
    role="tabpanel"
    aria-labelledby="design-tab-global-styles"
  >
    {#if loading && !catalog}
      <div class="workspace-state">Se citește catalogul semantic din ProjectWorkspace…</div>
    {:else if error}
      <div class="workspace-state error" role="alert">
        <IconAlertTriangle size={16} /> {error}
      </div>
    {:else}
      {#each visibleTargets as target (target.id)}
        <button
          type="button"
          class="style-target-row"
          class:selected={selected?.id === target.id}
          class:unavailable={!target.editable}
          onclick={() => selectTarget(target)}
        >
          <span class="target-icon"><IconBrush size={16} stroke={1.8} /></span>
          <span class="target-copy">
            <strong>{target.label}</strong>
            <small>{target.description}</small>
          </span>
          <code>{target.selector}</code>
          {#if target.hasOverrides}<span class="override-badge">Suprascris</span>{/if}
        </button>
      {:else}
        <div class="workspace-state">Nu există stiluri pentru filtrul curent.</div>
      {/each}
    {/if}
  </div>

  <aside class="style-detail" aria-label="Detalii stil semantic">
    {#if selected && mode === "edit"}
      <header class="detail-heading">
        <div>
          <span class="detail-kicker">Editare vizuală</span>
          <h2>{selected.label}</h2>
          <p>Draftul este proiectat live, fără revizii intermediare.</p>
        </div>
        <button type="button" aria-label="Renunță la editare" disabled={applying} onclick={cancelEdit}>
          <IconArrowBackUp size={15} />
        </button>
      </header>

      <div class="specimen" aria-label={`Previzualizare ${selected.label}`}>
        {#if selected.previewKind === "image"}
          <div class="image-specimen" style={specimenStyle}><IconPhoto size={30} stroke={1.5} /></div>
        {:else if selected.previewKind.includes("list")}
          <ul style={specimenStyle}>
            {#each selected.sampleText.split("|") as item}<li>{item}</li>{/each}
          </ul>
        {:else if selected.previewKind.startsWith("table")}
          <table style={specimenStyle}><tbody><tr>{#each selected.sampleText.split("|") as item}<td>{item}</td>{/each}</tr></tbody></table>
        {:else if selected.previewKind.includes("code")}
          <pre style={specimenStyle}>{selected.sampleText}</pre>
        {:else if selected.previewKind === "blockquote" || selected.previewKind === "quote-text"}
          <blockquote style={specimenStyle}>{selected.sampleText}</blockquote>
        {:else if selected.previewKind === "input" || selected.previewKind.includes("input") || selected.previewKind === "placeholder"}
          <input style={specimenStyle} value={selected.previewKind === "placeholder" ? "" : selected.sampleText} placeholder={selected.sampleText} readonly />
        {:else}
          <div class="text-specimen" style={specimenStyle}>{selected.sampleText}</div>
        {/if}
      </div>

      <div class="property-form">
        {#each selected.properties as property (property.id)}
          <label class="property-field">
            <span class="property-label">
              <span>{property.label}</span>
              {#if property.inheritedFrom && !draft[property.id]}
                <small>Moștenit</small>
              {/if}
            </span>
            {#if property.control === "color"}
              <ColorInput
                property={property.id}
                value={draft[property.id] ?? ""}
                suggestions={app.scssVariables}
                oninput={(value) => setDraftValue(property.id, value)}
                oncommit={(value) => setDraftValue(property.id, value)}
              />
            {:else if property.control === "choice"}
              <select
                value={draft[property.id] ?? ""}
                onchange={(event) => setDraftValue(property.id, event.currentTarget.value)}
              >
                {#if property.canClear}
                  <option value="">Moștenit · {property.effectiveValue ?? "valoare implicită"}</option>
                {/if}
                {#each property.options as option (option.value)}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </select>
            {:else}
              <div class="text-control">
                <input
                  value={draft[property.id] ?? ""}
                  placeholder={property.canClear ? `Moștenit · ${property.effectiveValue ?? "—"}` : ""}
                  oninput={(event) => setDraftValue(property.id, event.currentTarget.value)}
                />
                {#if property.canClear && draft[property.id]}
                  <button type="button" onclick={() => clearProperty(property.id)}>Moștenește</button>
                {/if}
              </div>
            {/if}
          </label>
        {/each}
      </div>

      {#if previewError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {previewError}</p>{/if}
      {#if applyError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {applyError}</p>{/if}
      <div class="edit-actions">
        <button type="button" disabled={applying} onclick={cancelEdit}>Renunță</button>
        <button
          class="primary"
          type="button"
          disabled={applying || !dirty || Boolean(previewError)}
          onclick={() => { void applyDraft(); }}
        >
          <IconDeviceFloppy size={14} />
          {applying ? "Se aplică prin Rust…" : "Aplică modificările"}
        </button>
      </div>
    {:else if selected}
      <span class="detail-kicker">{catalog?.categories.find((entry) => entry.id === selected.categoryId)?.label}</span>
      <h2>{selected.label}</h2>
      <p>{selected.description}</p>

      <div class="specimen info-specimen" aria-label={`Exemplu ${selected.label}`}>
        <div class="text-specimen">{selected.sampleText}</div>
      </div>

      <dl class="info-grid">
        {#each selected.properties as property (property.id)}
          <div>
            <dt>{property.label}</dt>
            <dd>
              <code>{property.effectiveValue ?? "—"}</code>
              {#if property.value === null}<small>moștenit</small>{/if}
            </dd>
          </div>
        {/each}
      </dl>

      {#if selected.diagnostic}
        <p class="form-error" role="alert"><IconAlertTriangle size={14} /> {selected.diagnostic}</p>
      {/if}
      <div class="source-card"><span>Sursă semantică</span><code>{selected.sourcePath}</code></div>
      <div class="detail-actions">
        <button class="primary-action" type="button" disabled={!selected.editable} onclick={beginEdit}>
          <IconEdit size={14} /> Editează
        </button>
        <button class="secondary-action" type="button" onclick={() => { void openWorkspaceSource(selected.sourcePath); }}>
          Deschide sursa <IconExternalLink size={13} />
        </button>
      </div>
    {:else}
      <div class="workspace-state">Selectează un stil semantic.</div>
    {/if}
  </aside>
</div>

<style>
  .theme-styles-body { display: grid; grid-template-columns: minmax(360px, 1fr) minmax(320px, .62fr); min-width: 0; min-height: 0; height: 100%; }
  .style-target-list { min-width: 0; min-height: 0; overflow: auto; padding: 8px; border-right: 1px solid var(--wb-border-subtle); }
  .style-target-row { display: grid; grid-template-columns: 32px minmax(0, 1fr) minmax(110px, auto) auto; align-items: center; gap: 9px; width: 100%; min-height: 56px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .style-target-row:hover { background: var(--wb-surface-hover); }
  .style-target-row.selected { border-color: color-mix(in srgb, var(--wb-accent) 34%, var(--wb-border-subtle)); background: var(--wb-accent-soft); box-shadow: inset 3px 0 0 var(--wb-accent); }
  .style-target-row.unavailable { opacity: .62; }
  .target-icon { display: grid; place-items: center; width: 30px; height: 30px; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .target-copy { min-width: 0; }
  .target-copy strong, .target-copy small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .target-copy strong { color: var(--text-strong); font-size: 12px; }
  .target-copy small { margin-top: 3px; color: var(--wb-text-muted); font-size: 11px; }
  .style-target-row > code { overflow: hidden; max-width: 230px; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .override-badge { padding: 2px 5px; border-radius: 4px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 11px; font-weight: 650; }
  .style-detail { min-width: 0; min-height: 0; overflow: auto; padding: 16px; background: var(--wb-surface-document); }
  .detail-heading, .detail-actions, .edit-actions, .property-label, .text-control { display: flex; align-items: center; }
  .detail-heading { justify-content: space-between; gap: 14px; }
  .detail-heading h2, .style-detail > h2 { margin: 5px 0 0; color: var(--text-strong); font-size: 19px; }
  .detail-heading p, .style-detail > p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  .detail-heading > button { display: grid; place-items: center; width: 28px; height: 28px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); }
  .detail-kicker { color: var(--wb-accent-strong); font-size: 11px; font-weight: 700; letter-spacing: .035em; text-transform: uppercase; }
  .specimen { display: grid; place-items: center; min-height: 132px; margin: 15px 0; padding: 18px; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 8px; background: linear-gradient(135deg, var(--wb-surface-chrome), var(--wb-surface-document)); }
  .specimen ul { justify-self: stretch; }
  .specimen table { width: 100%; }
  .specimen td { padding: 8px; border: 1px solid var(--wb-border-subtle); }
  .specimen input { width: min(100%, 360px); }
  .specimen pre { justify-self: stretch; overflow: auto; }
  .text-specimen { max-width: 100%; text-align: center; }
  .image-specimen { display: grid; place-items: center; width: min(100%, 320px); height: 96px; color: var(--wb-text-muted); background: var(--wb-accent-soft); }
  .info-specimen { min-height: 92px; }
  .property-form { display: grid; gap: 11px; }
  .property-field { display: grid; gap: 5px; }
  .property-label { justify-content: space-between; gap: 8px; color: var(--wb-text-primary); font-size: 11px; font-weight: 650; }
  .property-label small { color: var(--wb-text-muted); font-size: 11px; font-weight: 500; }
  .property-field > select, .text-control > input { width: 100%; min-width: 0; height: 30px; padding: 0 8px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .text-control { min-width: 0; }
  .text-control > input { border-radius: 5px 0 0 5px; }
  .text-control > button { flex: 0 0 auto; height: 30px; padding: 0 8px; border: 1px solid var(--wb-border-subtle); border-left: 0; border-radius: 0 5px 5px 0; color: var(--wb-accent-strong); background: var(--wb-surface-chrome); font-size: 11px; }
  .info-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; margin: 14px 0; }
  .info-grid div { min-width: 0; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-chrome); }
  .info-grid dt { color: var(--wb-text-muted); font-size: 11px; font-weight: 650; text-transform: uppercase; }
  .info-grid dd { display: grid; gap: 2px; min-width: 0; margin: 4px 0 0; }
  .info-grid code { overflow: hidden; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .info-grid small { color: var(--wb-text-muted); font-size: 11px; }
  .source-card { display: grid; gap: 5px; margin-top: 12px; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; }
  .source-card span { color: var(--wb-text-muted); font-size: 11px; font-weight: 650; text-transform: uppercase; }
  .source-card code { overflow-wrap: anywhere; font-size: 11px; }
  .detail-actions, .edit-actions { gap: 7px; margin-top: 12px; }
  .edit-actions { justify-content: flex-end; position: sticky; bottom: -16px; padding: 10px 0 0; background: var(--wb-surface-document); }
  .detail-actions button, .edit-actions button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-height: 29px; padding: 0 10px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; font-weight: 650; }
  .detail-actions .primary-action, .edit-actions .primary { border-color: var(--wb-accent); color: #fff; background: var(--wb-accent); }
  .form-error, .workspace-state { display: flex; align-items: center; gap: 6px; color: var(--wb-text-muted); font-size: 12px; }
  .form-error { margin: 10px 0 0; color: var(--danger-strong, #b42318); }
  .workspace-state { justify-content: center; min-height: 120px; padding: 20px; text-align: center; }
  .workspace-state.error { color: var(--danger-strong, #b42318); }

  @media (max-width: 980px) {
    .theme-styles-body { grid-template-columns: minmax(300px, .9fr) minmax(300px, 1fr); }
    .style-target-row { grid-template-columns: 32px minmax(0, 1fr); }
    .style-target-row > code, .override-badge { display: none; }
  }
</style>
