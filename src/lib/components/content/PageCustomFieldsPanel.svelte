<script lang="ts">
  import {
    IconAlertTriangle,
    IconBraces,
    IconCheck,
    IconForms,
  } from "@tabler/icons-svelte";
  import CustomFieldInput from "$lib/components/content/CustomFieldInput.svelte";
  import {
    applyContentModelMutation,
    planContentModelMutation,
    readContentModelCatalog,
  } from "$lib/project/io";
  import {
    flushWorkspaceMutationInputs,
    settleProjectWorkspaceMutation,
  } from "$lib/session/workspace-mutation-coordinator";
  import type { AppState } from "$lib/state/app.svelte";
  import type { ContentModelCatalog } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    app,
    pageFile,
    onSourceChanged = () => {},
  }: {
    app: AppState;
    pageFile: string;
    onSourceChanged?: () => void | Promise<void>;
  } = $props();

  let catalog = $state<ContentModelCatalog | null>(null);
  let values = $state<Record<string, unknown>>({});
  let loadedKey = $state("");
  let loading = $state(false);
  let saving = $state(false);
  let dirty = $state(false);
  let error = $state("");
  let notice = $state("");

  const binding = $derived(
    catalog?.pageBindings.find((candidate) => candidate.pageFile === pageFile) ?? null,
  );
  const model = $derived(
    catalog?.models.find((candidate) => candidate.id === binding?.modelId) ?? null,
  );

  $effect(() => {
    const root = app.sessionProjectRoot.trim();
    const session = app.kernelProjectSessionId.trim();
    const revision = app.projectWorkspaceSnapshot?.revision ?? 0;
    const key = `${root}:${session}:${revision}:${pageFile}`;
    if (!root || !session || !pageFile || loading || saving || dirty || loadedKey === key) return;
    loadedKey = key;
    void load(root, session, revision);
  });

  async function load(
    root = app.sessionProjectRoot,
    session = app.kernelProjectSessionId,
    revision = app.projectWorkspaceSnapshot?.revision ?? 0,
  ) {
    loading = true;
    error = "";
    const requestedPage = pageFile;
    try {
      const next = await readContentModelCatalog({
        expectedProjectRoot: root,
        expectedSessionId: session,
      }, revision);
      if (
        root !== app.sessionProjectRoot
        || session !== app.kernelProjectSessionId
        || requestedPage !== pageFile
      ) return;
      catalog = next;
      const nextBinding = next.pageBindings.find((candidate) => candidate.pageFile === pageFile);
      const nextModel = next.models.find((candidate) => candidate.id === nextBinding?.modelId);
      values = { ...(nextBinding?.values ?? {}) };
      for (const field of nextModel?.fields ?? []) {
        if (!(field.key in values) && field.defaultValue !== undefined) values[field.key] = field.defaultValue;
      }
      dirty = false;
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      loading = false;
    }
  }

  function setValue(key: string, value: unknown) {
    values = { ...values, [key]: value };
    dirty = true;
    notice = "";
  }

  function removeValue(key: string) {
    const next = { ...values };
    delete next[key];
    values = next;
    dirty = true;
    notice = "";
  }

  async function save() {
    if (!model || saving || !dirty) return;
    saving = true;
    error = "";
    notice = "";
    try {
      await flushWorkspaceMutationInputs("manual");
      const identity = {
        expectedProjectRoot: app.sessionProjectRoot,
        expectedSessionId: app.kernelProjectSessionId,
      };
      const input = {
        operation: {
          kind: "set_page_values" as const,
          pageFile,
          values,
        },
      };
      const plan = await planContentModelMutation(input, identity);
      if (plan.blocked) throw new Error(plan.blockers.join(" "));
      const receipt = await applyContentModelMutation(input, plan.planId, identity);
      const settlement = await settleProjectWorkspaceMutation(app, receipt.workspace, {
        preferredRelativePath: pageFile,
        warningLabel: "Câmpuri personalizate",
      });
      dirty = false;
      loadedKey = "";
      await onSourceChanged();
      await load();
      notice = settlement.warnings.length > 0
        ? `Valorile au fost actualizate. ${settlement.warnings.join(" ")}`
        : "Valorile au fost actualizate în frontmatter.";
      app.setGlobalStatus("Câmpurile personalizate au fost actualizate.", "unsaved");
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      saving = false;
    }
  }

