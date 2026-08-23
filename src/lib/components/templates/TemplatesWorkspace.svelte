<script lang="ts">
  import {
    IconAlertTriangle,
    IconArticle,
    IconCopy,
    IconDeviceFloppy,
    IconEdit,
    IconExternalLink,
    IconFileCode,
    IconFileText,
    IconHome,
    IconLayout,
    IconListDetails,
    IconPlus,
    IconRefresh,
    IconSearch,
    IconTags,
    IconTemplate,
    IconTrash,
    IconX,
  } from "@tabler/icons-svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import {
  createListingItem,
  createSemanticTemplate,
  deleteListingItem,
  deleteTemplate,
  duplicateTemplate,
  overrideThemeTemplate,
  readTemplateCatalog,
  renameTemplate,
  setTemplateAssignment,
  setTemplateParent,
} from "$lib/templates/io";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type {
    FileBufferRequestIdentity,
    WorkspaceEntryMutationReceipt,
  } from "$lib/project/workspace-contract";
  import type { SourceGraph } from "$lib/source-graph/graph-contract";
  import type {
    TemplateAssignmentSource,
    TemplateResource,
    TemplateSemanticCategory,
    TemplateSemanticCreateRole,
    TemplateSemanticEntry,
    TemplateSemanticRole,
  } from "$lib/templates/contracts";
  import type { WorkspaceSourceOpenOptions } from "$lib/workbench/contracts";
  import { errorMessage } from "$lib/util";

  let {
    globalStatus,
    workspaceMutations,
    sourceGraph,
    activeScannedPath,
    openEditor,
    openWorkspaceSource,
  }: {
    globalStatus: GlobalStatusState;
    workspaceMutations: ProjectWorkspaceMutationService;
    sourceGraph: SourceGraph | null;
    activeScannedPath: string | null;
    openEditor: () => void | Promise<void>;
    openWorkspaceSource: (
      path: string,
      options?: WorkspaceSourceOpenOptions,
    ) => void | Promise<void>;
  } = $props();

  type DetailMode = "info" | "create" | "rename";
  type CreateTarget = { id: string; label: string; file: string | null; url: string | null };
  type SectionSort = "none" | "date" | "title" | "weight";

  const NEW_SECTION_TARGET = "__new_section__";

  function diagnosticText(diagnostic: import("$lib/contracts/localized-diagnostic").LocalizedDiagnostic | null | undefined) {
    return diagnostic ? errorMessage(diagnostic) : "";
  }

  function entryLabel(entry: TemplateSemanticEntry) {
    return diagnosticText(entry.labelDiagnostic)
      || entry.label
      || t("templates-semantic-label-unavailable");
  }

  function targetLabel(target: TemplateSemanticEntry["target"]) {
    return diagnosticText(target.labelDiagnostic)
      || target.label
      || t("templates-semantic-label-unavailable");
  }

  function previewTitle(context: NonNullable<TemplateSemanticEntry["previewContext"]>) {
    return diagnosticText(context.titleDiagnostic)
      || context.title
      || t("templates-semantic-label-unavailable");
  }

  function createTarget(entry: TemplateSemanticEntry): CreateTarget {
    return {
      id: entry.target.id,
      label: targetLabel(entry.target),
      file: entry.target.file,
      url: entry.target.url,
    };
  }

  const views = $derived([
    { id: "layout" as const, label: t("templates-view-layouts") },
    { id: "page" as const, label: t("templates-view-pages") },
    { id: "archive" as const, label: t("templates-view-archives") },
    { id: "element" as const, label: t("templates-view-elements") },
    { id: "listing_item" as const, label: t("templates-view-listing-items") },
    { id: "taxonomy" as const, label: t("templates-view-taxonomies") },
    { id: "system" as const, label: t("templates-view-system") },
  ]);

  const createRoles: Record<TemplateSemanticCategory, TemplateSemanticCreateRole[]> = {
    layout: ["layout"],
    page: ["homepage", "default_page", "specific_page", "custom"],
    archive: ["section_archive"],
    element: ["section_element"],
    listing_item: ["listing_item"],
    taxonomy: ["taxonomy_list", "taxonomy_term"],
    system: ["not_found"],
  };

  let activeView = $state<TemplateSemanticCategory>("layout");
  let query = $state("");
  let catalog = $state<Awaited<ReturnType<typeof readTemplateCatalog>> | null>(null);
  let selectedId = $state<string | null>(null);
  let loading = $state(false);
  let busy = $state(false);
  let loadedKey = $state("");
  let loadError = $state("");
  let formError = $state("");
  let detailMode = $state<DetailMode>("info");
  let deleteConfirmationOpen = $state(false);

  let createRole = $state<TemplateSemanticCreateRole>("layout");
  let createName = $state("");
  let createTargetId = $state("");
  let createParent = $state("");
  let createSectionTitle = $state("");
  let createSectionSlug = $state("");
  let createSectionSort = $state<SectionSort>("weight");
  let createSectionSlugTouched = $state(false);
  let createNameTouched = $state(false);
  let includePageContent = $state(false);
  let listingLabel = $state("");
  let listingModelId = $state("");
  let listingPreviewPageFile = $state("");
  let duplicateSourcePath = $state<string | null>(null);
  let draftNameInput = $state<HTMLInputElement | null>(null);
  let draftSectionTitleInput = $state<HTMLInputElement | null>(null);

  let parentDraft = $state("");
  let parentDraftForId = $state("");
  let assignmentDraft = $state("");
  let assignmentDraftForId = $state("");

  const resources = $derived(catalog?.resources ?? []);
  const effectiveResources = $derived(resources.filter((resource) => resource.effective));
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const visibleEntries = $derived(
    (catalog?.semanticEntries ?? []).filter((entry) => (
      entry.category === activeView
      && (
        !normalizedQuery
        || `${entryLabel(entry)} ${targetLabel(entry.target)} ${entry.target.file ?? ""} ${entry.target.url ?? ""} ${entry.assignment.resourceName}`
          .toLocaleLowerCase(l10n.locale)
          .includes(normalizedQuery)
      )
    )),
  );
  const selectedEntry = $derived(
    visibleEntries.find((entry) => entry.id === selectedId) ?? visibleEntries[0] ?? null,
  );
  const selectedResource = $derived(resourceById(selectedEntry?.assignment.resourceId ?? null));
  const layoutOptions = $derived(
    effectiveResources.filter((resource) => resource.roles.includes("layout")),
  );
  const assignableResources = $derived(
    effectiveResources.filter((resource) => (
      resource.roles.includes("page")
      && !resource.roles.some((role) => (
        role === "partial" || role === "macro_library" || role === "shortcode"
      ))
    )),
  );
  const counts = $derived(Object.fromEntries(
    views.map((view) => [
      view.id,
      (catalog?.semanticEntries ?? []).filter((entry) => entry.category === view.id).length,
    ]),
  ) as Record<TemplateSemanticCategory, number>);
  const localResourceCount = $derived(resources.filter((resource) => resource.editable).length);
  const themeResourceCount = $derived(resources.filter((resource) => !resource.editable).length);
  const createTargets = $derived(targetsForRole(createRole));
  const listingModels = $derived(sourceGraph?.contentModels.models ?? []);
  const listingPreviewPages = $derived(
    (sourceGraph?.contentModels.pageBindings ?? [])
      .filter((binding) => binding.modelId === listingModelId)
      .map((binding) => sourceGraph?.pages.find((page) => page.file === binding.pageFile))
      .filter((page): page is NonNullable<typeof page> => Boolean(page)),
  );
  const creatingNewArchiveSection = $derived(
    createRole === "section_archive" && createTargetId === NEW_SECTION_TARGET,
  );
  const canAssignSelected = $derived(
    selectedEntry?.target.file
      && (selectedEntry.assignment.key === "template" || selectedEntry.assignment.key === "page_template"),
  );

  $effect(() => {
    const root = workspaceMutations.snapshot?.projectRoot.trim() ?? "";
    const sessionId = workspaceMutations.snapshot?.runtimeSessionId.trim() ?? "";
    const revision = workspaceMutations.snapshot?.revision ?? 0;
    const key = `${root}:${sessionId}:${revision}`;
    if (!root || !sessionId || loading || loadedKey === key) return;
    loadedKey = key;
    void loadCatalog(root, sessionId, revision);
  });

  $effect(() => {
    if (!selectedEntry) return;
    const key = `${selectedEntry.id}:${selectedEntry.assignment.resourceName}`;
    if (assignmentDraftForId === key) return;
    assignmentDraftForId = key;
    assignmentDraft = selectedEntry.assignment.resourceName;
  });

  $effect(() => {
    if (!selectedResource) return;
    const key = `${selectedResource.id}:${selectedResource.extends ?? ""}`;
    if (parentDraftForId === key) return;
    parentDraftForId = key;
    parentDraft = selectedResource.extends ?? "";
  });

  async function loadCatalog(
    root = workspaceMutations.snapshot?.projectRoot ?? "",
    sessionId = workspaceMutations.snapshot?.runtimeSessionId ?? "",
    expectedWorkspaceRevision = workspaceMutations.snapshot?.revision ?? 0,
  ) {
    loading = true;
    loadError = "";
    try {
      const snapshot = await readTemplateCatalog({
        expectedProjectRoot: root,
        expectedSessionId: sessionId,
      }, expectedWorkspaceRevision);
      if (
        root !== (workspaceMutations.snapshot?.projectRoot ?? "")
        || sessionId !== (workspaceMutations.snapshot?.runtimeSessionId ?? "")
        || workspaceMutations.snapshot?.revision !== expectedWorkspaceRevision
      ) return;
      catalog = snapshot;
      const activeResource = snapshot.resources.find(
        (resource) => resource.effective && resource.file === activeScannedPath,
      );
      const activeSemantic = snapshot.semanticEntries.find(
        (entry) => (
          entry.id === selectedId
          || entry.assignment.resourceId === activeResource?.id
          || entry.target.file === activeScannedPath
        ),
      );
      if (activeSemantic) {
        activeView = activeSemantic.category;
        selectedId = activeSemantic.id;
      } else if (!snapshot.semanticEntries.some(
        (entry) => entry.id === selectedId && entry.category === activeView,
      )) {
        selectedId = snapshot.semanticEntries.find((entry) => entry.category === activeView)?.id ?? null;
      }
    } catch (error) {
      if (
        root === (workspaceMutations.snapshot?.projectRoot ?? "")
        && sessionId === (workspaceMutations.snapshot?.runtimeSessionId ?? "")
      ) {
        loadError = errorMessage(error);
      }
    } finally {
      if (
        root === (workspaceMutations.snapshot?.projectRoot ?? "")
        && sessionId === (workspaceMutations.snapshot?.runtimeSessionId ?? "")
      ) {
        loading = false;
      }
    }
  }

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: workspaceMutations.snapshot?.projectRoot ?? "",
      expectedSessionId: workspaceMutations.snapshot?.runtimeSessionId ?? "",
    };
  }

  async function finishMutation(
    operation: () => Promise<WorkspaceEntryMutationReceipt>,
    successMessage: string,
  ) {
    if (busy) return null;
    busy = true;
    formError = "";
    try {
      let receipt: WorkspaceEntryMutationReceipt;
      try {
        receipt = await operation();
      } catch (error) {
        formError = errorMessage(error);
        globalStatus.set(t("templates-operation-failed", { error: formError }), "error");
        return null;
      }
      const settlement = await workspaceMutations.settle(receipt, {
        preferredRelativePath: receipt.relativePath,
        warningLabel: t("templates-operation-label"),
      });
      loadedKey = "";
      await loadCatalog();
      const next = catalog?.semanticEntries.find((entry) => (
        entry.id === selectedId
        || entry.target.file === receipt.relativePath
        || resourceById(entry.assignment.resourceId)?.file === receipt.relativePath
      ));
      if (next) {
        activeView = next.category;
        selectedId = next.id;
      }
      globalStatus.set(t(
        settlement.warnings.length ? "templates-saved-status-warning" : "templates-saved-status",
        { message: successMessage },
      ), "unsaved");
      return receipt;
    } finally {
      busy = false;
    }
  }

  function resourceById(id: string | null) {
    return id ? effectiveResources.find((resource) => resource.id === id) ?? null : null;
  }

  function originLabel(resource: TemplateResource) {
    return resource.editable ? t("templates-origin-local") : resource.themeName ?? t("templates-origin-theme");
  }

  function assignmentSourceLabel(source: TemplateAssignmentSource) {
    if (source === "explicit") return t("templates-assignment-source-explicit");
    if (source === "inherited") return t("templates-assignment-source-inherited");
    if (source === "convention") return t("templates-assignment-source-convention");
    return t("templates-assignment-source-default");
  }

  function roleLabel(role: TemplateSemanticRole) {
    const labels: Record<TemplateSemanticRole, string> = {
      layout: t("templates-role-layout"),
      homepage: t("templates-role-homepage"),
      default_page: t("templates-role-default-page"),
      specific_page: t("templates-role-specific-page"),
      section_archive: t("templates-role-section-archive"),
      section_element: t("templates-role-section-element"),
      listing_item: t("templates-role-listing-item"),
      taxonomy_list: t("templates-role-taxonomy-list"),
      taxonomy_term: t("templates-role-taxonomy-term"),
      not_found: t("templates-role-not-found"),
      custom: t("templates-role-custom"),
    };
    return labels[role];
  }

  function targetsForRole(role: TemplateSemanticCreateRole): CreateTarget[] {
    const entries = catalog?.semanticEntries ?? [];
    if (role === "homepage") {
      return entries.filter((entry) => entry.role === "homepage").map(createTarget);
    }
    if (role === "specific_page") {
      const pages = entries.flatMap((entry) => entry.affectedPages);
      return pages
        .filter((page, index) => pages.findIndex((candidate) => candidate.file === page.file) === index)
        .map((page) => ({ id: page.file, label: page.title, file: page.file, url: page.url }));
    }
    if (role === "section_archive") {
      return entries.filter((entry) => entry.role === "section_archive").map(createTarget);
    }
    if (role === "section_element") {
      return entries.filter((entry) => entry.role === "section_element").map(createTarget);
    }
    if (role === "taxonomy_list" || role === "taxonomy_term") {
      return entries
        .filter((entry) => entry.role === "taxonomy_list")
        .map(createTarget);
    }
    return [];
  }

  function suggestedName(role: TemplateSemanticCreateRole, targetId = createTargetId) {
    if (role === "layout") return "new-layout";
    if (role === "homepage") return "index";
    if (role === "default_page") return "page";
    if (role === "not_found") return "404";
    if (role === "custom") return "new-template";
    const target = targetsForRole(role).find((candidate) => candidate.id === targetId);
    const labelStem = target?.label
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLocaleLowerCase(l10n.locale)
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "");
    const stem = (role === "taxonomy_list" || role === "taxonomy_term")
      ? labelStem || "taxonomy"
      : target?.file
      ?.replace(/^content\//, "")
      .replace(/\/_index\.md$/, "")
      .replace(/\.md$/, "")
      || labelStem
      || "template";
    if (role === "section_archive" || role === "taxonomy_list") return `${stem}/list`;
    if (role === "section_element" || role === "taxonomy_term") return `${stem}/single`;
    return stem;
  }

  function collectionSlug(value: string) {
    return value
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLocaleLowerCase(l10n.locale)
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
  }

  function generatedArchiveName(slug: string) {
    return slug ? `${slug}/arhiva` : "";
  }

  function resetNewSectionDraft() {
    createSectionTitle = "";
    createSectionSlug = "";
    createSectionSort = "weight";
    createSectionSlugTouched = false;
    createNameTouched = false;
  }

  function updateSectionTitle(value: string) {
    createSectionTitle = value;
    if (createSectionSlugTouched) return;
    createSectionSlug = collectionSlug(value);
    if (!createNameTouched) createName = generatedArchiveName(createSectionSlug);
  }

  function updateSectionSlug(value: string) {
    createSectionSlugTouched = true;
    createSectionSlug = collectionSlug(value);
    if (!createNameTouched) createName = generatedArchiveName(createSectionSlug);
  }

  function resetPanelState() {
    detailMode = "info";
    formError = "";
    duplicateSourcePath = null;
    deleteConfirmationOpen = false;
  }

  function beginCreate() {
    createRole = createRoles[activeView][0] ?? "custom";
    const targets = targetsForRole(createRole);
    resetNewSectionDraft();
    createTargetId = createRole === "section_archive"
      ? NEW_SECTION_TARGET
      : targets[0]?.id ?? "";
    createName = createRole === "section_archive" ? "" : suggestedName(createRole, createTargetId);
    createParent = layoutOptions[0]?.name ?? "";
    if (createRole === "listing_item") {
      listingLabel = "";
      createName = "";
      listingModelId = listingModels[0]?.id ?? "";
      listingPreviewPageFile = (sourceGraph?.contentModels.pageBindings ?? [])
        .find((binding) => binding.modelId === listingModelId)?.pageFile ?? "";
    }
    includePageContent = false;
    duplicateSourcePath = null;
    formError = "";
    deleteConfirmationOpen = false;
    detailMode = "create";
    focusDraftName();
  }

  function beginCreateForEntry(entry: TemplateSemanticEntry) {
    const role = entry.role as TemplateSemanticCreateRole;
    if (!createRoles[entry.category].includes(role)) {
      beginCreate();
      return;
    }
    activeView = entry.category;
    createRole = role;
    resetNewSectionDraft();
    createTargetId = targetsForRole(role).some((target) => target.id === entry.target.id)
      ? entry.target.id
      : "";
    createName = suggestedName(role, createTargetId);
    createParent = layoutOptions[0]?.name ?? "";
    includePageContent = false;
    duplicateSourcePath = null;
    formError = "";
    deleteConfirmationOpen = false;
    detailMode = "create";
    focusDraftName();
  }

  function changeCreateRole(role: TemplateSemanticCreateRole) {
    createRole = role;
    const targets = targetsForRole(role);
    resetNewSectionDraft();
    createTargetId = role === "section_archive" ? NEW_SECTION_TARGET : targets[0]?.id ?? "";
    createName = role === "section_archive" ? "" : suggestedName(role, createTargetId);
  }

  function changeCreateTarget(targetId: string) {
    createTargetId = targetId;
    createNameTouched = false;
    createName = targetId === NEW_SECTION_TARGET
      ? generatedArchiveName(createSectionSlug)
      : suggestedName(createRole, targetId);
  }

  function beginDuplicate(resource: TemplateResource) {
    duplicateSourcePath = resource.file;
    createRole = resource.roles.includes("layout") ? "layout" : "custom";
    createName = resource.name.replace(/\.html$/i, "") + "-copy";
    createParent = "";
    detailMode = "create";
    formError = "";
    focusDraftName();
  }

  function beginRename(resource: TemplateResource) {
    createName = resource.file.replace(/^templates\//, "").replace(/\.html$/i, "");
    duplicateSourcePath = null;
    detailMode = "rename";
    formError = "";
    focusDraftName();
  }

  function focusDraftName() {
    requestAnimationFrame(() => {
      const input = detailMode === "create" && creatingNewArchiveSection
        ? draftSectionTitleInput
        : draftNameInput;
      input?.focus();
      input?.select();
    });
  }

  function updateListingLabel(value: string) {
    listingLabel = value;
    if (!createNameTouched) createName = collectionSlug(value);
  }

  function changeListingModel(modelId: string) {
    listingModelId = modelId;
    listingPreviewPageFile = (sourceGraph?.contentModels.pageBindings ?? [])
      .find((binding) => binding.modelId === modelId)?.pageFile ?? "";
  }

  async function submitCreate(event: SubmitEvent) {
    event.preventDefault();
    const name = createName.trim();
    if (createRole === "listing_item" && !duplicateSourcePath) {
      if (!listingLabel.trim() || !name || !listingModelId || !listingPreviewPageFile) {
        formError = t("templates-listing-required");
        return;
      }
      const receipt = await finishMutation(
        () => createListingItem({
          label: listingLabel.trim(),
          slug: name,
          modelId: listingModelId,
          previewPageFile: listingPreviewPageFile,
        }, identity()),
        t("templates-listing-created", { name: listingLabel.trim() }),
      );
      if (receipt) resetPanelState();
      return;
    }
    if (creatingNewArchiveSection && !createSectionTitle.trim()) {
      formError = t("templates-section-title-required");
      return;
    }
    if (creatingNewArchiveSection && !createSectionSlug.trim()) {
      formError = t("templates-section-slug-required");
      return;
    }
    if (!name) {
      formError = t("templates-name-required");
      return;
    }
    if (duplicateSourcePath) {
      const receipt = await finishMutation(
        () => duplicateTemplate({
          sourceRelativePath: duplicateSourcePath ?? "",
          destinationName: name,
        }, identity()),
        t("templates-status-duplicated", { name }),
      );
      if (receipt) resetPanelState();
      return;
    }
    if (!creatingNewArchiveSection && createTargets.length > 0 && !createTargetId) {
      formError = t("templates-target-required");
      return;
    }
    const receipt = await finishMutation(
      () => createSemanticTemplate({
        role: createRole,
        name,
        targetId: creatingNewArchiveSection ? null : createTargetId || null,
        newSection: creatingNewArchiveSection ? {
          title: createSectionTitle.trim(),
          slug: createSectionSlug,
          sortBy: createSectionSort,
        } : null,
        parentTemplateName: createParent || null,
        includePageContent,
      }, identity()),
      creatingNewArchiveSection
        ? t("templates-status-section-created", { title: createSectionTitle.trim() })
        : t("templates-status-created", { name }),
    );
    if (receipt) resetPanelState();
  }

  async function submitRename(event: SubmitEvent, resource: TemplateResource) {
    event.preventDefault();
    const name = createName.trim();
    if (!name) {
      formError = t("templates-name-required");
      return;
    }
    const receipt = await finishMutation(
      () => renameTemplate({
        sourceRelativePath: resource.file,
        destinationName: name,
      }, identity()),
      t("templates-status-renamed", { name }),
    );
    if (receipt) resetPanelState();
  }

  async function saveAssignment(entry: TemplateSemanticEntry) {
    if (!entry.target.file || !entry.assignment.key) return;
    const key = entry.assignment.key;
    if (key !== "template" && key !== "page_template") return;
    await finishMutation(
      () => setTemplateAssignment({
        contentRelativePath: entry.target.file ?? "",
        key,
        templateName: assignmentDraft || null,
      }, identity()),
      t("templates-status-assignment-updated", { name: entryLabel(entry) }),
    );
  }

  async function clearAssignment(entry: TemplateSemanticEntry) {
    if (!entry.target.file || !entry.assignment.key) return;
    const key = entry.assignment.key;
    if (key !== "template" && key !== "page_template") return;
    await finishMutation(
      () => setTemplateAssignment({
        contentRelativePath: entry.target.file ?? "",
        key,
        templateName: null,
      }, identity()),
      t("templates-status-assignment-cleared", { name: entryLabel(entry) }),
    );
  }

  async function saveParent(resource: TemplateResource) {
    await finishMutation(
      () => setTemplateParent({
        relativePath: resource.file,
        parentTemplateName: parentDraft || null,
      }, identity()),
      t("templates-status-parent-updated", { name: resource.name }),
    );
  }

  async function openResource(entry: TemplateSemanticEntry, resource: TemplateResource) {
    const context = entry.previewContext;
    if (context?.available && context.pageFile) {
      await openWorkspaceSource(resource.file, {
        surface: "visual",
        templateContextPagePath: context.pageFile,
      });
    } else if (context?.available && context.url) {
      await openWorkspaceSource(resource.file, {
        surface: "visual",
        templateContextUrl: context.url,
      });
    } else {
      await openWorkspaceSource(resource.file);
    }
    await openEditor();
  }

  async function overrideResource(resource: TemplateResource) {
    const receipt = await finishMutation(
      () => overrideThemeTemplate({ sourceRelativePath: resource.file }, identity()),
      t("templates-status-override-created", { path: resource.localOverridePath }),
    );
    if (receipt) resetPanelState();
  }

  async function removeResource(entry: TemplateSemanticEntry, resource: TemplateResource) {
    if (!resource.canDelete) return;
    const receipt = await finishMutation(
      () => entry.role === "listing_item"
        ? deleteListingItem({ id: entry.target.id }, identity())
        : deleteTemplate({ relativePath: resource.file }, identity()),
      t("templates-status-deleted", { name: resource.name }),
    );
    if (receipt) resetPanelState();
  }

  function changeView(view: TemplateSemanticCategory) {
    activeView = view;
    selectedId = catalog?.semanticEntries.find((entry) => entry.category === view)?.id ?? null;
    resetPanelState();
  }

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let next: number | null = null;
    if (event.key === "ArrowLeft") next = (index - 1 + views.length) % views.length;
    if (event.key === "ArrowRight") next = (index + 1) % views.length;
    if (event.key === "Home") next = 0;
    if (event.key === "End") next = views.length - 1;
    if (next === null) return;
    event.preventDefault();
    changeView(views[next]?.id ?? "layout");
    requestAnimationFrame(() => document.getElementById(`templates-tab-${next}`)?.focus());
  }
