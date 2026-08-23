<script lang="ts">
  import {
    IconAlertTriangle,
    IconArrowDown,
    IconArrowUp,
    IconBraces,
    IconCheck,
    IconForms,
    IconLink,
    IconPlus,
    IconRefresh,
    IconSearch,
    IconTrash,
    IconX,
  } from "@tabler/icons-svelte";
  import {
  applyContentModelMutation,
  planContentModelMutation,
  readContentModelCatalog,
} from "$lib/content-models/io";
  import {
    flushWorkspaceMutationInputs,
  } from "$lib/session/workspace-mutation-coordinator";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type {
    ContentFieldDefinition,
    ContentFieldKind,
    ContentModelCatalog,
    ContentModelDefinition,
    ContentModelMutationInput,
  } from "$lib/content-models/contracts";
  import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";
  import type { SourceGraph } from "$lib/source-graph/graph-contract";
  import { errorMessage } from "$lib/util";

  let {
    globalStatus,
    workspaceMutations,
    sourceGraph,
    openWorkspaceSource,
  }: {
    globalStatus: GlobalStatusState;
    workspaceMutations: ProjectWorkspaceMutationService;
    sourceGraph: SourceGraph | null;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type DetailMode = "info" | "create" | "model" | "field";
  type ModelView = "fields" | "sections" | "usages" | "validation";

  const MODEL_VIEWS: Array<{ id: ModelView; label: string }> = [
    { id: "fields", label: "Câmpuri" },
    { id: "sections", label: "Secțiuni" },
    { id: "usages", label: "Utilizări" },
    { id: "validation", label: "Validare" },
  ];

  const FIELD_KINDS: Array<{ value: ContentFieldKind; label: string }> = [
    { value: "text", label: "Text" },
    { value: "textarea", label: "Text multilinie" },
    { value: "markdown", label: "Markdown" },
    { value: "number", label: "Număr" },
    { value: "boolean", label: "Da / Nu" },
    { value: "date", label: "Dată" },
    { value: "select", label: "Selecție" },
    { value: "url", label: "URL" },
    { value: "color", label: "Culoare" },
    { value: "image", label: "Imagine" },
    { value: "group", label: "Grup" },
    { value: "repeater", label: "Repetor" },
  ];

  const FIELD_EXAMPLES: Record<ContentFieldKind, {
    key: string;
    label: string;
    help: string;
    defaultValue: string;
    pattern: string;
  }> = {
    text: {
      key: "subtitlu",
      label: "Subtitlu",
      help: "Ex.: Textul scurt afișat sub titlul serviciului.",
      defaultValue: "Ex.: Consultanță adaptată proiectului tău",
      pattern: "Ex.: ^.{3,80}$",
    },
    textarea: {
      key: "rezumat",
      label: "Rezumat",
      help: "Ex.: Descrierea scurtă folosită în cardurile arhivei.",
      defaultValue: "Ex.: O prezentare scurtă a serviciului.",
      pattern: "Ex.: ^[\\s\\S]{20,}$",
    },
    markdown: {
      key: "continut_suplimentar",
      label: "Conținut suplimentar",
      help: "Ex.: Conținut formatat care poate include liste și legături.",
      defaultValue: "Ex.: **Beneficiu principal**",
      pattern: "Ex.: ^[\\s\\S]{20,}$",
    },
    number: {
      key: "pret",
      label: "Preț",
      help: "Ex.: Valoarea numerică afișată în oferta serviciului.",
      defaultValue: "Ex.: 120",
      pattern: "",
    },
    boolean: {
      key: "promovat",
      label: "Serviciu promovat",
      help: "Ex.: Activează afișarea serviciului în zona recomandată.",
      defaultValue: "",
      pattern: "",
    },
    date: {
      key: "data_publicarii",
      label: "Data publicării",
      help: "Ex.: Data folosită pentru afișare sau ordonare.",
      defaultValue: "Ex.: 2026-08-03",
      pattern: "",
    },
    select: {
      key: "categorie",
      label: "Categorie",
      help: "Ex.: Alege categoria serviciului din opțiunile definite.",
      defaultValue: "Ex.: standard",
      pattern: "",
    },
    url: {
      key: "link_actiune",
      label: "Link acțiune",
      help: "Ex.: Adresa deschisă de butonul principal.",
      defaultValue: "Ex.: https://exemplu.ro/contact/",
      pattern: "Ex.: ^https://",
    },
    color: {
      key: "culoare_accent",
      label: "Culoare accent",
      help: "Ex.: Culoarea asociată vizual acestui serviciu.",
      defaultValue: "Ex.: #ff7a00",
      pattern: "",
    },
    image: {
      key: "imagine",
      label: "Imagine",
      help: "Ex.: Imaginea principală afișată în card și pe pagina serviciului.",
      defaultValue: "Ex.: /imagini/serviciu.webp",
      pattern: "",
    },
    group: {
      key: "detalii",
      label: "Detalii",
      help: "Ex.: Grupează câmpurile tehnice ale serviciului.",
      defaultValue: "Ex.: {}",
      pattern: "",
    },
    repeater: {
      key: "beneficii",
      label: "Beneficii",
      help: "Ex.: Listă repetabilă de beneficii, fiecare cu propriile subcâmpuri.",
      defaultValue: "Ex.: []",
      pattern: "",
    },
  };

  let catalog = $state<ContentModelCatalog | null>(null);
  let selectedModelId = $state("");
  let selectedFieldId = $state("");
  let mode = $state<DetailMode>("info");
  let activeView = $state<ModelView>("fields");
  let query = $state("");
  let loading = $state(false);
  let busy = $state(false);
  let loadedKey = $state("");
  let error = $state("");
  let notice = $state("");

  let modelIdDraft = $state("");
  let modelLabelDraft = $state("");
  let modelDescriptionDraft = $state("");
  let fieldIdDraft = $state("");
  let fieldKeyDraft = $state("");
  let fieldLabelDraft = $state("");
  let fieldKindDraft = $state<ContentFieldKind>("text");
  let fieldRequiredDraft = $state(false);
  let fieldHelpDraft = $state("");
  let fieldChoicesDraft = $state("");
  let fieldMinimumDraft = $state("");
  let fieldMaximumDraft = $state("");
  let fieldPatternDraft = $state("");
  let fieldDefaultDraft = $state("");
  let fieldParentIdDraft = $state<string | null>(null);
  let sectionDraft = $state("");

  type FieldEntry = {
    field: ContentFieldDefinition;
    parentFieldId: string | null;
    path: string;
    depth: number;
    index: number;
    siblingCount: number;
  };

  function flattenSchemaFields(
    fields: ContentFieldDefinition[],
    parentFieldId: string | null = null,
    parentPath = "",
    depth = 0,
  ): FieldEntry[] {
    return fields.flatMap((field, index) => {
      const path = parentPath ? `${parentPath}.${field.key}` : field.key;
      return [
        { field, parentFieldId, path, depth, index, siblingCount: fields.length },
        ...flattenSchemaFields(field.fields, field.id, path, depth + 1),
      ];
    });
  }

  function countFields(fields: ContentFieldDefinition[]): number {
    return fields.reduce((count, field) => count + 1 + countFields(field.fields), 0);
  }

  const selectedModel = $derived(
    catalog?.models.find((model) => model.id === selectedModelId)
      ?? catalog?.models[0]
      ?? null,
  );
  const schemaFields = $derived(flattenSchemaFields(selectedModel?.fields ?? []));
  const selectedFieldEntry = $derived(
    schemaFields.find((entry) => entry.field.id === selectedFieldId) ?? null,
  );
  const selectedField = $derived(
    selectedFieldEntry?.field ?? null,
  );
  const fieldContainers = $derived(
    schemaFields.filter((entry) => entry.field.kind === "group" || entry.field.kind === "repeater"),
  );
  const visibleModels = $derived(
    (catalog?.models ?? []).filter((model) => {
      const normalized = query.trim().toLocaleLowerCase();
      return !normalized
        || `${model.label} ${model.id} ${model.description}`.toLocaleLowerCase().includes(normalized);
    }),
  );
  const sections = $derived(
    (sourceGraph?.pages ?? [])
      .filter((page) => page.pageKind === "section" || page.pageKind === "home")
      .sort((left, right) => left.file.localeCompare(right.file)),
  );
  const selectedAssignments = $derived(
    (catalog?.assignments ?? []).filter((assignment) => assignment.modelId === selectedModel?.id),
  );
  const sectionDraftAssignment = $derived(
    catalog?.assignments.find((assignment) => assignment.sectionPath === sectionDraft) ?? null,
  );
  const selectedUsages = $derived(
    (catalog?.templateUsages ?? []).filter((usage) => usage.modelId === selectedModel?.id),
  );
  const viewCounts = $derived<Record<ModelView, number>>({
    fields: schemaFields.length,
    sections: selectedAssignments.length,
    usages: selectedUsages.length,
    validation: catalog?.diagnostics.length ?? 0,
  });

  $effect(() => {
    const root = workspaceMutations.snapshot?.projectRoot.trim() ?? "";
    const session = workspaceMutations.snapshot?.runtimeSessionId.trim() ?? "";
    const revision = workspaceMutations.snapshot?.revision ?? 0;
    const key = `${root}:${session}:${revision}`;
    if (!root || !session || loading || busy || loadedKey === key) return;
    loadedKey = key;
    void loadCatalog(root, session, revision);
  });

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: workspaceMutations.snapshot?.projectRoot ?? "",
      expectedSessionId: workspaceMutations.snapshot?.runtimeSessionId ?? "",
    };
  }

  async function loadCatalog(
    root = workspaceMutations.snapshot?.projectRoot ?? "",
    session = workspaceMutations.snapshot?.runtimeSessionId ?? "",
    revision = workspaceMutations.snapshot?.revision ?? 0,
  ) {
    loading = true;
    error = "";
    try {
      const next = await readContentModelCatalog({
        expectedProjectRoot: root,
        expectedSessionId: session,
      }, revision);
      if (
        root !== (workspaceMutations.snapshot?.projectRoot ?? "")
        || session !== (workspaceMutations.snapshot?.runtimeSessionId ?? "")
        || revision !== workspaceMutations.snapshot?.revision
      ) return;
      catalog = next;
      if (!next.models.some((model) => model.id === selectedModelId)) {
        selectedModelId = next.models[0]?.id ?? "";
        selectedFieldId = "";
      }
    } catch (cause) {
      if (
        root === (workspaceMutations.snapshot?.projectRoot ?? "")
        && session === (workspaceMutations.snapshot?.runtimeSessionId ?? "")
      ) {
        error = errorMessage(cause);
      }
    } finally {
      if (
        root === (workspaceMutations.snapshot?.projectRoot ?? "")
        && session === (workspaceMutations.snapshot?.runtimeSessionId ?? "")
      ) loading = false;
    }
  }

  async function executeMutation(
    input: ContentModelMutationInput,
    success: string,
    preferredPath: string | null = null,
  ) {
    if (busy) return false;
    busy = true;
    error = "";
    notice = "";
    try {
      const commandIdentity = identity();
      await flushWorkspaceMutationInputs("manual");
      const plan = await planContentModelMutation(input, commandIdentity);
      if (plan.blocked) {
        error = plan.blockers.join(" ");
        return false;
      }
      if (plan.destructive) {
        const details = [
          plan.affectedPages.length > 0
            ? `${plan.affectedPages.length} pagini vor fi curățate.`
            : "Nu sunt pagini cu valori de curățat.",
          plan.affectedKeys.length > 0
            ? `Chei administrate: ${plan.affectedKeys.join(", ")}.`
            : "Nu sunt chei de conținut afectate.",
          ...plan.warnings,
        ].join("\n");
        if (!window.confirm(`${plan.label}\n\n${details}\n\nContinui?`)) return false;
      }
      const receipt = await applyContentModelMutation(input, plan.planId, commandIdentity);
      const settlement = await workspaceMutations.settle(receipt.workspace, {
        preferredRelativePath: preferredPath ?? receipt.plan.touchedFiles[0] ?? null,
        warningLabel: "Modele de conținut",
      });
      loadedKey = "";
      await loadCatalog();
      const warnings = [...plan.warnings, ...settlement.warnings];
      notice = warnings.length > 0 ? `${success} ${warnings.join(" ")}` : success;
      globalStatus.set(success, "unsaved");
      return true;
    } catch (cause) {
      error = errorMessage(cause);
      globalStatus.set(`Modelele de conținut nu au putut fi modificate: ${error}`, "error");
      return false;
    } finally {
      busy = false;
    }
  }

  function selectModel(model: ContentModelDefinition) {
    selectedModelId = model.id;
    selectedFieldId = "";
    activeView = "fields";
    mode = "info";
    error = "";
    notice = "";
  }

  function beginCreateModel() {
    modelIdDraft = "";
    modelLabelDraft = "";
    modelDescriptionDraft = "";
    mode = "create";
    error = "";
  }

  function beginEditModel() {
    if (!selectedModel) return;
    modelIdDraft = selectedModel.id;
    modelLabelDraft = selectedModel.label;
    modelDescriptionDraft = selectedModel.description;
    mode = "model";
    error = "";
  }

  function beginField(entry: FieldEntry | null = null, parentFieldId: string | null = null) {
    const field = entry?.field ?? null;
    selectedFieldId = field?.id ?? "";
    fieldIdDraft = field?.id ?? "";
    fieldKeyDraft = field?.key ?? "";
    fieldLabelDraft = field?.label ?? "";
    fieldKindDraft = field?.kind ?? "text";
    fieldRequiredDraft = field?.required ?? false;
    fieldHelpDraft = field?.help ?? "";
    fieldChoicesDraft = (field?.choices ?? []).map((choice) => `${choice.value}|${choice.label}`).join("\n");
    fieldMinimumDraft = field?.minimum === undefined ? "" : String(field.minimum);
    fieldMaximumDraft = field?.maximum === undefined ? "" : String(field.maximum);
    fieldPatternDraft = field?.pattern ?? "";
    fieldDefaultDraft = field?.defaultValue === undefined
      ? ""
      : typeof field.defaultValue === "string"
        ? field.defaultValue
        : JSON.stringify(field.defaultValue, null, 2);
    fieldParentIdDraft = entry?.parentFieldId ?? parentFieldId;
    activeView = "fields";
    mode = "field";
    error = "";
  }

  function selectView(view: ModelView) {
    activeView = view;
    selectedFieldId = "";
    mode = "info";
  }

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + MODEL_VIEWS.length) % MODEL_VIEWS.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % MODEL_VIEWS.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = MODEL_VIEWS.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const next = MODEL_VIEWS[nextIndex];
    if (!next) return;
    selectView(next.id);
    requestAnimationFrame(() => document.getElementById(`content-models-tab-${next.id}`)?.focus());
  }

  function fieldDraft(): ContentFieldDefinition {
    const minimum = Number.parseFloat(fieldMinimumDraft);
    const maximum = Number.parseFloat(fieldMaximumDraft);
    const defaultValue = parseDefaultValue();
    return {
      id: fieldIdDraft,
      key: fieldKeyDraft.trim(),
      label: fieldLabelDraft.trim(),
      kind: fieldKindDraft,
      required: fieldRequiredDraft,
      help: fieldHelpDraft.trim(),
      choices: fieldKindDraft === "select"
        ? fieldChoicesDraft.split("\n").map((line) => line.trim()).filter(Boolean).map((line) => {
            const [value, ...label] = line.split("|");
            return { value: value?.trim() ?? "", label: label.join("|").trim() || value?.trim() || "" };
          })
        : [],
      ...(Number.isFinite(minimum) ? { minimum } : {}),
      ...(Number.isFinite(maximum) ? { maximum } : {}),
      ...(fieldPatternDraft.trim() ? { pattern: fieldPatternDraft.trim() } : {}),
      ...(defaultValue === undefined ? {} : { defaultValue }),
      fields: selectedField?.fields ?? [],
    };
  }

  function parseDefaultValue(): unknown {
    if (!fieldDefaultDraft.trim()) return undefined;
    if (fieldKindDraft === "number") {
      const value = Number(fieldDefaultDraft);
      if (!Number.isFinite(value)) throw new Error("Valoarea implicită trebuie să fie un număr valid.");
      return value;
    }
    if (fieldKindDraft === "boolean") {
      if (fieldDefaultDraft === "true") return true;
      if (fieldDefaultDraft === "false") return false;
      throw new Error("Valoarea implicită booleană trebuie să fie true sau false.");
    }
    if (fieldKindDraft === "group" || fieldKindDraft === "repeater") {
      try {
        return JSON.parse(fieldDefaultDraft) as unknown;
      } catch {
        throw new Error("Valoarea implicită structurată trebuie să fie JSON valid.");
      }
    }
    return fieldDefaultDraft;
  }

  async function saveModel(event: SubmitEvent) {
    event.preventDefault();
    const create = mode === "create";
    const rename = !create && Boolean(selectedModel) && modelIdDraft.trim() !== selectedModel?.id;
    const input: ContentModelMutationInput = {
      operation: create
        ? {
            kind: "create_model",
            id: modelIdDraft.trim(),
            label: modelLabelDraft.trim(),
            description: modelDescriptionDraft.trim(),
          }
        : rename
          ? {
              kind: "rename_model",
              modelId: selectedModel?.id ?? "",
              newId: modelIdDraft.trim(),
              label: modelLabelDraft.trim(),
              description: modelDescriptionDraft.trim(),
            }
          : {
            kind: "update_model",
            modelId: selectedModel?.id ?? "",
            label: modelLabelDraft.trim(),
            description: modelDescriptionDraft.trim(),
          },
    };
    if (await executeMutation(input, create ? "Modelul a fost creat." : "Modelul a fost actualizat.")) {
      if (create || rename) selectedModelId = modelIdDraft.trim();
      mode = "info";
    }
  }

  async function saveField(event: SubmitEvent) {
    event.preventDefault();
    if (!selectedModel) return;
    let draft: ContentFieldDefinition;
    try {
      draft = fieldDraft();
    } catch (cause) {
      error = errorMessage(cause);
      return;
    }
    const input: ContentModelMutationInput = {
      operation: {
        kind: "upsert_field",
        modelId: selectedModel.id,
        parentFieldId: fieldParentIdDraft,
        originalFieldId: selectedField?.id ?? null,
        field: draft,
      },
    };
    if (await executeMutation(input, selectedField ? "Câmpul a fost actualizat." : "Câmpul a fost adăugat.")) {
      mode = "info";
    }
  }

  async function removeField(entry: FieldEntry) {
    if (!selectedModel) return;
    await executeMutation({
      operation: {
        kind: "remove_field",
        modelId: selectedModel.id,
        parentFieldId: entry.parentFieldId,
        fieldId: entry.field.id,
      },
    }, "Câmpul și valorile sale au fost eliminate.");
  }

  async function moveField(entry: FieldEntry, direction: -1 | 1) {
    if (!selectedModel) return;
    const targetIndex = Math.max(0, Math.min(entry.siblingCount - 1, entry.index + direction));
    if (targetIndex === entry.index) return;
    await executeMutation({
      operation: {
        kind: "reorder_field",
        modelId: selectedModel.id,
        parentFieldId: entry.parentFieldId,
        fieldId: entry.field.id,
        targetIndex,
      },
    }, "Ordinea câmpurilor a fost actualizată.");
  }

  async function attachModel() {
    if (!selectedModel || !sectionDraft) return;
    const existing = catalog?.assignments.find((assignment) => assignment.sectionPath === sectionDraft);
    if (existing) {
      if (existing.modelId === selectedModel.id) {
        error = "Modelul este deja atașat acestei secțiuni.";
        return;
      }
      const previous = catalog?.models.find((model) => model.id === existing.modelId);
      const fieldMigrations = Object.fromEntries(
        (previous?.fields ?? []).flatMap((fromField) => {
          const toField = selectedModel.fields.find((candidate) => (
            candidate.key === fromField.key && candidate.kind === fromField.kind
          ));
          return toField ? [[fromField.id, toField.id]] : [];
        }),
      );
      await executeMutation({
        operation: {
          kind: "replace_model",
          sectionPath: sectionDraft,
          fromModelId: existing.modelId,
          toModelId: selectedModel.id,
          fieldMigrations,
        },
      }, "Modelul secțiunii a fost înlocuit, iar valorile compatibile au fost migrate.");
      return;
    }
    if (await executeMutation({
      operation: {
        kind: "attach_model",
        modelId: selectedModel.id,
        sectionPath: sectionDraft,
      },
    }, "Modelul a fost atașat secțiunii.")) sectionDraft = "";
  }

  async function detachModel(sectionPath: string) {
    if (!selectedModel) return;
    await executeMutation({
      operation: {
        kind: "detach_model",
        modelId: selectedModel.id,
        sectionPath,
      },
    }, "Modelul a fost detașat, iar valorile sale au fost eliminate din pagini.");
  }

  async function deleteModel() {
    if (!selectedModel) return;
    if (await executeMutation({
      operation: { kind: "delete_model", modelId: selectedModel.id },
    }, "Modelul a fost șters.")) {
      selectedModelId = "";
      mode = "info";
    }
  }