</script>

<section class="custom-fields-panel">
  <header>
    <div><span><IconForms size={14} /> Câmpuri personalizate</span><h3>{model?.label ?? "Model neatașat"}</h3></div>
    {#if model}<code>{model.id}</code>{/if}
  </header>

  {#if loading && !catalog}
    <div class="empty">Se citește contractul Rust…</div>
  {:else if error && !model}
    <div class="message error" role="alert"><IconAlertTriangle size={15} /> {error}</div>
  {:else if !binding || !model}
    <div class="empty">
      <IconBraces size={24} />
      <strong>Pagina nu moștenește un model.</strong>
      <span>Atașează un model secțiunii sale din activitatea Modele de conținut.</span>
      <button type="button" onclick={() => { void app.setWorkbenchActivity("content_models"); }}>Deschide modelele</button>
    </div>
  {:else}
    <div class="contract-origin">
      <span>Moștenit din</span><code>{binding.sectionPath}</code>
    </div>
    {#if error}<div class="message error" role="alert"><IconAlertTriangle size={15} /> {error}</div>{/if}
    {#if notice}<div class="message success"><IconCheck size={15} /> {notice}</div>{/if}
    <div class="fields">
      {#each model.fields as field (field.id)}
        <CustomFieldInput field={field} value={values[field.key] ?? field.defaultValue} path={`extra.${field.key}`} onValueChange={(nextValue) => nextValue === undefined ? removeValue(field.key) : setValue(field.key, nextValue)} />
      {:else}
        <div class="empty compact">Modelul nu conține câmpuri.</div>
      {/each}
    </div>
    <footer>
      <span>{dirty ? "Modificări neconfirmate" : "Valorile corespund proiecției Rust"}</span>
      <button class="primary" type="button" disabled={!dirty || saving} onclick={() => { void save(); }}>{saving ? "Se confirmă…" : "Aplică valorile"}</button>
    </footer>
  {/if}
</section>

<style>
  .custom-fields-panel { display: grid; gap: 9px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  header div { display: grid; gap: 2px; } header span { display: flex; align-items: center; gap: 4px; color: var(--wb-accent-strong); font-size: 11px; font-weight: 800; text-transform: uppercase; } h3 { margin: 0; color: var(--text-strong); font-size: 15px; }
  header > code, .contract-origin code { color: var(--wb-accent-strong); font-size: 11px; }
  .contract-origin { display: flex; justify-content: space-between; gap: 8px; padding: 7px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); font-size: 11px; }
  .contract-origin span { color: var(--wb-text-muted); }
  .fields { display: grid; gap: 8px; }
  .message { display: flex; align-items: flex-start; gap: 5px; padding: 7px; border: 1px solid; border-radius: 6px; font-size: 11px; line-height: 1.4; } .message.error { border-color: color-mix(in srgb, var(--danger) 35%, var(--wb-border-subtle)); color: var(--danger); } .message.success { border-color: color-mix(in srgb, var(--success) 35%, var(--wb-border-subtle)); color: var(--success); }
  footer { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding-top: 8px; border-top: 1px solid var(--wb-border-subtle); } footer span { color: var(--wb-text-muted); font-size: 11px; } button { min-height: 30px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); } button.primary { border-color: var(--wb-accent); color: #fff; background: var(--wb-accent); }
  .empty { display: flex; min-height: 180px; align-items: center; justify-content: center; flex-direction: column; gap: 6px; padding: 18px; color: var(--wb-text-muted); text-align: center; font-size: 11px; } .empty strong { color: var(--text-strong); } .empty.compact { min-height: 60px; }
</style>