</script>

<section class="activity-workspace templates-workspace" aria-labelledby="templates-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconTemplate size={15} stroke={1.9} /> {t("templates-eyebrow")}</span>
      <h1 id="templates-title">{t("templates-title")}</h1>
      <p>{t("templates-description")}</p>
    </div>
    <dl>
      <div><dt>{t("templates-stat-roles")}</dt><dd>{l10n.formatNumber(catalog?.semanticEntries.length ?? 0)}</dd></div>
      <div><dt>{t("templates-stat-resources")}</dt><dd>{l10n.formatNumber(resources.length)}</dd></div>
      <div><dt>{t("templates-stat-local")}</dt><dd>{l10n.formatNumber(localResourceCount)}</dd></div>
      <div><dt>{t("templates-stat-theme")}</dt><dd>{l10n.formatNumber(themeResourceCount)}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label={t("templates-tabs-label")}>
      {#each views as view, index (view.id)}
        <button
          id={`templates-tab-${index}`}
          class="ui-tab"
          type="button"
          role="tab"
          aria-selected={activeView === view.id}
          tabindex={activeView === view.id ? 0 : -1}
          class:active={activeView === view.id}
          onclick={() => changeView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}<span>{l10n.formatNumber(counts[view.id])}</span></button>
      {/each}
    </div>
    <label class="search-field">
      <span class="sr-only">{t("templates-search-label")}</span>
      <IconSearch size={14} stroke={1.9} />
      <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("templates-search-placeholder")} />
    </label>
    <button class="ui-button primary toolbar toolbar-action" type="button" disabled={busy} onclick={beginCreate}>
      <IconPlus size={14} stroke={2} /> {t("templates-add")}
    </button>
  </div>

  <div class="workspace-body">
    <div class="template-list" role="listbox" aria-label={t("templates-list-label")}>
      {#if loadError}
        <div class="workspace-state error" role="alert">{loadError}</div>
      {:else if loading && !catalog}
        <div class="workspace-state">
          <span class="spin"><IconRefresh size={18} /></span>
          {t("templates-loading")}
        </div>
      {:else}
        {#each visibleEntries as entry (entry.id)}
          {@const resource = resourceById(entry.assignment.resourceId)}
          <button
            class="template-card ui-entity-selectable"
            data-ui-selected={selectedEntry?.id === entry.id ? "true" : undefined}
            type="button"
            role="option"
            aria-selected={selectedEntry?.id === entry.id}
            onclick={() => { selectedId = entry.id; resetPanelState(); }}
          >
            <span class="resource-icon">
              {#if entry.role === "layout"}<IconLayout size={17} stroke={1.8} />
              {:else if entry.role === "homepage"}<IconHome size={17} stroke={1.8} />
              {:else if entry.category === "archive"}<IconListDetails size={17} stroke={1.8} />
              {:else if entry.category === "element" || entry.category === "listing_item"}<IconArticle size={17} stroke={1.8} />
              {:else if entry.category === "taxonomy"}<IconTags size={17} stroke={1.8} />
              {:else if entry.category === "system"}<IconAlertTriangle size={17} stroke={1.8} />
              {:else}<IconFileText size={17} stroke={1.8} />{/if}
            </span>
            <span class="card-copy">
              <strong>{entryLabel(entry)}</strong>
              <small>{entry.target.url ?? entry.target.file ?? targetLabel(entry.target)}</small>
            </span>
            <span class="technical-copy">
              <em>{entry.assignment.resourceName}</em>
              <small>{assignmentSourceLabel(entry.assignment.source)}</small>
            </span>
            {#if resource}
              <span class:theme={!resource.editable} class="origin">{originLabel(resource)}</span>
            {:else}
              <span class="origin missing">{t("templates-missing")}</span>
            {/if}
          </button>
        {:else}
          <div class="workspace-state">{t("templates-empty-category")}</div>
        {/each}
      {/if}
    </div>

    <aside class="template-detail" aria-label={t("templates-detail-label")}>
      {#if detailMode === "create"}
        <form class="template-form" onsubmit={submitCreate}>
          <div class="detail-heading">
            <div>
              <span class="detail-kicker">{duplicateSourcePath ? t("templates-new-tera-resource") : roleLabel(createRole)}</span>
              <h2>{duplicateSourcePath
                ? t("templates-duplicate-resource")
                : t("templates-add-to", { category: views.find((view) => view.id === activeView)?.label ?? "" })}</h2>
              <p>{t("templates-create-description")}</p>
            </div>
            <button class="ui-icon-button ui-close-button" type="button" aria-label={t("templates-cancel")} disabled={busy} onclick={resetPanelState}>
              <IconX size={14} />
            </button>
          </div>

          <div class="form-fields">
            {#if createRole === "listing_item" && !duplicateSourcePath}
              <label>
                <span>{t("templates-listing-name")}</span>
                <input
                  bind:this={draftNameInput}
                  value={listingLabel}
                  type="text"
                  autocomplete="off"
                  placeholder={t("templates-listing-name-placeholder")}
                  disabled={busy}
                  oninput={(event) => updateListingLabel(event.currentTarget.value)}
                />
              </label>
              <label>
                <span>{t("templates-listing-id")}</span>
                <input
                  value={createName}
                  type="text"
                  autocomplete="off"
                  placeholder={t("templates-listing-id-placeholder")}
                  disabled={busy}
                  oninput={(event) => {
                    createNameTouched = true;
                    createName = collectionSlug(event.currentTarget.value);
                  }}
                />
                <small>{t("templates-listing-result-file")} <code>templates/listing-items/{createName || "…"}.html</code></small>
              </label>
              <label>
                <span>{t("templates-listing-model")}</span>
                <select value={listingModelId} disabled={busy} onchange={(event) => changeListingModel(event.currentTarget.value)}>
                  {#each listingModels as model (model.id)}
                    <option value={model.id}>{model.label} · {model.id}</option>
                  {/each}
                </select>
                <small>{t("templates-listing-model-help")}</small>
              </label>
              <label>
                <span>{t("templates-listing-preview-page")}</span>
                <select bind:value={listingPreviewPageFile} disabled={busy || listingPreviewPages.length === 0}>
                  {#each listingPreviewPages as page (page.file)}
                    <option value={page.file}>{page.title} · {page.url}</option>
                  {/each}
                </select>
                <small>{t("templates-listing-preview-help")}</small>
              </label>
              {#if listingModels.length === 0}
                <p class="guard-message">{t("templates-listing-model-missing")}</p>
              {:else if listingPreviewPages.length === 0}
                <p class="guard-message">{t("templates-listing-preview-missing")}</p>
              {/if}
            {:else}
            {#if !duplicateSourcePath && createRoles[activeView].length > 1}
              <label>
                <span>{t("templates-field-role")}</span>
                <select value={createRole} disabled={busy} onchange={(event) => changeCreateRole(event.currentTarget.value as TemplateSemanticCreateRole)}>
                  {#each createRoles[activeView] as role}
                    <option value={role}>{roleLabel(role)}</option>
                  {/each}
                </select>
              </label>
            {/if}

            {#if !duplicateSourcePath && createRole === "section_archive"}
              <label>
                <span>{t("templates-field-section-target")}</span>
                <select value={createTargetId} disabled={busy} onchange={(event) => changeCreateTarget(event.currentTarget.value)}>
                  <option value={NEW_SECTION_TARGET}>{t("templates-create-new-section")}</option>
                  {#each createTargets as target (target.id)}
                    <option value={target.id}>{target.label} · {target.url ?? target.file}</option>
                  {/each}
                </select>
                <small>{creatingNewArchiveSection
                  ? t("templates-new-section-atomic-help")
                  : t("templates-existing-section-help")}</small>
              </label>
            {:else if !duplicateSourcePath && createTargets.length > 0}
              <label>
                <span>{t("templates-field-exact-target")}</span>
                <select value={createTargetId} disabled={busy} onchange={(event) => changeCreateTarget(event.currentTarget.value)}>
                  {#each createTargets as target (target.id)}
                    <option value={target.id}>{target.label} · {target.url ?? target.file}</option>
                  {/each}
                </select>
              </label>
            {/if}

            {#if !duplicateSourcePath && creatingNewArchiveSection}
              <label>
                <span>{t("templates-field-section-title")}</span>
                <input
                  bind:this={draftSectionTitleInput}
                  value={createSectionTitle}
                  type="text"
                  autocomplete="off"
                  placeholder={t("templates-section-title-placeholder")}
                  disabled={busy}
                  oninput={(event) => updateSectionTitle(event.currentTarget.value)}
                />
              </label>
              <label>
                <span>{t("templates-field-section-slug")}</span>
                <input
                  value={createSectionSlug}
                  type="text"
                  autocomplete="off"
                  placeholder={t("templates-section-slug-placeholder")}
                  disabled={busy}
                  oninput={(event) => updateSectionSlug(event.currentTarget.value)}
                />
                <small>{t("templates-resulting-section-path")} <code>content/{createSectionSlug || "…"}/_index.md</code></small>
              </label>
              <label>
                <span>{t("templates-field-section-sort")}</span>
                <select bind:value={createSectionSort} disabled={busy}>
                  <option value="weight">{t("templates-section-sort-weight")}</option>
                  <option value="date">{t("templates-section-sort-date")}</option>
                  <option value="title">{t("templates-section-sort-title")}</option>
                  <option value="none">{t("templates-section-sort-none")}</option>
                </select>
              </label>
            {/if}

            <label>
              <span>{t("templates-field-logical-name")}</span>
              <input
                bind:this={draftNameInput}
                bind:value={createName}
                type="text"
                autocomplete="off"
                disabled={busy}
                oninput={() => { createNameTouched = true; }}
              />
              <small>{t("templates-resulting-path")} <code>templates/{createName || "…"}.html</code></small>
            </label>

            {#if !duplicateSourcePath && createRole !== "not_found"}
              <label>
                <span>{t("templates-field-parent-layout")}</span>
                <select bind:value={createParent} disabled={busy || createRole === "layout"}>
                  <option value="">{t("templates-no-parent-layout")}</option>
                  {#each layoutOptions as layout (layout.id)}
                    <option value={layout.name}>{layout.name} · {originLabel(layout)}</option>
                  {/each}
                </select>
                <small>{t("templates-parent-help")}</small>
              </label>
            {:else if duplicateSourcePath}
              <div class="source-summary"><span>{t("templates-copied-source")}</span><code>{duplicateSourcePath}</code></div>
            {/if}

            {#if createRole === "section_element" && !duplicateSourcePath}
              <label class="check-field">
                <input bind:checked={includePageContent} type="checkbox" disabled={busy} />
                <span>{t("templates-include-page-content")}</span>
              </label>
            {/if}
            {/if}
          </div>

          {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button type="button" disabled={busy} onclick={resetPanelState}>{t("templates-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={busy}>
              <IconPlus size={14} /> {busy ? t("templates-validating") : t("templates-create-session")}
            </button>
          </div>
        </form>
      {:else if detailMode === "rename" && selectedResource?.editable}
        <form class="template-form" onsubmit={(event) => submitRename(event, selectedResource)}>
          <div class="detail-heading">
            <div>
              <span class="detail-kicker">{t("templates-local-identity")}</span>
              <h2>{t("templates-rename-title", { name: selectedResource.name })}</h2>
              <p>{t("templates-rename-description")}</p>
            </div>
            <button class="ui-icon-button ui-close-button" type="button" aria-label={t("templates-cancel")} disabled={busy} onclick={resetPanelState}><IconX size={14} /></button>
          </div>
          <div class="form-fields">
            <label><span>{t("templates-field-logical-name")}</span><input bind:this={draftNameInput} bind:value={createName} type="text" disabled={busy} /></label>
          </div>
          {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button type="button" disabled={busy} onclick={resetPanelState}>{t("templates-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={busy}><IconDeviceFloppy size={14} /> {t("templates-save")}</button>
          </div>
        </form>
      {:else if selectedEntry}
        <div class="detail-heading">
          <div>
            <span class="detail-kicker">{roleLabel(selectedEntry.role)}</span>
            <h2>{entryLabel(selectedEntry)}</h2>
            <code>{selectedEntry.target.url ?? selectedEntry.target.file ?? targetLabel(selectedEntry.target)}</code>
          </div>
          {#if selectedResource}
            <button
              type="button"
              disabled={busy}
              title={diagnosticText(selectedEntry.previewContext?.unavailableDiagnostic) || t("templates-open-editor")}
              onclick={() => { void openResource(selectedEntry, selectedResource); }}
            ><IconExternalLink size={14} /> {selectedEntry.previewContext?.available ? t("templates-edit-visual") : t("templates-open-source")}</button>
          {/if}
        </div>

        <dl class="contract-grid">
          <div><dt>{t("templates-effective-resource")}</dt><dd>{selectedEntry.assignment.resourceName}</dd></div>
          <div><dt>{t("templates-provenance")}</dt><dd>{assignmentSourceLabel(selectedEntry.assignment.source)}</dd></div>
          <div><dt>{t("templates-zola-key")}</dt><dd>{selectedEntry.assignment.key ?? "—"}</dd></div>
          <div><dt>{t("templates-affected-pages")}</dt><dd>{l10n.formatNumber(selectedEntry.affectedPages.length)}</dd></div>
        </dl>

        <section class="semantic-contract">
          <div><span>{t("templates-target")}</span><strong>{targetLabel(selectedEntry.target)}</strong><code>{selectedEntry.target.file ?? selectedEntry.target.url ?? t("templates-tera-resource")}</code></div>
          <div>
            <span>{t("templates-preview-context")}</span>
            {#if selectedEntry.previewContext}
              <strong>{previewTitle(selectedEntry.previewContext)}</strong>
              <code>{selectedEntry.previewContext.url}</code>
              {#if !selectedEntry.previewContext.available}
                <small>{diagnosticText(selectedEntry.previewContext.unavailableDiagnostic)}</small>
              {/if}
            {:else}
              <strong>{t("templates-no-real-consumer")}</strong>
            {/if}
          </div>
        </section>

        {#if canAssignSelected}
          <section class="control-section">
            <div>
              <h3>{t("templates-assignment")}</h3>
              <p>{t("templates-assignment-description", {
                key: selectedEntry.assignment.key ?? "",
                file: selectedEntry.target.file ?? "",
              })}</p>
            </div>
            <label>
              <span>{t("templates-resource")}</span>
              <select bind:value={assignmentDraft} disabled={busy}>
                {#each assignableResources as resource (resource.id)}
                  <option value={resource.name}>{resource.name} · {originLabel(resource)}</option>
                {/each}
              </select>
            </label>
            <button type="button" disabled={busy || assignmentDraft === selectedEntry.assignment.resourceName} onclick={() => { void saveAssignment(selectedEntry); }}>
              <IconDeviceFloppy size={14} /> {t("templates-apply")}
            </button>
            {#if selectedEntry.assignment.source === "explicit"}
              <button type="button" disabled={busy} onclick={() => { void clearAssignment(selectedEntry); }}>{t("templates-return-to-inheritance")}</button>
            {/if}
          </section>
        {/if}

        {#if selectedResource}
          <section class="resource-section">
            <div class="resource-heading">
              <span class="resource-icon"><IconFileCode size={17} /></span>
              <span><small>{t("templates-tera-resource")}</small><strong>{selectedResource.name}</strong><code>{selectedResource.file}</code></span>
              <em class:theme={!selectedResource.editable}>{originLabel(selectedResource)}</em>
            </div>
            <dl class="contract-grid compact">
              <div><dt>{t("templates-field-parent-layout")}</dt><dd>{selectedResource.extends ?? t("templates-none")}</dd></div>
              <div><dt>{t("templates-blocks")}</dt><dd>{l10n.formatNumber(selectedResource.blocks.length)}</dd></div>
              <div><dt>{t("templates-includes-imports")}</dt><dd>{l10n.formatNumber(selectedResource.includes.length + selectedResource.imports.length)}</dd></div>
              <div><dt>{t("templates-uses")}</dt><dd>{l10n.formatNumber(selectedResource.usedByTemplates.length)}</dd></div>
            </dl>

            {#if selectedResource.editable && selectedEntry.role !== "listing_item"}
              <div class="parent-control">
                <label>
                  <span>{t("templates-field-parent-layout")}</span>
                  <select bind:value={parentDraft} disabled={busy}>
                    <option value="">{t("templates-no-parent-layout")}</option>
                    {#each layoutOptions.filter((layout) => layout.id !== selectedResource?.id) as layout (layout.id)}
                      <option value={layout.name}>{layout.name} · {originLabel(layout)}</option>
                    {/each}
                  </select>
                </label>
                <button type="button" disabled={busy || parentDraft === (selectedResource.extends ?? "")} onclick={() => { void saveParent(selectedResource); }}>
                  <IconDeviceFloppy size={14} /> {t("templates-apply")}
                </button>
              </div>
            {/if}
          </section>

          <div class="detail-actions">
            <button class="ui-button primary" type="button" disabled={busy} onclick={() => { void openResource(selectedEntry, selectedResource); }}>
              <IconExternalLink size={14} /> {t("templates-open")}
            </button>
            {#if selectedResource.editable}
              {#if selectedEntry.role !== "listing_item"}
              <button type="button" disabled={busy} onclick={() => beginRename(selectedResource)}><IconEdit size={14} /> {t("templates-rename")}</button>
              <button type="button" disabled={busy} onclick={() => beginDuplicate(selectedResource)}><IconCopy size={14} /> {t("templates-duplicate")}</button>
              {/if}
              <button
                class="ui-button danger"
                type="button"
                disabled={busy || !selectedResource.canDelete}
                title={diagnosticText(selectedResource.deleteBlockedDiagnostic) || t("templates-delete-title")}
                onclick={() => { deleteConfirmationOpen = true; }}
              ><IconTrash size={14} /> {t("templates-delete")}</button>
            {:else}
              <button type="button" disabled={busy} onclick={() => { void overrideResource(selectedResource); }}><IconCopy size={14} /> {t("templates-local-override")}</button>
            {/if}
          </div>

          {#if deleteConfirmationOpen && selectedResource.editable}
            <section class="delete-confirmation">
              <div><strong>{t("templates-delete-question", { name: selectedResource.name })}</strong><span>{t("templates-delete-description")}</span></div>
              <div>
                <button type="button" onclick={() => { deleteConfirmationOpen = false; }}>{t("templates-cancel")}</button>
                <button class="ui-button danger" type="button" disabled={busy} onclick={() => { void removeResource(selectedEntry, selectedResource); }}><IconTrash size={14} /> {t("templates-confirm")}</button>
              </div>
            </section>
          {/if}
          {#if selectedResource.deleteBlockedDiagnostic && selectedResource.editable}
            <p class="guard-message">{diagnosticText(selectedResource.deleteBlockedDiagnostic)}</p>
          {/if}
        {:else}
          <section class="missing-resource">
            <IconAlertTriangle size={18} />
            <div><strong>{t("templates-missing-resource-title")}</strong><p>{t("templates-missing-resource-description")}</p></div>
            <button type="button" onclick={() => beginCreateForEntry(selectedEntry)}><IconPlus size={14} /> {t("templates-create")}</button>
          </section>
        {/if}

        {#if formError}<p class="form-error detail-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
      {:else}
        <div class="workspace-state">{t("templates-select-role")}</div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .detail-kicker { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 11px; font-weight: 720; letter-spacing: .04em; text-transform: uppercase; }
  .view-tabs button span { min-width: 17px; padding: 1px 4px; border-radius: 9px; background: var(--wb-surface-document); font-size: 11px; text-align: center; }
  .workspace-body { display: grid; grid-template-columns: minmax(420px, 58%) minmax(330px, 42%); min-width: 0; min-height: 0; }
  .template-list, .template-detail { min-width: 0; min-height: 0; overflow: auto; scrollbar-gutter: stable; }
  .template-list { border-right: 1px solid var(--wb-border-subtle); }
  .template-detail { padding: 17px; background: var(--wb-surface-chrome); }
  .template-card { display: grid; grid-template-columns: 32px minmax(0, 1fr) minmax(130px, .55fr) auto; align-items: center; gap: 8px; width: calc(100% - 16px); margin: 6px 8px 0; padding: 8px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .resource-icon { display: grid; width: 30px; height: 30px; place-items: center; border-radius: 7px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .card-copy, .technical-copy { display: grid; min-width: 0; gap: 2px; }
  .card-copy strong, .resource-heading strong { overflow: hidden; color: var(--text-strong); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .card-copy small, .technical-copy small, .resource-heading small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .technical-copy em { overflow: hidden; color: var(--wb-accent-strong); font-size: 11px; font-style: normal; text-overflow: ellipsis; white-space: nowrap; }
  .origin { padding: 3px 5px; border-radius: 4px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 11px; font-style: normal; }
  .origin.theme, .resource-heading em.theme { color: var(--wb-text-muted); background: var(--wb-surface-document); }
  .origin.missing { color: var(--danger); background: color-mix(in srgb, var(--danger) 8%, transparent); }
  .detail-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  .detail-heading > div { min-width: 0; }
  .detail-heading h2 { margin: 5px 0 4px; color: var(--text-strong); font-size: 20px; line-height: 1.15; }
  .detail-heading p { margin: 0; color: var(--wb-text-muted); font-size: 11px; line-height: 1.45; }
  .detail-heading code { display: block; overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .detail-heading button, .detail-actions button, .control-section button, .parent-control button, .missing-resource button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-height: 29px; padding: 0 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 11px; white-space: nowrap; }
  .contract-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; margin: 14px 0 0; }
  .contract-grid div { min-width: 0; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .contract-grid dt { color: var(--wb-text-muted); font-size: 11px; font-weight: 700; text-transform: uppercase; }
  .contract-grid dd { overflow: hidden; margin: 3px 0 0; color: var(--text-strong); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .contract-grid.compact { margin-top: 8px; }
  .semantic-contract { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; margin-top: 7px; }
  .semantic-contract > div { display: grid; align-content: start; min-width: 0; gap: 3px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .semantic-contract span { color: var(--wb-accent-strong); font-size: 11px; font-weight: 700; text-transform: uppercase; }
  .semantic-contract strong { color: var(--text-strong); font-size: 12px; }
  .semantic-contract code, .semantic-contract small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; text-overflow: ellipsis; }
  .control-section, .resource-section { display: grid; gap: 8px; margin-top: 13px; padding: 10px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .control-section { grid-template-columns: minmax(0, 1fr) auto auto; }
  .control-section > div { grid-column: 1 / -1; }
  h3 { margin: 0 0 3px; color: var(--text-strong); font-size: 11px; text-transform: uppercase; }
  .control-section p { margin: 0; color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  .control-section label, .parent-control label { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: 7px; min-width: 0; }
  .control-section label > span, .parent-control label > span { color: var(--wb-text-muted); font-size: 11px; font-weight: 700; text-transform: uppercase; }
  select { min-width: 0; height: 29px; padding: 0 7px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; }
  .resource-heading { display: grid; grid-template-columns: 32px minmax(0, 1fr) auto; align-items: center; gap: 8px; }
  .resource-heading > span:nth-child(2) { display: grid; min-width: 0; gap: 2px; }
  .resource-heading code { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .resource-heading em { padding: 3px 5px; border-radius: 4px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 11px; font-style: normal; }
  .parent-control { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 7px; margin-top: 2px; }
  .detail-actions, .form-actions, .delete-confirmation > div:last-child { display: flex; align-items: center; gap: 6px; margin-top: 12px; }
  .detail-actions .primary, .form-actions .primary { color: #fff; border-color: var(--wb-accent); background: var(--wb-accent); }
  .detail-actions .danger { margin-left: auto; color: var(--danger); }
  .template-form { display: grid; align-content: start; gap: 16px; }
  .form-fields { display: grid; gap: 12px; }
  .form-fields label { display: grid; gap: 5px; }
  .form-fields label > span, .source-summary > span { color: var(--wb-text-muted); font-size: 11px; font-weight: 650; text-transform: uppercase; }
  .form-fields input, .form-fields select { width: 100%; min-height: 32px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .form-fields small { color: var(--wb-text-muted); font-size: 11px; line-height: 1.45; }
  .form-fields .check-field { display: flex; align-items: center; gap: 7px; }
  .form-fields .check-field input { width: auto; min-height: auto; }
  .form-fields .check-field span { color: var(--wb-text-primary); font-weight: 500; text-transform: none; }
  .source-summary { display: grid; gap: 5px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); background: var(--wb-surface-document); }
  .source-summary code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .form-actions { justify-content: flex-end; }
  .form-actions button, .delete-confirmation button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-height: 29px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .form-error { display: flex; align-items: center; gap: 6px; margin: 0; padding: 8px; border-left: 3px solid var(--danger); color: var(--danger); background: var(--wb-surface-document); font-size: 11px; }
  .detail-error { margin-top: 12px; }
  .missing-resource { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 9px; margin-top: 13px; padding: 10px; border: 1px solid color-mix(in srgb, var(--danger) 35%, var(--wb-border-subtle)); border-radius: 7px; background: var(--wb-surface-document); }
  .missing-resource strong { color: var(--text-strong); font-size: 12px; }
  .missing-resource p { margin: 2px 0 0; color: var(--wb-text-muted); font-size: 11px; }
  .delete-confirmation { display: grid; gap: 8px; margin-top: 10px; padding: 10px; border: 1px solid color-mix(in srgb, var(--danger) 42%, var(--wb-border-subtle)); border-radius: 7px; background: var(--wb-surface-document); }
  .delete-confirmation > div:first-child { display: grid; gap: 3px; }
  .delete-confirmation strong { color: var(--text-strong); font-size: 12px; }
  .delete-confirmation span, .guard-message { color: var(--wb-text-muted); font-size: 11px; }
  .delete-confirmation > div:last-child { justify-content: flex-end; margin: 0; }
  .delete-confirmation .danger { color: var(--danger); }
  .guard-message { margin: 8px 0 0; padding: 7px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .workspace-state { display: grid; min-height: 180px; place-items: center; gap: 7px; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .workspace-state.error { color: var(--danger); }
  .spin { animation: spin 1s linear infinite; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { opacity: .45; }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 1050px) { .workspace-toolbar { grid-template-columns: minmax(0, 1fr) 190px auto; } .technical-copy { display: none; } .template-card { grid-template-columns: 32px minmax(0, 1fr) auto; } }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .template-detail { display: none; } .template-list { border-right: 0; } .workspace-header dl { display: none; } }
</style>
