<script lang="ts">
  import {
    IconAlertTriangle,
    IconCheck,
    IconExternalLink,
    IconRefresh,
    IconTags,
  } from "@tabler/icons-svelte";
  import {
    applyTaxonomyMutation,
    planTaxonomyMutation,
    readTaxonomyCatalog,
  } from "$lib/project/io";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import { settleProjectWorkspaceMutation } from "$lib/session/workspace-mutation-coordinator";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    FileBufferRequestIdentity,
    SourceGraphPage,
    TaxonomyCatalogSnapshot,
    TaxonomyMutationInput,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    app,
    page,
  }: {
    app: AppState;
    page: SourceGraphPage;
  } = $props();

  let catalog = $state<TaxonomyCatalogSnapshot | null>(null);
  let drafts = $state<Record<string, string>>({});
  let loading = $state(false);
  let busyId = $state("");
  let loadedKey = $state("");
  let hydratedKey = $state("");
  let loadError = $state("");
  let mutationError = $state("");

  const pageLanguage = $derived(resolvePageLanguage(page.file, catalog));
  const definitions = $derived(
    (catalog?.entries ?? []).filter(
      (entry) => entry.declared && entry.language === pageLanguage,
    ),
  );

  $effect(() => {
    const root = app.sessionProjectRoot.trim();
    const sessionId = app.kernelProjectSessionId.trim();
    const revision = app.projectWorkspaceSnapshot?.revision ?? 0;
    const key = `${root}:${sessionId}:${revision}`;
    if (!root || !sessionId || loading || busyId || loadedKey === key) return;
    loadedKey = key;
    void loadCatalog(root, sessionId, revision);
  });

  $effect(() => {
    const revision = app.projectWorkspaceSnapshot?.revision ?? 0;
    const key = `${page.id}:${revision}:${catalog?.schemaVersion ?? 0}`;
    if (!catalog || hydratedKey === key) return;
    const next: Record<string, string> = {};
    for (const entry of definitions) {
      next[entry.id] = (page.taxonomies[entry.name] ?? []).join(", ");
    }
    drafts = next;
    hydratedKey = key;
  });

  async function loadCatalog(
    root = app.sessionProjectRoot,
    sessionId = app.kernelProjectSessionId,
    expectedWorkspaceRevision = app.projectWorkspaceSnapshot?.revision ?? 0,
  ) {
    loading = true;
    loadError = "";
    try {
      const snapshot = await readTaxonomyCatalog({
        expectedProjectRoot: root,
        expectedSessionId: sessionId,
      }, expectedWorkspaceRevision);
      if (
        root !== app.sessionProjectRoot
        || sessionId !== app.kernelProjectSessionId
        || app.projectWorkspaceSnapshot?.revision !== expectedWorkspaceRevision
      ) return;
      catalog = snapshot;
      hydratedKey = "";
    } catch (error) {
      if (root === app.sessionProjectRoot && sessionId === app.kernelProjectSessionId) {
        loadError = errorMessage(error);
      }
    } finally {
      if (root === app.sessionProjectRoot && sessionId === app.kernelProjectSessionId) {
        loading = false;
      }
    }
  }

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: app.sessionProjectRoot,
      expectedSessionId: app.kernelProjectSessionId,
    };
  }

  function parsedTerms(value: string): string[] {
    const seen = new Set<string>();
    return value
      .split(",")
      .map((term) => term.trim())
      .filter((term) => {
        const key = term.toLocaleLowerCase(l10n.locale);
        if (!term || seen.has(key)) return false;
        seen.add(key);
        return true;
      });
  }

  async function saveAssignment(entryId: string, taxonomyName: string) {
    if (busyId) return;
    busyId = entryId;
    mutationError = "";
    const input: TaxonomyMutationInput = {
      operation: {
        kind: "set_page_terms",
        pageFile: page.file,
        taxonomyName,
        terms: parsedTerms(drafts[entryId] ?? ""),
      },
    };
    try {
      const commandIdentity = identity();
      const plan = await planTaxonomyMutation(input, commandIdentity);
      if (
        commandIdentity.expectedProjectRoot !== app.sessionProjectRoot
        || commandIdentity.expectedSessionId !== app.kernelProjectSessionId
      ) return;
      const receipt = await applyTaxonomyMutation(input, plan.planId, commandIdentity);
      const settlement = await settleProjectWorkspaceMutation(app, receipt.workspace, {
        preferredRelativePath: page.file,
        warningLabel: t("content-taxonomy-operation-label"),
      });
      loadedKey = "";
      hydratedKey = "";
      app.setGlobalStatus(
        settlement.warnings.length > 0
          ? t("content-taxonomy-assignment-warning", { taxonomy: taxonomyName })
          : t("content-taxonomy-assignment-success", { taxonomy: taxonomyName }),
        "unsaved",
      );
    } catch (error) {
      mutationError = errorMessage(error);
      app.setGlobalStatus(t("content-taxonomy-assignment-failed", { message: mutationError }), "error");
    } finally {
      busyId = "";
    }
  }

  function resolvePageLanguage(
    file: string,
    snapshot: TaxonomyCatalogSnapshot | null,
  ): string {
    if (!snapshot) return "";
    const languages = new Set(snapshot.entries.map((entry) => entry.language));
    languages.add(snapshot.defaultLanguage);
    const normalized = file.replaceAll("\\", "/");
    for (const language of languages) {
      if (language !== snapshot.defaultLanguage && normalized.endsWith(`.${language}.md`)) {
        return language;
      }
    }
    return snapshot.defaultLanguage;
  }
