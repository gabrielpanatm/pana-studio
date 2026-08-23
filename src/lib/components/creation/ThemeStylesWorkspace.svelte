<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import {
    IconAlertTriangle,
    IconArrowBackUp,
    IconBrowser,
    IconBrush,
    IconCode,
    IconDeviceFloppy,
    IconEdit,
    IconExternalLink,
    IconForms,
    IconLink,
    IconList,
    IconListNumbers,
    IconPhoto,
    IconQuote,
    IconSeparatorHorizontal,
    IconTable,
    IconTypography,
  } from "@tabler/icons-svelte";
  import ColorInput from "$lib/components/inspector/controls/ColorInput.svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import {
    applyThemeStyleDraft,
    previewThemeStyleDraft,
  } from "$lib/css/io";
  import type {
    ThemeStyleCatalogSnapshot,
    ThemeStyleDraftPreview,
    ThemeStylePropertyInput,
    ThemeStyleTargetSnapshot,
  } from "$lib/css/design-system-contract";
  import type { CssMutationAuthorityReceipt } from "$lib/css/mutation-contract";
  import type { ScssVariable } from "$lib/css/contracts";
  import {
    registerEditFlushHandler,
    type EditFlushReason,
  } from "$lib/session/edit-flush-registry";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";
  import { errorMessage } from "$lib/util";

  const LIVE_STYLE_ID = "pana-theme-style-draft";

  let {
    globalStatus,
    workspaceMutations,
    scssVariables,
    injectRawCss,
    projectCommittedCssMutation,
    catalog,
    loading,
    error,
    query,
    category,
    reload,
    openWorkspaceSource,
  }: {
    globalStatus: GlobalStatusState;
    workspaceMutations: ProjectWorkspaceMutationService;
    scssVariables: ScssVariable[];
    injectRawCss: (id: string, css: string) => void;
    projectCommittedCssMutation: (
      authority: CssMutationAuthorityReceipt,
      liveEpoch: number | null,
    ) => Promise<unknown>;
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

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const visibleTargets = $derived(
    (catalog?.targets ?? []).filter((target) => (
      (category === "all" || target.categoryId === category)
      && (
        !normalizedQuery
        || `${target.label} ${target.description} ${target.selector}`
          .toLocaleLowerCase(l10n.locale)
          .includes(normalizedQuery)
      )
    )),
  );
  const visibleCategories = $derived(
    (catalog?.categories ?? [])
      .map((entry) => ({
        ...entry,
        targets: visibleTargets.filter((target) => target.categoryId === entry.id),
      }))
      .filter((entry) => entry.targets.length > 0),
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
    (preview?.targetId === selected?.id
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

  $effect(() => {
    const targetId = selected?.id ?? "";
    const currentRevision = catalog?.workspaceRevision ?? -1;
    const detailMode = mode;
    if (!targetId || detailMode !== "info" || currentRevision < 0) return;
    untrack(() => {
      preview = null;
      schedulePreview(0);
    });
  });

  const unregisterFlush = registerEditFlushHandler(
    "theme-styles-workspace",
    async (reason: EditFlushReason) => {
      if (mode === "edit" && dirty) await applyDraft(reason);
    },
    () => mode === "edit" && dirty,
  );

  onDestroy(() => {
    unregisterFlush();
    clearPreviewTimer();
    injectRawCss(LIVE_STYLE_ID, "");
  });

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: workspaceMutations.snapshot?.projectRoot ?? "",
      expectedSessionId: workspaceMutations.snapshot?.runtimeSessionId ?? "",
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
    injectRawCss(LIVE_STYLE_ID, "");
  }

  function setDraftValue(propertyId: string, value: string) {
    draft = { ...draft, [propertyId]: value };
    applyError = "";
    schedulePreview();
  }

  function clearProperty(propertyId: string) {
    setDraftValue(propertyId, "");
  }

  function inputsFor(
    target: ThemeStyleTargetSnapshot,
    detailMode: DetailMode,
  ): ThemeStylePropertyInput[] {
    return target.properties.map((property) => ({
      id: property.id,
      value: detailMode === "edit"
        ? draft[property.id] ?? ""
        : property.value ?? "",
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
    const target = selected;
    const requestMode = mode;
    const expectedRevision = requestMode === "edit"
      ? editRevision
      : catalog?.workspaceRevision ?? -1;
    if (!target?.editable || expectedRevision < 0) return;
    const requestId = ++previewSequence;
    const sessionId = workspaceMutations.snapshot?.runtimeSessionId ?? "";
    previewError = "";
    try {
      const next = await previewThemeStyleDraft(
        target.id,
        inputsFor(target, requestMode),
        expectedRevision,
        identity(),
      );
      if (
        requestId !== previewSequence
        || mode !== requestMode
        || (workspaceMutations.snapshot?.runtimeSessionId ?? "") !== sessionId
        || selected.id !== next.targetId
      ) return;
      preview = next;
      if (requestMode === "edit") injectRawCss(LIVE_STYLE_ID, next.css);
    } catch (cause) {
      if (requestId !== previewSequence || mode !== requestMode) return;
      preview = null;
      if (requestMode === "edit") injectRawCss(LIVE_STYLE_ID, "");
      previewError = errorMessage(cause);
    }
  }

  async function applyDraft(_reason: EditFlushReason | "button" = "button") {
    if (!selected || !dirty || applying) return;
    clearPreviewTimer();
    applying = true;
    applyError = "";
    const projectRoot = workspaceMutations.snapshot?.projectRoot ?? "";
    const sessionId = workspaceMutations.snapshot?.runtimeSessionId ?? "";
    try {
      const receipt = await applyThemeStyleDraft(
        selected.id,
        inputsFor(selected, "edit"),
        editRevision,
        identity(),
      );
      if (
        (workspaceMutations.snapshot?.projectRoot ?? "") !== projectRoot
        || (workspaceMutations.snapshot?.runtimeSessionId ?? "") !== sessionId
      ) return;
      await projectCommittedCssMutation(receipt.authority, null);
      if (
        (workspaceMutations.snapshot?.projectRoot ?? "") !== projectRoot
        || (workspaceMutations.snapshot?.runtimeSessionId ?? "") !== sessionId
      ) return;
      injectRawCss(LIVE_STYLE_ID, "");
      await reload();
      mode = "info";
      draft = {};
      original = {};
      preview = null;
      editRevision = -1;
      globalStatus.set(
        t("theme-style-updated", { name: receipt.payload.label }),
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

{#snippet renderSpecimen(target: ThemeStyleTargetSnapshot)}
  {#if target.previewKind === "image"}
    <div class="image-specimen" style={specimenStyle}><IconPhoto size={30} stroke={1.5} /></div>
  {:else if target.previewKind.includes("list")}
    <ul style={specimenStyle}>
      {#each target.sampleText.split("|") as item}<li>{item}</li>{/each}
    </ul>
  {:else if target.previewKind.startsWith("table")}
    <table style={specimenStyle}><tbody><tr>{#each target.sampleText.split("|") as item}<td>{item}</td>{/each}</tr></tbody></table>
  {:else if target.previewKind.includes("code")}
    <pre style={specimenStyle}>{target.sampleText}</pre>
  {:else if target.previewKind === "blockquote" || target.previewKind === "quote-text"}
    <blockquote style={specimenStyle}>{target.sampleText}</blockquote>
  {:else if target.previewKind === "input" || target.previewKind.includes("input") || target.previewKind === "placeholder"}
    <input style={specimenStyle} value={target.previewKind === "placeholder" ? "" : target.sampleText} placeholder={target.sampleText} readonly />
  {:else}
    <div class="text-specimen" style={specimenStyle}>{target.sampleText}</div>
  {/if}
{/snippet}

<div class="theme-styles-body">
  <div
    class="style-target-list"
    id="design-panel-global-styles"
    role="tabpanel"
    aria-labelledby="design-tab-global-styles"
  >
    {#if loading && !catalog}
      <div class="workspace-state">{t("theme-style-loading")}</div>
    {:else if error}
      <div class="workspace-state error" role="alert">
        <IconAlertTriangle size={16} /> {error}
      </div>
    {:else}
      {#each visibleCategories as section (section.id)}
        <section class="style-category" aria-label={section.label}>
          <div class="style-category-rows">
            {#each section.targets as target (target.id)}
              <button
                type="button"
                class="style-target-row ui-entity-selectable"
                data-ui-selected={selected?.id === target.id ? "true" : undefined}
                aria-pressed={selected?.id === target.id}
                class:unavailable={!target.editable}
                onclick={() => selectTarget(target)}
              >
                <span class="target-icon">
                  {#if target.previewKind === "image"}
                    <IconPhoto size={16} stroke={1.8} />
                  {:else if target.previewKind.includes("ordered-list")}
                    <IconListNumbers size={16} stroke={1.8} />
                  {:else if target.previewKind.includes("list")}
                    <IconList size={16} stroke={1.8} />
                  {:else if target.previewKind.startsWith("table")}
                    <IconTable size={16} stroke={1.8} />
                  {:else if target.categoryId === "forms" || target.previewKind.includes("input")}
                    <IconForms size={16} stroke={1.8} />
                  {:else if target.previewKind.includes("code")}
                    <IconCode size={16} stroke={1.8} />
                  {:else if target.previewKind.includes("quote") || target.previewKind === "blockquote"}
                    <IconQuote size={16} stroke={1.8} />
                  {:else if target.categoryId === "links"}
                    <IconLink size={16} stroke={1.8} />
                  {:else if target.categoryId === "typography"}
                    <IconTypography size={16} stroke={1.8} />
                  {:else if target.categoryId === "general"}
                    <IconBrowser size={16} stroke={1.8} />
                  {:else if target.categoryId === "auxiliary"}
                    <IconSeparatorHorizontal size={16} stroke={1.8} />
                  {:else}
                    <IconBrush size={16} stroke={1.8} />
                  {/if}
                </span>
                <span class="target-copy">
                  <strong>{target.label}</strong>
                  <small>{target.description}</small>
                </span>
                <code>{target.selector}</code>
                {#if target.hasOverrides}<span class="override-badge">{t("theme-style-overridden")}</span>{/if}
              </button>
            {/each}
          </div>
        </section>
      {:else}
        <div class="workspace-state">{t("theme-style-empty")}</div>
      {/each}
    {/if}
  </div>

  <aside class="style-detail" aria-label={t("theme-style-detail-label")}>
    {#if selected && mode === "edit"}
      <header class="detail-heading">
        <div>
          <span class="detail-kicker">{t("theme-style-visual-edit")}</span>
          <h2>{selected.label}</h2>
          <p>{t("theme-style-edit-description")}</p>
        </div>
        <button type="button" aria-label={t("theme-style-cancel-edit")} disabled={applying} onclick={cancelEdit}>
          <IconArrowBackUp size={15} />
        </button>
      </header>

      <div class="specimen" aria-label={t("theme-style-preview", { name: selected.label })}>
        {@render renderSpecimen(selected)}
      </div>

      <div class="property-form">
        {#each selected.properties as property (property.id)}
          <label class="property-field">
            <span class="property-label">
              <span>{property.label}</span>
              {#if property.inheritedFrom && !draft[property.id]}
                <small>{t("theme-style-inherited")}</small>
              {/if}
            </span>
            {#if property.control === "color"}
              <ColorInput
                property={property.id}
                value={draft[property.id] ?? ""}
                suggestions={scssVariables}
                oninput={(value) => setDraftValue(property.id, value)}
                oncommit={(value) => setDraftValue(property.id, value)}
              />
            {:else if property.control === "choice"}
              <select
                value={draft[property.id] ?? ""}
                onchange={(event) => setDraftValue(property.id, event.currentTarget.value)}
              >
                {#if property.canClear}
                  <option value="">{t("theme-style-inherited")} · {property.effectiveValue ?? t("theme-style-default-value")}</option>
                {/if}
                {#each property.options as option (option.value)}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </select>
            {:else}
              <div class="text-control">
                <input
                  value={draft[property.id] ?? ""}
                  placeholder={property.canClear ? `${t("theme-style-inherited")} · ${property.effectiveValue ?? "—"}` : ""}
                  oninput={(event) => setDraftValue(property.id, event.currentTarget.value)}
                />
                {#if property.canClear && draft[property.id]}
                  <button type="button" onclick={() => clearProperty(property.id)}>{t("theme-style-inherit")}</button>
                {/if}
              </div>
            {/if}
          </label>
        {/each}
      </div>

      {#if previewError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {previewError}</p>{/if}
      {#if applyError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {applyError}</p>{/if}
      <div class="edit-actions">
        <button type="button" disabled={applying} onclick={cancelEdit}>{t("theme-style-cancel")}</button>
        <button
          class="ui-button primary"
          type="button"
          disabled={applying || !dirty || Boolean(previewError)}
          onclick={() => { void applyDraft(); }}
        >
          <IconDeviceFloppy size={14} />
          {applying ? t("theme-style-applying") : t("theme-style-apply")}
        </button>
      </div>
    {:else if selected}
      <span class="detail-kicker">{catalog?.categories.find((entry) => entry.id === selected.categoryId)?.label}</span>
      <h2>{selected.label}</h2>
      <p>{selected.description}</p>

      <div class="specimen info-specimen" aria-label={t("theme-style-example", { name: selected.label })}>
        {@render renderSpecimen(selected)}
      </div>

      <dl class="info-grid">
        {#each selected.properties as property (property.id)}
          <div>
            <dt>{property.label}</dt>
            <dd>
              <code>{property.effectiveValue ?? "—"}</code>
              {#if property.value === null}<small>{t("theme-style-inherited").toLocaleLowerCase(l10n.locale)}</small>{/if}
            </dd>
          </div>
        {/each}
      </dl>

      {#if selected.diagnostic}
        <p class="form-error" role="alert"><IconAlertTriangle size={14} /> {t("theme-style-invalid-source")}</p>
      {/if}
      <div class="source-card"><span>{t("theme-style-semantic-source")}</span><code>{selected.sourcePath}</code></div>
      <div class="detail-actions">
        <button class="ui-button primary primary-action" type="button" disabled={!selected.editable} onclick={beginEdit}>
          <IconEdit size={14} /> {t("theme-style-edit")}
        </button>
        <button class="ui-button secondary-action" type="button" onclick={() => { void openWorkspaceSource(selected.sourcePath); }}>
          {t("theme-style-open-source")} <IconExternalLink size={13} />
        </button>
      </div>
    {:else}
      <div class="workspace-state">{t("theme-style-select")}</div>
    {/if}
  </aside>
</div>

<style>
  .theme-styles-body { display: grid; grid-template-columns: minmax(360px, 1fr) minmax(320px, .62fr); min-width: 0; min-height: 0; height: 100%; }
  .style-target-list { min-width: 0; min-height: 0; overflow: auto; padding: 8px; border-right: 1px solid var(--wb-border-subtle); }
  .style-category { min-width: 0; padding: 4px 4px 12px; border-bottom: 1px solid var(--wb-border-subtle); }
  .style-category + .style-category { padding-top: 12px; }
  .style-category:last-child { border-bottom: 0; }
  .style-category-rows { display: grid; gap: 2px; }
  .style-target-row { display: grid; grid-template-columns: 32px minmax(0, 1fr) minmax(110px, auto) auto; align-items: center; gap: 9px; width: 100%; min-height: 56px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
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