</script>

<section class="activity-workspace content-models-workspace" aria-labelledby="content-models-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconForms size={15} /> Catalog Rust</span>
      <h1 id="content-models-title">Modele de conținut</h1>
      <p>Definește câmpuri reutilizabile, apoi atașează modelul secțiunilor Zola.</p>
    </div>
    <dl>
      <div><dt>Modele</dt><dd>{catalog?.models.length ?? 0}</dd></div>
      <div><dt>Câmpuri</dt><dd>{catalog?.models.reduce((sum, model) => sum + countFields(model.fields), 0) ?? 0}</dd></div>
      <div><dt>Atașări</dt><dd>{catalog?.assignments.length ?? 0}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label="Zonele modelului de conținut">
      {#each MODEL_VIEWS as view, index (view.id)}
        <button
          id={`content-models-tab-${view.id}`}
          class="ui-tab"
          class:active={activeView === view.id}
          type="button"
          role="tab"
          aria-selected={activeView === view.id}
          tabindex={activeView === view.id ? 0 : -1}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}<span>{viewCounts[view.id]}</span></button>
      {/each}
    </div>
    <label class="search-field">
      <span class="sr-only">Caută modele</span>
      <IconSearch size={14} stroke={1.9} />
      <input class="ui-field toolbar" bind:value={query} type="search" placeholder="Caută modele și câmpuri…" />
    </label>
    <button class="ui-icon-button toolbar" type="button" aria-label="Reîncarcă" disabled={loading || busy} onclick={() => { loadedKey = ""; void loadCatalog(); }}><IconRefresh size={14} /></button>
    <button class="ui-button primary toolbar toolbar-action" type="button" disabled={busy} onclick={beginCreateModel}><IconPlus size={14} /> Adaugă model</button>
  </div>

  <div class="workspace-body">
    <div class="model-list" role="listbox" aria-label="Modele de conținut">
      {#if loading && !catalog}
        <div class="empty">Se citește catalogul autoritativ…</div>
      {:else}
        {#each visibleModels as model (model.id)}
          <button
            class="model-card ui-entity-selectable"
            data-ui-selected={selectedModel?.id === model.id ? "true" : undefined}
            type="button"
            role="option"
            aria-selected={selectedModel?.id === model.id}
            onclick={() => selectModel(model)}
          >
            <i><IconBraces size={16} /></i>
            <span><strong>{model.label}</strong><small>{model.id} · {countFields(model.fields)} câmpuri</small></span>
          </button>
        {:else}
          <div class="empty">Nu există modele. Creează primul contract de conținut.</div>
        {/each}
      {/if}
    </div>

    <div
      class="model-detail"
      id={`content-models-panel-${activeView}`}
      role="tabpanel"
      aria-labelledby={`content-models-tab-${activeView}`}
    >
      {#if error}<div class="banner error" role="alert"><IconAlertTriangle size={15} /> {error}</div>{/if}
      {#if notice}<div class="banner success"><IconCheck size={15} /> {notice}</div>{/if}
      {#if mode === "create" || mode === "model"}
        <form onsubmit={saveModel}>
          <header><div><span>{mode === "create" ? "Model nou" : "Editare model"}</span><h2>{mode === "create" ? "Contract de conținut" : selectedModel?.label}</h2></div><button type="button" onclick={() => { mode = "info"; }}><IconX size={14} /></button></header>
          <label><span>ID stabil</span><input bind:value={modelIdDraft} placeholder="serviciu" disabled={busy} required pattern="[A-Za-z0-9_-]+" /><small>La redenumire, Rust migrează atomic fișierul, assignments și binding-urile dinamice.</small></label>
          <label><span>Nume</span><input bind:value={modelLabelDraft} placeholder="Serviciu" disabled={busy} required /></label>
          <label><span>Descriere</span><textarea bind:value={modelDescriptionDraft} placeholder="Ex.: Câmpurile folosite pentru paginile individuale ale serviciilor." disabled={busy}></textarea></label>
          <footer><button type="button" onclick={() => { mode = "info"; }} disabled={busy}>Renunță</button><button class="primary" type="submit" disabled={busy || !modelLabelDraft.trim() || mode === "create" && !modelIdDraft.trim()}>Salvează</button></footer>
        </form>
      {:else if mode === "field" && selectedModel}
        <form onsubmit={saveField}>
          <header><div><span>{selectedField ? "Editare câmp" : "Câmp nou"}</span><h2>{selectedModel.label}</h2></div><button type="button" onclick={() => { mode = "info"; }}><IconX size={14} /></button></header>
          <label>
            <span>Container</span>
            <select bind:value={fieldParentIdDraft} disabled={busy || Boolean(selectedField)}>
              <option value={null}>Nivel principal (`extra`)</option>
              {#each fieldContainers as entry (entry.field.id)}
                <option value={entry.field.id}>{"— ".repeat(entry.depth + 1)}{entry.field.label} · extra.{entry.path}</option>
              {/each}
            </select>
            {#if selectedField}<small>Containerul unui câmp existent rămâne stabil; mutarea cere migrare explicită.</small>{/if}
          </label>
          <div class="field-grid"><label><span>Cheie în `extra`</span><input bind:value={fieldKeyDraft} placeholder={FIELD_EXAMPLES[fieldKindDraft].key} disabled={busy} required pattern="[A-Za-z0-9_-]+" /></label><label><span>Etichetă</span><input bind:value={fieldLabelDraft} placeholder={FIELD_EXAMPLES[fieldKindDraft].label} disabled={busy} required /></label></div>
          <div class="field-grid"><label><span>Tip</span><select bind:value={fieldKindDraft} disabled={busy}>{#each FIELD_KINDS as kind (kind.value)}<option value={kind.value}>{kind.label}</option>{/each}</select></label><label class="check"><input type="checkbox" bind:checked={fieldRequiredDraft} disabled={busy} /> Obligatoriu</label></div>
          <label><span>Ajutor pentru editor</span><textarea bind:value={fieldHelpDraft} placeholder={FIELD_EXAMPLES[fieldKindDraft].help} disabled={busy}></textarea></label>
          <label><span>Valoare implicită</span>{#if fieldKindDraft === "boolean"}<select bind:value={fieldDefaultDraft} disabled={busy}><option value="">Fără valoare implicită</option><option value="true">Da</option><option value="false">Nu</option></select>{:else}<textarea class:code={["group", "repeater"].includes(fieldKindDraft)} bind:value={fieldDefaultDraft} rows={["group", "repeater"].includes(fieldKindDraft) ? 5 : 2} placeholder={FIELD_EXAMPLES[fieldKindDraft].defaultValue} disabled={busy}></textarea>{/if}</label>
          {#if fieldKindDraft === "select"}<label><span>Opțiuni</span><textarea bind:value={fieldChoicesDraft} placeholder={'standard|Standard\npremium|Premium'} disabled={busy}></textarea><small>Câte o opțiune pe rând, în formatul valoare|Etichetă.</small></label>{/if}
          {#if fieldKindDraft === "number"}<div class="field-grid"><label><span>Minim</span><input type="number" bind:value={fieldMinimumDraft} placeholder="Ex.: 0" disabled={busy} /></label><label><span>Maxim</span><input type="number" bind:value={fieldMaximumDraft} placeholder="Ex.: 1000" disabled={busy} /></label></div>{/if}
          {#if ["text", "textarea", "markdown", "url"].includes(fieldKindDraft)}<label><span>Validare (pattern)</span><input bind:value={fieldPatternDraft} placeholder={FIELD_EXAMPLES[fieldKindDraft].pattern} disabled={busy} /><small>Expresie regulată opțională; exemplul indică formatul acceptat.</small></label>{/if}
          {#if ["group", "repeater"].includes(fieldKindDraft)}<p class="note">După salvare poți adăuga subcâmpuri direct în acest container. ID-urile și căile lor rămân stabile.</p>{/if}
          <footer><button type="button" onclick={() => { mode = "info"; }} disabled={busy}>Renunță</button><button class="primary" type="submit" disabled={busy || !fieldKeyDraft.trim() || !fieldLabelDraft.trim()}>Salvează câmpul</button></footer>
        </form>
      {:else if activeView === "validation"}
        <div class="detail-heading validation-heading">
          <div>
            <span>Contractul proiectului</span>
            <h2>Validare catalog</h2>
            <p>Metadatele modelelor sunt salvate în proiect și verificate de Rust.</p>
          </div>
        </div>
        <section class="contract-section validation-section">
          <header><div><h3>Starea contractului</h3><p>Fișierele sunt portabile și pot fi versionate împreună cu proiectul.</p></div></header>
          <dl class="contract-grid">
            <div><dt>Rădăcină</dt><dd><code>.panastudio/</code></dd></div>
            <div><dt>Stare</dt><dd>{catalog?.metadataPresent ? "Inițializat" : "Va fi creat la prima mutație"}</dd></div>
            <div><dt>Modele</dt><dd>{catalog?.models.length ?? 0}</dd></div>
            <div><dt>Diagnostice</dt><dd>{catalog?.diagnostics.length ?? 0}</dd></div>
          </dl>
          <div class="diagnostic-list">
            {#each catalog?.diagnostics ?? [] as diagnostic (`${diagnostic.code}:${diagnostic.file}`)}
              <div class="diagnostic" class:error={diagnostic.severity === "error"}>
                <strong>{diagnostic.code}</strong>
                <span>{diagnostic.message}</span>
                {#if diagnostic.file}<code>{diagnostic.file}</code>{/if}
              </div>
            {:else}
              <div class="empty compact">Catalogul nu raportează probleme.</div>
            {/each}
          </div>
        </section>
      {:else if selectedModel}
        <div class="detail-heading"><div><span>Contract schema {selectedModel.schemaVersion}</span><h2>{selectedModel.label}</h2><p>{selectedModel.description || "Fără descriere."}</p><code>{selectedModel.file}</code></div><div><button type="button" onclick={beginEditModel}>Editează</button><button class="danger" type="button" disabled={busy} onclick={() => { void deleteModel(); }}><IconTrash size={14} /> Șterge</button></div></div>

        {#if activeView === "fields"}
        <section class="contract-section">
          <header><div><h3>Câmpuri</h3><p>Ordinea contractului devine ordinea formularului de completare.</p></div><button class="primary" type="button" onclick={() => beginField()}><IconPlus size={14} /> Adaugă</button></header>
          <div class="field-list">
            {#each schemaFields as entry (entry.field.id)}
              <article style={`--field-depth: ${entry.depth}`}>
                <button class="field-main" type="button" onclick={() => beginField(entry)}><i>{entry.index + 1}</i><span><strong>{entry.field.label}</strong><small><code>extra.{entry.path}</code> · {FIELD_KINDS.find((kind) => kind.value === entry.field.kind)?.label}{entry.field.required ? " · obligatoriu" : ""}</small></span></button>
                <div>
                  {#if entry.field.kind === "group" || entry.field.kind === "repeater"}<button type="button" disabled={busy} aria-label="Adaugă subcâmp" title="Adaugă subcâmp" onclick={() => beginField(null, entry.field.id)}><IconPlus size={13} /></button>{/if}
                  <button type="button" disabled={entry.index === 0 || busy} aria-label="Mută în sus" onclick={() => { void moveField(entry, -1); }}><IconArrowUp size={13} /></button><button type="button" disabled={entry.index === entry.siblingCount - 1 || busy} aria-label="Mută în jos" onclick={() => { void moveField(entry, 1); }}><IconArrowDown size={13} /></button><button class="danger" type="button" disabled={busy} aria-label="Șterge câmpul" onclick={() => { void removeField(entry); }}><IconTrash size={13} /></button>
                </div>
              </article>
            {:else}<div class="empty compact">Modelul nu definește încă niciun câmp.</div>{/each}
          </div>
        </section>

        {:else if activeView === "sections"}
        <section class="contract-section">
          <header><div><h3>Atașări la secțiuni</h3><p>Cel mai specific contract moștenit de o pagină este autoritativ.</p></div></header>
          <div class="attach-row"><select bind:value={sectionDraft} disabled={busy}><option value="">Alege secțiunea…</option>{#each sections as section (section.file)}<option value={section.file}>{section.title} · {section.file}{catalog?.assignments.some((assignment) => assignment.sectionPath === section.file) ? " · are model" : ""}</option>{/each}</select><button class="primary" type="button" disabled={!sectionDraft || busy} onclick={() => { void attachModel(); }}><IconLink size={14} /> {sectionDraftAssignment ? "Înlocuiește" : "Atașează"}</button></div>
          {#if sectionDraftAssignment && sectionDraftAssignment.modelId !== selectedModel.id}<p class="note">Secțiunea folosește modelul {sectionDraftAssignment.modelId}. Înlocuirea migrează automat câmpurile-rădăcină cu aceeași cheie și același tip; Rust afișează planul distructiv înainte de aplicare.</p>{/if}
          <div class="assignment-list">{#each selectedAssignments as assignment (assignment.sectionPath)}<article><span><strong>{assignment.sectionPath}</strong><small>{catalog?.pageBindings.filter((binding) => binding.sectionPath === assignment.sectionPath).length ?? 0} pagini proiectate</small></span><button class="danger" type="button" disabled={busy} onclick={() => { void detachModel(assignment.sectionPath); }}><IconX size={14} /> Detașează</button></article>{:else}<div class="empty compact">Modelul nu este atașat niciunei secțiuni.</div>{/each}</div>
        </section>

        {:else if activeView === "usages"}
        <section class="contract-section">
          <header><div><h3>Consumatori Tera</h3><p>Aceste legături blochează ștergerea sau detașarea distructivă.</p></div></header>
          <div class="usage-list">{#each selectedUsages as usage (`${usage.templateFile}:${usage.offset}:${usage.fieldId}`)}<button type="button" onclick={() => { void openWorkspaceSource(usage.templateFile); }}><code>{usage.expression}</code><span>{usage.templateFile}</span></button>{:else}<div class="empty compact">Nicio expresie Tera nu consumă câmpurile modelului.</div>{/each}</div>
        </section>
        {/if}
      {:else}
        <div class="empty">Selectează sau creează un model de conținut.</div>
      {/if}
    </div>

  </div>
</section>

<style>
  .workspace-body { display: grid; grid-template-columns: minmax(230px, .42fr) minmax(460px, 1fr); min-width: 0; min-height: 0; }
  .view-tabs button span { min-width: 17px; padding: 1px 4px; border-radius: 9px; background: var(--wb-surface-document); font-size: 11px; text-align: center; }
  .contract-section p, .detail-heading p { margin: 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  .banner { display: flex; align-items: center; gap: 6px; margin-bottom: 10px; padding: 7px 9px; border: 1px solid; border-radius: 7px; font-size: 12px; }
  .banner.error { border-color: color-mix(in srgb, var(--danger) 35%, transparent); color: var(--danger); background: color-mix(in srgb, var(--danger) 8%, var(--wb-surface-document)); }
  .banner.success { border-color: color-mix(in srgb, var(--success) 35%, transparent); color: var(--success); background: color-mix(in srgb, var(--success) 8%, var(--wb-surface-document)); }
  .model-list { min-width: 0; min-height: 0; overflow: auto; padding: 8px; background: var(--wb-surface-document); }
  .model-list { border-right: 1px solid var(--wb-border-subtle); }
  .model-list > button { display: flex; width: 100%; align-items: center; gap: 8px; padding: 8px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .model-list i { display: grid; width: 28px; height: 28px; flex: 0 0 auto; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-surface-document); }
  .model-list span, .field-main span, .assignment-list span { display: grid; min-width: 0; gap: 2px; }
  strong { color: var(--text-strong); font-size: 12px; } small { color: var(--wb-text-muted); font-size: 11px; }
  .model-detail { min-width: 0; min-height: 0; overflow: auto; padding: 17px; background: var(--wb-surface-chrome); }
  .diagnostic-list { display: grid; gap: 7px; margin-top: 10px; }
  .diagnostic { display: grid; gap: 3px; padding: 8px; border: 1px solid color-mix(in srgb, var(--wb-warning) 35%, var(--wb-border-subtle)); border-radius: 6px; background: var(--wb-surface-document); font-size: 11px; }
  .diagnostic.error { border-color: color-mix(in srgb, var(--danger) 35%, var(--wb-border-subtle)); }
  .diagnostic code { color: var(--wb-text-muted); overflow-wrap: anywhere; }
  .detail-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; padding-bottom: 14px; border-bottom: 1px solid var(--wb-border-subtle); }
  .detail-heading h2 { margin: 3px 0; color: var(--text-strong); font-size: 21px; }
  .detail-heading > div + div { display: flex; gap: 6px; }
  .detail-heading code { display: inline-block; margin-top: 7px; color: var(--wb-accent-strong); font-size: 11px; }
  button, input, select, textarea { font: inherit; } button:not(:disabled) { cursor: pointer; }
  button { border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  button.primary, .ui-button.primary { border-color: var(--wb-accent); color: #fff; background: var(--wb-accent); }
  button.danger { display: inline-flex; align-items: center; gap: 4px; color: var(--danger); }
  .contract-section { margin-top: 14px; padding: 12px; border: 1px solid var(--wb-border-subtle); border-radius: 9px; background: var(--wb-surface-chrome); }
  .contract-section > header { display: flex; align-items: center; justify-content: space-between; gap: 10px; margin-bottom: 9px; }
  .contract-section h3 { margin: 0 0 2px; color: var(--text-strong); font-size: 14px; }
  .contract-section header > button { display: flex; align-items: center; gap: 4px; min-height: 29px; padding: 0 8px; }
  .contract-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; margin: 0; }
  .contract-grid div { min-width: 0; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .contract-grid dt { color: var(--wb-text-muted); font-size: 11px; font-weight: 800; text-transform: uppercase; }
  .contract-grid dd { margin: 3px 0 0; color: var(--text-strong); font-size: 13px; font-weight: 650; overflow-wrap: anywhere; }
  .field-list, .assignment-list, .usage-list { display: grid; gap: 5px; }
  .field-list article, .assignment-list article { display: flex; align-items: center; gap: 6px; padding: 5px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .field-list article { margin-left: calc(var(--field-depth, 0) * 18px); }
  .field-main { display: flex; min-width: 0; flex: 1; align-items: center; gap: 7px; padding: 3px; border: 0; text-align: left; }
  .field-main i { display: grid; width: 23px; height: 23px; place-items: center; border-radius: 5px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 11px; font-style: normal; font-weight: 800; }
  .field-list article > div { display: flex; gap: 3px; } .field-list article > div button { display: grid; width: 25px; height: 25px; padding: 0; place-items: center; }
  .assignment-list article { justify-content: space-between; padding: 8px; } .assignment-list article > button { display: flex; align-items: center; gap: 4px; padding: 5px 7px; }
  .attach-row { display: flex; gap: 6px; margin-bottom: 7px; } .attach-row select { min-width: 0; flex: 1; } .attach-row button { display: flex; align-items: center; gap: 4px; padding: 0 9px; }
  .usage-list button { display: grid; gap: 2px; padding: 7px; text-align: left; } .usage-list code { color: var(--wb-accent-strong); } .usage-list span { color: var(--wb-text-muted); font-size: 11px; }
  form { display: grid; gap: 11px; max-width: 720px; margin: 0 auto; padding: 14px; border: 1px solid var(--wb-border-subtle); border-radius: 10px; background: var(--wb-surface-chrome); }
  form header { display: flex; justify-content: space-between; padding-bottom: 10px; border-bottom: 1px solid var(--wb-border-subtle); } form header span { color: var(--wb-accent-strong); font-size: 11px; font-weight: 800; text-transform: uppercase; } form h2 { margin: 2px 0 0; color: var(--text-strong); font-size: 18px; } form header button { width: 27px; height: 27px; }
  form label { display: grid; gap: 4px; color: var(--wb-text-muted); font-size: 11px; font-weight: 700; } form input, form select, form textarea, .attach-row select { min-height: 32px; padding: 6px 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--material-inset); } form textarea { min-height: 74px; resize: vertical; } form small { font-weight: 400; }
  form textarea.code { font-family: "JetBrains Mono", monospace; font-size: 11px; }
  .field-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 9px; } label.check { display: flex; align-items: center; align-self: end; min-height: 32px; flex-direction: row; } label.check input { min-height: auto; }
  form footer { display: flex; justify-content: flex-end; gap: 6px; padding-top: 8px; border-top: 1px solid var(--wb-border-subtle); } form footer button { min-height: 31px; padding: 0 10px; }
  .note { padding: 8px; border-left: 3px solid var(--wb-accent); color: var(--wb-text-muted); background: var(--wb-accent-soft); font-size: 11px; }
  .empty { display: grid; min-height: 160px; place-items: center; padding: 18px; color: var(--wb-text-muted); font-size: 12px; text-align: center; } .empty.compact { min-height: 56px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @media (max-width: 1050px) { .workspace-body { grid-template-columns: 220px minmax(440px, 1fr); } }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; grid-template-rows: minmax(160px, 230px) minmax(0, 1fr); } .model-list { border-right: 0; border-bottom: 1px solid var(--wb-border-subtle); } .workspace-header dl { display: none; } }
</style>