</script>

<section class="taxonomy-assignments" aria-labelledby={`page-taxonomies-${page.id}`}>
  <header>
    <span class="section-icon"><IconTags size={15} /></span>
    <div>
      <h3 id={`page-taxonomies-${page.id}`}>{t("content-taxonomies-title")}</h3>
      <p>{t("content-taxonomies-description", { language: pageLanguage || "…" })}</p>
    </div>
    <button type="button" onclick={() => { void app.setWorkbenchActivity("taxonomies"); }}>
      {t("content-manage")} <IconExternalLink size={13} />
    </button>
  </header>

  {#if loadError}
    <div class="state error" role="alert">
      <IconAlertTriangle size={15} />
      <span>{loadError}</span>
      <button
        type="button"
        onclick={() => {
          loadedKey = `${app.sessionProjectRoot.trim()}:${app.kernelProjectSessionId.trim()}:${app.projectWorkspaceSnapshot?.revision ?? 0}`;
          void loadCatalog();
        }}
      >
        <IconRefresh size={13} /> {t("content-retry")}
      </button>
    </div>
  {:else if loading && !catalog}
    <div class="state"><span class="spin"><IconRefresh size={15} /></span> {t("content-loading-taxonomies")}</div>
  {:else if definitions.length === 0}
    <div class="state">{t("content-no-language-taxonomies")}</div>
  {:else}
    <div class="assignment-list">
      {#each definitions as entry (entry.id)}
        <div class="assignment-row">
          <label for={`taxonomy-${page.id}-${entry.id}`}>
            <span>{entry.name}</span>
            <small>{t("content-known-terms", { count: entry.terms.length })}</small>
          </label>
          <input
            id={`taxonomy-${page.id}-${entry.id}`}
            value={drafts[entry.id] ?? ""}
            list={`taxonomy-options-${page.id}-${entry.id}`}
            placeholder={t("content-terms-placeholder")}
            disabled={Boolean(busyId)}
            oninput={(event) => { drafts[entry.id] = event.currentTarget.value; }}
          />
          <datalist id={`taxonomy-options-${page.id}-${entry.id}`}>
            {#each entry.terms as term (term.id)}<option value={term.name}></option>{/each}
          </datalist>
          <button
            type="button"
            disabled={Boolean(busyId)}
            onclick={() => { void saveAssignment(entry.id, entry.name); }}
          >
            <IconCheck size={13} />
            {busyId === entry.id ? t("content-applying") : t("content-apply")}
          </button>
        </div>
      {/each}
    </div>
  {/if}

  {#if mutationError}
    <p class="mutation-error" role="alert"><IconAlertTriangle size={14} /> {mutationError}</p>
  {/if}
</section>

<style>
  .taxonomy-assignments { display: grid; gap: 9px; padding: 10px; border: 1px solid var(--wb-border-subtle); border-radius: 8px; background: var(--wb-surface-document); }
  header { display: flex; align-items: center; gap: 8px; }
  .section-icon { display: grid; flex: 0 0 auto; width: 28px; height: 28px; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  header > div { flex: 1; min-width: 0; }
  h3 { margin: 0; color: var(--text-strong); font-size: 12px; }
  header p { margin: 2px 0 0; color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  header button, .assignment-row button, .state button { display: inline-flex; align-items: center; justify-content: center; gap: 4px; min-height: 27px; padding: 0 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 11px; }
  .assignment-list { display: grid; gap: 6px; }
  .assignment-row { display: grid; grid-template-columns: minmax(90px, .42fr) minmax(150px, 1fr) auto; gap: 7px; align-items: center; padding-top: 6px; border-top: 1px solid var(--wb-border-subtle); }
  .assignment-row label { display: grid; gap: 2px; min-width: 0; }
  .assignment-row label span { color: var(--text-strong); font-size: 11px; font-weight: 650; }
  .assignment-row label small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .assignment-row input { width: 100%; min-width: 0; height: 29px; padding: 0 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; }
  .state { display: flex; align-items: center; gap: 6px; min-height: 36px; padding: 7px; border: 1px dashed var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-muted); font-size: 11px; }
  .state span:not(.spin) { flex: 1; }
  .state.error, .mutation-error { color: var(--danger); }
  .mutation-error { display: flex; align-items: flex-start; gap: 5px; margin: 0; font-size: 11px; }
  .spin { animation: spin 1s linear infinite; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { opacity: .45; }
  button:focus-visible, input:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
