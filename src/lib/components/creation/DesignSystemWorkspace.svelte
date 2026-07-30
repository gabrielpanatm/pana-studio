<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    IconAlertTriangle,
    IconBrandGoogle,
    IconCircleCheck,
    IconDeviceFloppy,
    IconDownload,
    IconEdit,
    IconExternalLink,
    IconFileTypeCss,
    IconFolderOpen,
    IconPalette,
    IconPlus,
    IconSearch,
    IconTags,
    IconTrash,
    IconTypography,
    IconX,
  } from "@tabler/icons-svelte";
  import {
    createProjectTextFile,
    applyLocalFontImport,
    assignFontRole,
    chooseFontFiles,
    downloadGoogleFontFamily,
    getFontManager,
    getFontPreviewAsset,
    planFontFamilyRemoval,
    planLocalFontImport,
    readDesignTokenCatalog,
    readThemeStyleCatalog,
    searchGoogleFonts,
    removeFontFamily,
    setFontDisplay,
    setFontPreload,
  } from "$lib/project/io";
  import DesignTokenCatalog from "./DesignTokenCatalog.svelte";
  import ThemeStylesWorkspace from "./ThemeStylesWorkspace.svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import {
    settleProjectWorkspaceMutation,
    workspaceMutationAuthorityReceipt,
  } from "$lib/session/workspace-mutation-coordinator";
  import type {
    DesignTokenCatalogSnapshot,
    DesignTokenSnapshot,
    FileBufferRequestIdentity,
    FontInventory,
    FontDeliveryDiagnostic,
    FontFamilyRemovalPlan,
    FontRoleAssignment,
    FontRoleId,
    GoogleFontCatalogFamily,
    LocalFontImportPlan,
    ProjectWorkspaceIdentity,
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
  type FontCreateSource = "google" | "local";

  let activeView = $state<DesignView>("global-styles");
  let category = $state("all");
  let styleCategory = $state("all");
  let query = $state("");
  let selectedTokenKey = $state("");
  let selectedStyleId = $state("");
  let selectedClassName = $state("");
  let selectedFontKey = $state("");
  let detailMode = $state<DetailMode>("info");
  let fontInventory = $state<FontInventory | null>(null);
  let fontRoles = $state<FontRoleAssignment[]>([]);
  let fontDiagnostics = $state<FontDeliveryDiagnostic[]>([]);
  let fontRemovalPlan = $state<FontFamilyRemovalPlan | null>(null);
  let fontRemovalPlanning = $state(false);
  let fontError = $state("");
  let fontPreviewError = $state("");
  let fontPreviewLoading = $state(false);
  let fontPreviewSequence = 0;
  let fontPreviewStyle: HTMLStyleElement | null = null;
  let fontManagerLoadSequence = 0;
  let fontManagerLoadedIdentityKey = "";
  let formName = $state("");
  let formValue = $state("");
  let formPath = $state("");
  let formWeights = $state("400, 700");
  let formGoogleStyles = $state<string[]>(["normal"]);
  let formVariableFont = $state(false);
  let formGoogleAxes = $state<string[]>([]);
  let formGoogleCharacterSet = $state("");
  let googleFontQuery = $state("");
  let googleFontResults = $state<GoogleFontCatalogFamily[]>([]);
  let googleFontLoading = $state(false);
  let googleFontError = $state("");
  let googleFontSearchSequence = 0;
  let fontCreateSource = $state<FontCreateSource>("google");
  let localFontPaths = $state<string[]>([]);
  let localFontPlan = $state<LocalFontImportPlan | null>(null);
  let localFontPlanning = $state(false);
  let localFontPlanSequence = 0;
  let formError = $state("");
  let mutating = $state(false);
  let designTokenCatalog = $state<DesignTokenCatalogSnapshot | null>(null);
  let designTokenLoading = $state(false);
  let designTokenError = $state("");
  let designTokenLoadSequence = 0;
  let designTokenLoadedIdentityKey = "";
  let themeStyleCatalog = $state<ThemeStyleCatalogSnapshot | null>(null);
  let themeStyleLoading = $state(false);
  let themeStyleError = $state("");
  let themeStyleLoadSequence = 0;
  let themeStyleLoadedIdentityKey = "";

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const filteredTokens = $derived(
    (designTokenCatalog?.tokens ?? []).filter((token) => {
      return (category === "all" || token.categoryId === category)
        && (!normalizedQuery || `${token.name} ${token.rawValue} ${token.resolvedValue ?? ""} ${token.sourcePath} ${token.groupLabel}`
          .toLocaleLowerCase(l10n.locale)
          .includes(normalizedQuery));
    }),
  );
  const styles = $derived(
    (app.sourceGraph?.styles ?? []).filter((style) => (
      !normalizedQuery
      || `${style.file} ${style.scope}`.toLocaleLowerCase(l10n.locale).includes(normalizedQuery)
    )),
  );
  const classes = $derived(
    (app.designClassInventory?.classes ?? []).filter((entry) => (
      !normalizedQuery
      || `${entry.name} ${entry.files.join(" ")}`.toLocaleLowerCase(l10n.locale).includes(normalizedQuery)
    )),
  );
  const selectedToken = $derived(
    (designTokenCatalog?.tokens ?? []).find((token) => token.id === selectedTokenKey)
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
      || `${family.family} ${family.directory}`.toLocaleLowerCase(l10n.locale).includes(normalizedQuery)
    )),
  );
  const selectedFont = $derived(
    (fontInventory?.families ?? []).find(
      (family) => `${family.origin}:${family.directory}` === selectedFontKey,
    )
      ?? visibleFonts[0]
      ?? null,
  );
  const selectedFontDiagnostics = $derived(
    fontDiagnostics.filter((diagnostic) => (
      diagnostic.family === null || diagnostic.family === selectedFont?.family
    )),
  );
  const selectedGoogleFont = $derived(
    googleFontResults.find((family) => family.family === formName) ?? null,
  );
  const selectedFontPreviewFile = $derived.by(() => {
    if (!selectedFont) return null;
    return [...selectedFont.files].sort((left, right) => {
      const leftScore = (left.extension === "woff2" ? 0 : 20)
        + (left.style === "normal" ? 0 : 5)
        + (left.weight === 400 || (left.weightRange && left.weightRange.start <= 400 && left.weightRange.end >= 400) ? 0 : 2);
      const rightScore = (right.extension === "woff2" ? 0 : 20)
        + (right.style === "normal" ? 0 : 5)
        + (right.weight === 400 || (right.weightRange && right.weightRange.start <= 400 && right.weightRange.end >= 400) ? 0 : 2);
      return leftScore - rightScore;
    })[0] ?? null;
  });
  const formReady = $derived.by(() => {
    if (activeView === "global-styles") return false;
    if (activeView === "styles") return Boolean(formPath.trim());
    if (activeView === "tokens" || activeView === "classes") {
      return Boolean(formName.trim() && formPath.trim());
    }
    if (fontCreateSource === "local") {
      return Boolean(
        localFontPlan
        && localFontPlan.changed
        && localFontPlan.conflicts.length === 0
        && !localFontPlanning
      );
    }
    return Boolean(formName.trim() && formGoogleStyles.length > 0);
  });
  $effect(() => {
    const view = activeView;
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    const identityKey = `${projectRoot}\u0000${runtimeSessionId}\u0000${workspaceRevision}`;
    if (
      view === "global-styles"
      && themeStyleLoadedIdentityKey !== identityKey
    ) {
      void reloadThemeStyleCatalog();
    } else if (
      view === "tokens"
      && designTokenLoadedIdentityKey !== identityKey
    ) {
      void reloadDesignTokenCatalog();
    } else if (view === "classes") {
      void app.refreshDesignClassInventory();
    } else if (
      view === "fonts"
      && fontManagerLoadedIdentityKey !== identityKey
    ) {
      void reloadFontManager();
    }
  });

  $effect(() => {
    const file = selectedFontPreviewFile?.file;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!file || workspaceRevision === undefined) {
      clearFontPreview();
      return;
    }
    void loadSelectedFontPreview(file, workspaceRevision);
  });

  onDestroy(() => clearFontPreview());

  function categoryLabel(token: DesignTokenSnapshot) {
    return designTokenCatalog?.categories.find((entry) => entry.id === token.categoryId)?.label
      ?? t("assets-view-other");
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

  function workspaceIdentity(): ProjectWorkspaceIdentity {
    const snapshot = app.projectWorkspaceSnapshot;
    if (!snapshot) throw new Error(t("design-workspace-not-ready"));
    return {
      expectedProjectRoot: snapshot.projectRoot,
      expectedSessionId: snapshot.runtimeSessionId,
      expectedRevision: snapshot.revision,
    };
  }

  async function reloadDesignTokenCatalog() {
    const requestId = ++designTokenLoadSequence;
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    const identityKey = `${projectRoot}\u0000${runtimeSessionId}\u0000${workspaceRevision}`;
    designTokenLoadedIdentityKey = identityKey;
    designTokenLoading = true;
    designTokenError = "";
    try {
      const snapshot = await readDesignTokenCatalog(identity(), workspaceRevision);
      if (
        requestId !== designTokenLoadSequence
        || app.sessionProjectRoot !== projectRoot
        || app.kernelProjectSessionId !== runtimeSessionId
        || app.projectWorkspaceSnapshot?.revision !== workspaceRevision
      ) return;
      designTokenCatalog = snapshot;
      if (
        category !== "all"
        && !snapshot.categories.some((entry) => entry.id === category)
      ) category = "all";
      if (
        selectedTokenKey
        && !snapshot.tokens.some((token) => token.id === selectedTokenKey)
      ) selectedTokenKey = "";
    } catch (cause) {
      if (requestId !== designTokenLoadSequence) return;
      if (designTokenLoadedIdentityKey === identityKey) {
        designTokenLoadedIdentityKey = "";
      }
      designTokenError = errorMessage(cause);
    } finally {
      if (requestId === designTokenLoadSequence) designTokenLoading = false;
    }
  }

  async function reloadThemeStyleCatalog() {
    const requestId = ++themeStyleLoadSequence;
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    const identityKey = `${projectRoot}\u0000${runtimeSessionId}\u0000${workspaceRevision}`;
    themeStyleLoadedIdentityKey = identityKey;
    themeStyleLoading = true;
    themeStyleError = "";
    try {
      const snapshot = await readThemeStyleCatalog(identity(), workspaceRevision);
      if (
        requestId !== themeStyleLoadSequence
        || app.sessionProjectRoot !== projectRoot
        || app.kernelProjectSessionId !== runtimeSessionId
        || app.projectWorkspaceSnapshot?.revision !== workspaceRevision
      ) return;
      themeStyleCatalog = snapshot;
      if (
        styleCategory !== "all"
        && !snapshot.categories.some((entry) => entry.id === styleCategory)
      ) styleCategory = "all";
    } catch (cause) {
      if (requestId !== themeStyleLoadSequence) return;
      if (themeStyleLoadedIdentityKey === identityKey) {
        themeStyleLoadedIdentityKey = "";
      }
      themeStyleError = errorMessage(cause);
    } finally {
      if (requestId === themeStyleLoadSequence) themeStyleLoading = false;
    }
  }

  async function reloadFontManager() {
    const requestId = ++fontManagerLoadSequence;
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    const identityKey = `${projectRoot}\u0000${runtimeSessionId}\u0000${workspaceRevision}`;
    fontManagerLoadedIdentityKey = identityKey;
    fontError = "";
    try {
      const manager = await getFontManager(workspaceIdentity());
      if (
        requestId !== fontManagerLoadSequence
        || app.sessionProjectRoot !== projectRoot
        || app.kernelProjectSessionId !== runtimeSessionId
        || app.projectWorkspaceSnapshot?.revision !== workspaceRevision
      ) return;
      fontInventory = manager.inventory;
      fontRoles = manager.roles;
      fontDiagnostics = manager.diagnostics;
    } catch (cause) {
      if (requestId !== fontManagerLoadSequence) return;
      if (fontManagerLoadedIdentityKey === identityKey) {
        fontManagerLoadedIdentityKey = "";
      }
      fontError = errorMessage(cause);
    }
  }

  function clearFontPreview() {
    fontPreviewSequence += 1;
    fontPreviewLoading = false;
    fontPreviewError = "";
    fontPreviewStyle?.remove();
    fontPreviewStyle = null;
  }

  async function loadSelectedFontPreview(file: string, workspaceRevision: number) {
    const requestId = ++fontPreviewSequence;
    fontPreviewLoading = true;
    fontPreviewError = "";
    fontPreviewStyle?.remove();
    fontPreviewStyle = null;
    try {
      const asset = await getFontPreviewAsset(file, workspaceIdentity());
      if (
        requestId !== fontPreviewSequence
        || app.projectWorkspaceSnapshot?.revision !== workspaceRevision
        || selectedFontPreviewFile?.file !== file
      ) return;
      const style = document.createElement("style");
      style.dataset.panaFontPreview = asset.contentHash;
      style.textContent = `@font-face { font-family: "Pana Studio Font Preview"; src: url("${asset.dataUrl}") format("${asset.format}"); font-weight: 100 900; font-style: normal; font-display: swap; }`;
      document.head.append(style);
      fontPreviewStyle = style;
    } catch (cause) {
      if (requestId !== fontPreviewSequence) return;
      fontPreviewError = errorMessage(cause);
      fontPreviewStyle?.remove();
      fontPreviewStyle = null;
    } finally {
      if (requestId === fontPreviewSequence) fontPreviewLoading = false;
    }
  }

  function resetPanel() {
    detailMode = "info";
    formName = "";
    formValue = "";
    formPath = "";
    formWeights = "400, 700";
    formGoogleStyles = ["normal"];
    formVariableFont = false;
    formGoogleAxes = [];
    formGoogleCharacterSet = "";
    googleFontQuery = "";
    googleFontResults = [];
    googleFontLoading = false;
    googleFontError = "";
    googleFontSearchSequence += 1;
    fontCreateSource = "google";
    localFontPaths = [];
    localFontPlan = null;
    localFontPlanning = false;
    localFontPlanSequence += 1;
    fontRemovalPlan = null;
    fontRemovalPlanning = false;
    formError = "";
  }

  function selectToken(token: DesignTokenSnapshot) {
    selectedTokenKey = token.id;
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
      formPath = selectedToken?.sourcePath ?? defaultStylePath();
    } else if (activeView === "classes") {
      formName = "clasa-noua";
      formPath = selectedClass?.files.find((file) => /\.(?:s?css)$/i.test(file)) ?? defaultStylePath();
    } else if (activeView === "styles") {
      formName = "stil-nou.scss";
      formPath = "sass/pagini/stil-nou.scss";
    } else if (activeView === "fonts") {
      void searchGoogleFontCatalog("");
    }
  }

  async function searchGoogleFontCatalog(search = googleFontQuery) {
    const requestId = ++googleFontSearchSequence;
    googleFontLoading = true;
    googleFontError = "";
    try {
      const results = await searchGoogleFonts(search.trim(), 30, 0);
      if (requestId !== googleFontSearchSequence) return;
      googleFontResults = results;
    } catch (cause) {
      if (requestId !== googleFontSearchSequence) return;
      googleFontResults = [];
      googleFontError = errorMessage(cause);
    } finally {
      if (requestId === googleFontSearchSequence) googleFontLoading = false;
    }
  }

  function selectFontCreateSource(source: FontCreateSource) {
    if (mutating || localFontPlanning || fontCreateSource === source) return;
    fontCreateSource = source;
    formError = "";
    if (source === "google" && googleFontResults.length === 0) {
      void searchGoogleFontCatalog("");
    }
  }

  const fontCreateSources: FontCreateSource[] = ["google", "local"];

  function handleFontSourceKeydown(event: KeyboardEvent, source: FontCreateSource) {
    const index = fontCreateSources.indexOf(source);
    let nextIndex = index;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % fontCreateSources.length;
    else if (event.key === "ArrowLeft") {
      nextIndex = (index - 1 + fontCreateSources.length) % fontCreateSources.length;
    } else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = fontCreateSources.length - 1;
    else return;
    event.preventDefault();
    const next = fontCreateSources[nextIndex];
    if (!next) return;
    selectFontCreateSource(next);
    requestAnimationFrame(() => document.getElementById(`font-source-tab-${next}`)?.focus());
  }

  async function chooseAndPlanLocalFonts() {
    if (mutating || localFontPlanning) return;
    formError = "";
    const paths = await chooseFontFiles();
    if (paths.length === 0) return;
    const requestId = ++localFontPlanSequence;
    localFontPaths = paths;
    localFontPlan = null;
    localFontPlanning = true;
    try {
      const plan = await planLocalFontImport(paths, workspaceIdentity());
      if (requestId !== localFontPlanSequence) return;
      localFontPlan = plan;
    } catch (cause) {
      if (requestId !== localFontPlanSequence) return;
      formError = errorMessage(cause);
    } finally {
      if (requestId === localFontPlanSequence) localFontPlanning = false;
    }
  }

  function selectGoogleFont(font: GoogleFontCatalogFamily) {
    formName = font.family;
    formVariableFont = false;
    formGoogleAxes = [];
    formGoogleCharacterSet = "";
    const preferred = [400, 700].filter((weight) => font.weights.includes(weight));
    const fallback = font.weights.slice(0, 2);
    formWeights = (preferred.length ? preferred : fallback).join(", ");
    formGoogleStyles = availableGoogleStyles(font).includes("normal")
      ? ["normal"]
      : [availableGoogleStyles(font)[0] ?? "normal"];
  }

  function availableGoogleStyles(font: GoogleFontCatalogFamily) {
    const styles: string[] = [];
    if (font.variants.some((variant) => !variant.endsWith("italic"))) styles.push("normal");
    if (font.variants.some((variant) => variant.endsWith("italic") || variant === "italic")) {
      styles.push("italic");
    }
    return styles.length ? styles : ["normal"];
  }

  function toggleGoogleStyle(style: string) {
    const selected = new Set(formGoogleStyles);
    if (selected.has(style)) selected.delete(style);
    else selected.add(style);
    formGoogleStyles = ["normal", "italic"].filter((entry) => selected.has(entry));
  }

  function selectedGoogleWeights() {
    return formWeights
      .split(",")
      .map((weight) => Number.parseInt(weight.trim(), 10))
      .filter((weight) => Number.isInteger(weight));
  }

  function toggleGoogleWeight(weight: number) {
    const selected = new Set(selectedGoogleWeights());
    if (selected.has(weight)) selected.delete(weight);
    else selected.add(weight);
    formWeights = [...selected].sort((left, right) => left - right).join(", ");
  }

  function setVariableFont(enabled: boolean) {
    formVariableFont = enabled;
    if (!enabled) {
      formGoogleAxes = [];
      return;
    }
    if (!selectedGoogleFont) return;
    const axis = selectedGoogleFont.axes.find((entry) => entry.tag === "wght");
    if (axis) formWeights = `${Math.round(axis.start)}, ${Math.round(axis.end)}`;
  }

  function advancedGoogleAxes(font: GoogleFontCatalogFamily) {
    return font.axes.filter((axis) => !["ital", "wght"].includes(axis.tag.toLocaleLowerCase("en")));
  }

  function toggleGoogleAxis(tag: string) {
    const selected = new Set(formGoogleAxes);
    if (selected.has(tag)) selected.delete(tag);
    else selected.add(tag);
    formGoogleAxes = [...selected].sort();
  }

  function beginEdit() {
    resetPanel();
    detailMode = "edit";
    if (activeView === "tokens" && selectedToken) {
      formName = selectedToken.name;
      formValue = selectedToken.rawValue;
      formPath = selectedToken.sourcePath;
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
          await reloadDesignTokenCatalog();
          selectedTokenKey = designTokenCatalog?.tokens.find((token) => (
            token.sourcePath === formPath
            && token.name === formName.replace(/^\$/, "")
          ))?.id ?? "";
        }
      } else if (activeView === "classes") {
        const created = await app.createDesignSystemClass(formName, formPath);
        if (created) selectedClassName = formName.replace(/^\./, "");
      } else if (activeView === "styles") {
        const receipt = await createProjectTextFile(
          formPath,
          `/* ${t("design-new-stylesheet-comment")} */\n`,
          identity(),
        );
        const settlement = await settleProjectWorkspaceMutation(app, receipt, {
          preferredRelativePath: receipt.relativePath,
          warningLabel: t("design-operation-stylesheet-create"),
        });
        selectedStyleId = app.sourceGraph?.styles.find((style) => style.file === receipt.relativePath)?.id ?? "";
        app.setGlobalStatus(
          settlement.warnings.length > 0
            ? t("design-stylesheet-created-warning", { path: formPath })
            : t("design-stylesheet-created-success", { path: formPath }),
          "unsaved",
        );
      } else {
        if (fontCreateSource === "local") {
          if (!localFontPlan || localFontPaths.length === 0) {
            throw new Error(t("design-local-fonts-required"));
          }
          if (localFontPlan.conflicts.length > 0) {
            throw new Error(t("design-local-font-conflicts"));
          }
          const receipt = await applyLocalFontImport(
            localFontPaths,
            localFontPlan.planToken,
            workspaceIdentity(),
          );
          const settlement = await settleProjectWorkspaceMutation(
            app,
            workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
            { warningLabel: t("design-operation-local-font-import") },
          );
          await reloadFontManager();
          const installed = receipt.plan.families[0];
          if (installed) selectedFontKey = `local:${installed.directory}`;
          app.setGlobalStatus(
            settlement.warnings.length > 0
              ? t("design-local-files-warning", { count: receipt.plan.files.length })
              : t("design-local-files-success", { count: receipt.plan.files.length }),
            "unsaved",
          );
          resetPanel();
          return;
        }
        const weights = formWeights
          .split(",")
          .map((weight) => Number.parseInt(weight.trim(), 10))
          .filter((weight) => Number.isInteger(weight) && weight >= 1 && weight <= 1000);
        if (!formName.trim()) throw new Error(t("design-font-family-required"));
        if (!formVariableFont && weights.length === 0) {
          throw new Error(t("design-font-weight-required"));
        }
        const receipt = await downloadGoogleFontFamily(
          formName.trim(),
          weights,
          formGoogleStyles,
          formVariableFont,
          (selectedGoogleFont?.axes ?? []).filter((axis) => formGoogleAxes.includes(axis.tag)),
          formGoogleCharacterSet.trim() ? formGoogleCharacterSet : null,
          workspaceIdentity(),
        );
        const settlement = await settleProjectWorkspaceMutation(
          app,
          workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
          { warningLabel: t("design-operation-google-font-install") },
        );
        await reloadFontManager();
        selectedFontKey = `${receipt.result.family.origin}:${receipt.result.family.directory}`;
        app.setGlobalStatus(
          settlement.warnings.length > 0
            ? t("design-google-font-warning", { family: formName.trim() })
            : t("design-google-font-success", { family: formName.trim() }),
          "unsaved",
        );
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
        const changed = await app.updateDesignSystemVariable({
          name: selectedToken.name,
          value: selectedToken.rawValue,
          file: selectedToken.sourcePath,
        }, formValue);
        if (changed) await reloadDesignTokenCatalog();
      } else if (activeView === "classes" && selectedClass) {
        const changed = await app.renameDesignSystemClass(selectedClass.name, formName);
        if (changed) selectedClassName = formName.replace(/^\./, "");
      } else if (activeView === "styles" && selectedStyle) {
        let explorer = app.fileExplorerSnapshot;
        if (!explorer?.entries.some((entry) => entry.relativePath === selectedStyle.file)) {
          explorer = await app.refreshFileExplorerSnapshot();
        }
        const entry = explorer?.entries.find(
          (candidate) => candidate.relativePath === selectedStyle.file,
        );
        if (!entry) {
          throw new Error(t("project-files-source-gone"));
        }
        const plan = await app.planFileExplorerOperation({
          kind: "rename",
          entryId: entry.id,
          newName: formName,
        });
        if (!plan.allowed) {
          throw new Error(plan.diagnostic ?? t("project-files-rename-invalid"));
        }
        await app.commitFileExplorerOperation(plan);
        const renamedPath = plan.destinationPath ?? selectedStyle.file;
        selectedStyleId = app.sourceGraph?.styles.find((style) => style.file === renamedPath)?.id ?? "";
        app.setGlobalStatus(
          t("design-stylesheet-renamed-success", { path: renamedPath }),
          "unsaved",
        );
      }
      resetPanel();
    } catch (error) {
      formError = errorMessage(error);
    } finally {
      mutating = false;
    }
  }

  async function assignSelectedFontToRole(roleId: FontRoleId) {
    if (mutating || !selectedFont) return;
    formError = "";
    mutating = true;
    try {
      const receipt = await assignFontRole(
        roleId,
        selectedFont.family,
        workspaceIdentity(),
      );
      const settlement = await settleProjectWorkspaceMutation(
        app,
        workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
        { warningLabel: t("design-operation-font-assignment") },
      );
      fontInventory = receipt.manager.inventory;
      fontRoles = receipt.manager.roles;
      fontDiagnostics = receipt.manager.diagnostics;
      app.setGlobalStatus(
        settlement.warnings.length > 0
          ? t("design-font-assigned-warning", {
            family: selectedFont.family,
            role: receipt.role.label,
          })
          : t("design-font-assigned-success", {
            family: selectedFont.family,
            role: receipt.role.label,
          }),
        "unsaved",
      );
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  async function changeSelectedFontDisplay(
    display: "auto" | "block" | "swap" | "fallback" | "optional",
  ) {
    if (mutating || !selectedFont) return;
    formError = "";
    mutating = true;
    try {
      const family = selectedFont.family;
      const receipt = await setFontDisplay(family, display, workspaceIdentity());
      const settlement = await settleProjectWorkspaceMutation(
        app,
        workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
        { warningLabel: t("design-operation-font-display") },
      );
      fontInventory = receipt.manager.inventory;
      fontRoles = receipt.manager.roles;
      fontDiagnostics = receipt.manager.diagnostics;
      app.setGlobalStatus(
        settlement.warnings.length > 0
          ? t("design-font-display-warning", { family, display })
          : t("design-font-display-success", { family, display }),
        "unsaved",
      );
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  async function toggleFontPreload(file: string, enabled: boolean) {
    if (mutating) return;
    formError = "";
    mutating = true;
    try {
      const receipt = await setFontPreload(file, enabled, workspaceIdentity());
      const settlement = await settleProjectWorkspaceMutation(
        app,
        workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
        { warningLabel: t("design-operation-font-preload") },
      );
      fontInventory = receipt.manager.inventory;
      fontRoles = receipt.manager.roles;
      fontDiagnostics = receipt.manager.diagnostics;
      const state = enabled ? t("design-preload-enabled") : t("design-preload-disabled");
      app.setGlobalStatus(
        settlement.warnings.length > 0
          ? t("design-preload-warning", { state, file: file.split("/").at(-1) ?? file })
          : t("design-preload-success", { state, file: file.split("/").at(-1) ?? file }),
        "unsaved",
      );
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  async function planSelectedFontRemoval() {
    if (mutating || fontRemovalPlanning || !selectedFont) return;
    const selectedKey = `${selectedFont.origin}:${selectedFont.directory}`;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (workspaceRevision === undefined) return;
    formError = "";
    fontRemovalPlan = null;
    fontRemovalPlanning = true;
    try {
      const plan = await planFontFamilyRemoval(
        selectedFont.family,
        selectedFont.directory,
        workspaceIdentity(),
      );
      if (
        `${selectedFont?.origin}:${selectedFont?.directory}` !== selectedKey
        || app.projectWorkspaceSnapshot?.revision !== workspaceRevision
      ) return;
      fontRemovalPlan = plan;
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      fontRemovalPlanning = false;
    }
  }

  async function confirmSelectedFontRemoval() {
    if (mutating || !selectedFont || !fontRemovalPlan) return;
    formError = "";
    mutating = true;
    try {
      const family = selectedFont.family;
      const receipt = await removeFontFamily(
        fontRemovalPlan.family,
        fontRemovalPlan.directory,
        fontRemovalPlan.planToken,
        workspaceIdentity(),
      );
      const settlement = await settleProjectWorkspaceMutation(
        app,
        workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
        { warningLabel: t("design-operation-font-remove") },
      );
      fontInventory = receipt.manager.inventory;
      fontRoles = receipt.manager.roles;
      fontDiagnostics = receipt.manager.diagnostics;
      selectedFontKey = "";
      fontRemovalPlan = null;
      clearFontPreview();
      app.setGlobalStatus(
        settlement.warnings.length > 0
          ? t("design-font-removed-warning", { family })
          : t("design-font-removed-success", { family }),
        "unsaved",
      );
    } catch (cause) {
      formError = errorMessage(cause);
    } finally {
      mutating = false;
    }
  }

  const designViews = $derived([
    { id: "global-styles" as const, label: t("design-view-styles") },
    { id: "tokens" as const, label: t("design-view-tokens") },
    { id: "classes" as const, label: t("design-view-classes") },
    { id: "styles" as const, label: t("design-view-stylesheets") },
    { id: "fonts" as const, label: t("design-view-fonts") },
  ]);

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

<section class="activity-workspace design-workspace" aria-labelledby="design-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconPalette size={15} stroke={1.9} /> {t("design-eyebrow")}</span>
      <h1 id="design-title">{t("design-title")}</h1>
      <p>{t("design-description")}</p>
    </div>
    <dl>
      <div><dt>{t("design-view-styles")}</dt><dd>{l10n.formatNumber(themeStyleCatalog?.targets.length ?? 0)}</dd></div>
      <div><dt>{t("design-view-tokens")}</dt><dd>{l10n.formatNumber(designTokenCatalog?.tokens.length ?? 0)}</dd></div>
      <div><dt>{t("design-view-classes")}</dt><dd>{l10n.formatNumber(app.designClassInventory?.classes.length ?? 0)}</dd></div>
      <div><dt>{t("design-view-stylesheets")}</dt><dd>{l10n.formatNumber(app.sourceGraph?.styles.length ?? 0)}</dd></div>
      <div><dt>{t("design-view-fonts")}</dt><dd>{l10n.formatNumber(fontInventory?.families.length ?? 0)}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label={t("design-areas-label")}>
      {#each designViews as view, index (view.id)}
        <button
          id={`design-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          aria-controls={`design-panel-${view.id}`}
          tabindex={activeView === view.id ? 0 : -1}
          class="ui-tab"
          class:active={activeView === view.id}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}</button>
      {/each}
    </div>
    <div
      class="toolbar-query-group"
      class:with-filter={activeView === "global-styles" || activeView === "tokens"}
    >
      {#if activeView === "global-styles"}
        <label class="toolbar-filter">
          <span class="sr-only">{t("design-style-category")}</span>
          <select
            class="ui-field toolbar"
            bind:value={styleCategory}
            aria-label={t("design-style-category")}
          >
            <option value="all">{t("design-all-categories")}</option>
            {#each themeStyleCatalog?.categories ?? [] as entry (entry.id)}
              <option value={entry.id}>{entry.label} ({entry.targetCount})</option>
            {/each}
          </select>
        </label>
      {:else if activeView === "tokens"}
        <label class="toolbar-filter">
          <span class="sr-only">{t("design-token-category")}</span>
          <select
            class="ui-field toolbar"
            bind:value={category}
            aria-label={t("design-token-category")}
          >
            <option value="all">{t("design-all-categories")}</option>
            {#each designTokenCatalog?.categories ?? [] as entry (entry.id)}
              <option value={entry.id}>{entry.label} ({entry.tokenCount})</option>
            {/each}
          </select>
        </label>
      {/if}
      <label class="search-field">
        <span class="sr-only">{t("design-search-label")}</span>
        <IconSearch size={14} stroke={1.9} />
        <input
          class="ui-field toolbar"
          bind:value={query}
          type="search"
          placeholder={activeView === "global-styles"
            ? t("design-search-styles")
            : t("design-search-resources")}
        />
      </label>
    </div>
    {#if activeView !== "global-styles"}
      <button class="ui-button primary toolbar toolbar-action" type="button" disabled={mutating} onclick={beginCreate}>
        <IconPlus size={14} stroke={2} /> {t("design-add")}
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
        <DesignTokenCatalog
          catalog={designTokenCatalog}
          loading={designTokenLoading}
          error={designTokenError}
          {query}
          {category}
          selectedId={selectedToken?.id ?? ""}
          {selectToken}
        />
      {:else if activeView === "classes"}
        {#if app.designClassInventoryError}
          <div class="workspace-state error" role="alert">{app.designClassInventoryError}</div>
        {:else if app.designClassInventoryLoading && !app.designClassInventory}
          <div class="workspace-state">{t("design-loading-classes")}</div>
        {:else}
          {#each classes as entry (entry.name)}
            <button
              type="button"
              class="class-row ui-entity-selectable"
              data-ui-selected={selectedClass?.name === entry.name ? "true" : undefined}
              aria-pressed={selectedClass?.name === entry.name}
              onclick={() => selectClass(entry.name)}
            >
              <span class="resource-icon"><IconTags size={16} stroke={1.8} /></span>
              <span><strong>.{entry.name}</strong><small>{t("design-files-count", { count: entry.files.length })}</small></span>
              <code>{t("design-markup-count", { count: entry.markupOccurrences })}</code>
              <small>{t("design-selectors-count", { count: entry.selectorOccurrences })}</small>
            </button>
          {:else}
            <div class="workspace-state">{t("design-empty-classes")}</div>
          {/each}
        {/if}
      {:else if activeView === "styles"}
        {#each styles as style (style.id)}
          <button
            type="button"
            class="style-row ui-entity-selectable"
            data-ui-selected={selectedStyle?.id === style.id ? "true" : undefined}
            aria-pressed={selectedStyle?.id === style.id}
            onclick={() => selectStyle(style.id)}
          >
            <span class="resource-icon"><IconFileTypeCss size={16} stroke={1.8} /></span>
            <span><strong>{style.file.split("/").at(-1)}</strong><small>{style.file}</small></span>
            <code>{style.scope}</code>
            <small>{t("design-usages-count", { count: styleUsageCount(style) })}</small>
          </button>
        {:else}
          <div class="workspace-state">{t("design-empty-stylesheets")}</div>
        {/each}
      {:else if fontError}
        <div class="workspace-state error" role="alert">{fontError}</div>
      {:else if fontInventory}
        <section class="font-role-overview" aria-label={t("design-font-roles-label")}>
          <header>
            <strong>{t("design-semantic-use")}</strong>
            <small>{t("design-authoritative-scss")}</small>
          </header>
          <div>
            {#each fontRoles as role (role.id)}
              <span class:missing={!role.installed} title={role.diagnostic ?? role.value ?? ""}>
                <small>{role.label}</small>
                <strong>{role.family ?? t("design-role-missing", { variable: role.variableName })}</strong>
              </span>
            {/each}
          </div>
        </section>
        {#each visibleFonts as family (`${family.origin}:${family.directory}`)}
          <button
            type="button"
            class="font-row ui-entity-selectable"
            data-ui-selected={selectedFont?.directory === family.directory && selectedFont?.origin === family.origin ? "true" : undefined}
            aria-pressed={selectedFont?.directory === family.directory && selectedFont?.origin === family.origin}
            onclick={() => selectFont(family.origin, family.directory)}
          >
            <span class="resource-icon"><IconTypography size={16} stroke={1.8} /></span>
            <div><strong>{family.family}</strong><small>{family.directory}</small></div>
            <span>{t("design-files-count", { count: family.files.length })}</span>
            <span
              class="font-registration"
              class:missing={!family.registration.registered}
              title={family.registration.registered
                ? t("design-font-registered-in", { stylesheets: family.registration.stylesheets.join(", ") })
                : t("design-font-unregistered-help")}
            >
              {family.origin === "local" ? t("design-origin-local") : t("design-origin-theme")} ·
              {family.registration.registered
                ? (family.registration.managed ? t("design-font-managed") : t("design-font-registered"))
                : t("design-font-unregistered")}
            </span>
          </button>
        {:else}
          <div class="workspace-state">{t("design-empty-fonts")}</div>
        {/each}
      {:else}
        <div class="workspace-state">{t("design-loading-fonts")}</div>
      {/if}
    </div>

    <aside class="resource-detail" aria-label={t("design-detail-label")}>
      {#if detailMode === "create"}
        <form class="resource-form" onsubmit={(event) => { event.preventDefault(); void createResource(); }}>
          <header class="detail-heading">
            <div>
              <span class="detail-kicker">{t("design-new-resource")}</span>
              <h2>{t("design-add-resource", {
                resource: designViews.find((view) => view.id === activeView)?.label.toLocaleLowerCase(l10n.locale) ?? "",
              })}</h2>
              <p>{t("design-create-description")}</p>
            </div>
            <button class="ui-icon-button ui-close-button" type="button" aria-label={t("design-cancel-create")} disabled={mutating} onclick={resetPanel}><IconX size={14} /></button>
          </header>

          {#if activeView === "tokens"}
            <label><span>{t("design-token-name")}</span><input bind:value={formName} disabled={mutating} placeholder="color-accent" /></label>
            <label><span>{t("design-scss-value")}</span><input bind:value={formValue} disabled={mutating} placeholder="#16836f" /></label>
            <label><span>{t("design-scss-file")}</span><input bind:value={formPath} disabled={mutating} /></label>
          {:else if activeView === "classes"}
            <label><span>{t("design-class-name")}</span><input bind:value={formName} disabled={mutating} placeholder="service-card" /></label>
            <label><span>{t("design-destination-stylesheet")}</span><input bind:value={formPath} disabled={mutating} /></label>
          {:else if activeView === "styles"}
            <label><span>{t("design-project-path")}</span><input bind:value={formPath} disabled={mutating} placeholder="sass/pages/new-style.scss" /></label>
          {:else}
            <div class="ui-tabs font-source-switch" role="tablist" aria-label={t("design-font-source")}>
              <button
                id="font-source-tab-google"
                class="ui-tab"
                type="button"
                role="tab"
                aria-selected={fontCreateSource === "google"}
                class:active={fontCreateSource === "google"}
                tabindex={fontCreateSource === "google" ? 0 : -1}
                disabled={mutating || localFontPlanning}
                onclick={() => selectFontCreateSource("google")}
                onkeydown={(event) => handleFontSourceKeydown(event, "google")}
              ><IconBrandGoogle size={14} /> Google Fonts</button>
              <button
                id="font-source-tab-local"
                class="ui-tab"
                type="button"
                role="tab"
                aria-selected={fontCreateSource === "local"}
                class:active={fontCreateSource === "local"}
                tabindex={fontCreateSource === "local" ? 0 : -1}
                disabled={mutating || localFontPlanning}
                onclick={() => selectFontCreateSource("local")}
                onkeydown={(event) => handleFontSourceKeydown(event, "local")}
              ><IconFolderOpen size={14} /> {t("design-from-computer")}</button>
            </div>
            {#if fontCreateSource === "google"}
            <div class="google-source">
              <span class="google-source-title"><IconBrandGoogle size={15} stroke={1.9} /> {t("design-google-catalog")}</span>
              <p>{t("design-google-description")}</p>
            </div>
            <div class="font-search-field">
              <span>{t("design-search-family")}</span>
              <span class="google-search">
                <input
                  bind:value={googleFontQuery}
                  disabled={mutating || googleFontLoading}
                  placeholder="Space Grotesk"
                  onkeydown={(event) => {
                    if (event.key !== "Enter") return;
                    event.preventDefault();
                    void searchGoogleFontCatalog();
                  }}
                />
                <button
                  type="button"
                  disabled={mutating || googleFontLoading}
                  onclick={() => { void searchGoogleFontCatalog(); }}
                >
                  <IconSearch size={14} /> {googleFontLoading ? t("design-searching") : t("design-search")}
                </button>
              </span>
            </div>
            {#if googleFontError}
              <p class="form-error" role="alert"><IconAlertTriangle size={14} /> {googleFontError}</p>
            {:else if googleFontLoading}
              <div class="google-state">{t("design-loading-google-catalog")}</div>
            {:else}
              <div class="google-results" aria-label={t("design-google-families-label")}>
                {#each googleFontResults as font (font.family)}
                  <button
                    type="button"
                    class="ui-entity-selectable"
                    data-ui-selected={selectedGoogleFont?.family === font.family ? "true" : undefined}
                    aria-pressed={selectedGoogleFont?.family === font.family}
                    onclick={() => selectGoogleFont(font)}
                  >
                    <span class="google-font-sample">Ag</span>
                    <span>
                      <strong>{font.family}</strong>
                      <small>{font.category ?? t("design-web-font")} · {t("design-variants-count", { count: font.variants.length })}</small>
                    </span>
                    {#if selectedGoogleFont?.family === font.family}
                      <IconCircleCheck size={16} stroke={2} />
                    {/if}
                  </button>
                {:else}
                  <div class="google-state">{t("design-empty-google-search")}</div>
                {/each}
              </div>
            {/if}
            {#if selectedGoogleFont}
              <div class="font-install-options">
                <span>{t("design-installed-styles")}</span>
                <div class="weight-options">
                  {#each availableGoogleStyles(selectedGoogleFont) as style (style)}
                    <button
                      type="button"
                      class:selected={formGoogleStyles.includes(style)}
                      aria-pressed={formGoogleStyles.includes(style)}
                      disabled={mutating}
                      onclick={() => toggleGoogleStyle(style)}
                    >{style === "normal" ? t("design-style-normal") : t("design-style-italic")}</button>
                  {/each}
                </div>
                <span>{t("design-installed-weights")}</span>
                <div class="weight-options">
                  {#each selectedGoogleFont.weights as weight (weight)}
                    <button
                      type="button"
                      class:selected={selectedGoogleWeights().includes(weight)}
                      aria-pressed={selectedGoogleWeights().includes(weight)}
                      disabled={mutating || formVariableFont}
                      onclick={() => toggleGoogleWeight(weight)}
                    >{weight}</button>
                  {/each}
                </div>
                {#if selectedGoogleFont.axes.some((axis) => axis.tag === "wght")}
                  <label class="check-field">
                    <input
                      checked={formVariableFont}
                      type="checkbox"
                      disabled={mutating}
                      onchange={(event) => setVariableFont(event.currentTarget.checked)}
                    />
                    <span>{t("design-full-variable-range")}</span>
                  </label>
                {/if}
                {#if advancedGoogleAxes(selectedGoogleFont).length}
                  <span>{t("design-advanced-axes")}</span>
                  <div class="axis-options">
                    {#each advancedGoogleAxes(selectedGoogleFont) as axis (axis.tag)}
                      <button
                        type="button"
                        class:selected={formGoogleAxes.includes(axis.tag)}
                        aria-pressed={formGoogleAxes.includes(axis.tag)}
                        disabled={mutating}
                        title={t("design-google-axis-range", { start: axis.start, end: axis.end })}
                        onclick={() => toggleGoogleAxis(axis.tag)}
                      >
                        <strong>{axis.tag}</strong>
                        <small>{axis.start}–{axis.end}</small>
                      </button>
                    {/each}
                  </div>
                  <small class="axis-help">{t("design-axis-help")}</small>
                {/if}
                <label class="font-character-set">
                  <span>{t("design-character-optimization")}</span>
                  <textarea
                    bind:value={formGoogleCharacterSet}
                    disabled={mutating}
                    maxlength="640"
                    rows="3"
                    placeholder={t("design-character-example")}
                  ></textarea>
                  <small>{t("design-character-help")}</small>
                </label>
              </div>
            {/if}
            {:else}
              <div class="google-source">
                <span class="google-source-title"><IconFolderOpen size={15} stroke={1.9} /> {t("design-local-files")}</span>
                <p>{t("design-local-description")}</p>
              </div>
              <button
                class="local-font-picker"
                type="button"
                disabled={mutating || localFontPlanning}
                onclick={() => { void chooseAndPlanLocalFonts(); }}
              >
                <IconFolderOpen size={15} />
                {localFontPlanning
                  ? t("design-analyzing-rust")
                  : localFontPaths.length
                    ? t("design-choose-other-files")
                    : t("design-choose-font-files")}
              </button>
              {#if localFontPlanning}
                <div class="google-state">{t("design-checking-fonts")}</div>
              {:else if localFontPlan}
                <div class="local-font-plan" aria-label={t("design-local-plan-label")}>
                  <div class="local-plan-summary">
                    <strong>{localFontPlan.families.map((family) => family.family).join(", ")}</strong>
                    <small>
                      {t("design-plan-files", { count: localFontPlan.files.length })} ·
                      {localFontPlan.families.some((family) => family.variable) ? ` ${t("design-includes-variable-font")} ·` : ""}
                      {localFontPlan.stylesheetPath}
                    </small>
                  </div>
                  {#each localFontPlan.files as file (file.destinationPath)}
                    <div class="local-plan-file">
                      <span>
                        <strong>{file.subfamily ?? `${file.weightRange ? `${file.weightRange.start}–${file.weightRange.end}` : file.weight ?? 400} ${file.style}`}</strong>
                        <small>{file.family} · {file.format.toUpperCase()} · {Math.max(1, Math.round(file.sizeBytes / 1024))} KB</small>
                      </span>
                      <code>{file.destinationPath}</code>
                    </div>
                  {/each}
                </div>
                {#each localFontPlan.warnings as warning}
                  <p class="plan-message warning"><IconAlertTriangle size={14} /> {warning}</p>
                {/each}
                {#each localFontPlan.conflicts as conflict}
                  <p class="form-error" role="alert"><IconAlertTriangle size={14} /> {conflict}</p>
                {/each}
              {:else}
                <div class="google-state">{t("design-local-selection-help")}</div>
              {/if}
            {/if}
          {/if}

          {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button type="button" disabled={mutating} onclick={resetPanel}>{t("design-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={mutating || !formReady}>
              {#if activeView === "fonts"}<IconDownload size={14} />{:else}<IconPlus size={14} />{/if}
              {mutating
                ? (activeView === "fonts" ? t("design-installing-rust") : t("design-creating-rust"))
                : (activeView === "fonts"
                  ? (fontCreateSource === "local" ? t("design-confirm-import") : t("design-install-project"))
                  : t("design-create-session"))}
            </button>
          </div>
        </form>
      {:else if detailMode === "edit"}
        <form class="resource-form" onsubmit={(event) => { event.preventDefault(); void saveEdit(); }}>
          <header class="detail-heading">
            <div>
              <span class="detail-kicker">{t("design-controlled-change")}</span>
              <h2>
                {activeView === "tokens" && selectedToken ? `$${selectedToken.name}`
                  : activeView === "classes" && selectedClass ? `.${selectedClass.name}`
                    : selectedStyle?.file.split("/").at(-1) ?? t("design-generic-resource")}
              </h2>
              <p>{t("design-change-description")}</p>
            </div>
            <button class="ui-icon-button ui-close-button" type="button" aria-label={t("design-cancel-edit")} disabled={mutating} onclick={resetPanel}><IconX size={14} /></button>
          </header>

          {#if activeView === "tokens"}
            <label><span>{t("design-scss-value")}</span><input bind:value={formValue} disabled={mutating} /></label>
            <div class="source-card"><span>{t("design-source")}</span><code>{formPath}</code></div>
          {:else if activeView === "classes"}
            <label><span>{t("design-class-name")}</span><input bind:value={formName} disabled={mutating} /></label>
          {:else}
            <label><span>{t("design-file-name")}</span><input bind:value={formName} disabled={mutating} /></label>
            <div class="source-card"><span>{t("design-current-path")}</span><code>{formPath}</code></div>
          {/if}

          {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button type="button" disabled={mutating} onclick={resetPanel}>{t("design-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={mutating}>
              <IconDeviceFloppy size={14} /> {mutating ? t("design-updating-rust") : t("design-save-changes")}
            </button>
          </div>
        </form>
      {:else if activeView === "tokens" && selectedToken}
        <span class="detail-kicker">{categoryLabel(selectedToken)} · {selectedToken.groupLabel}</span>
        <h2>${selectedToken.name}</h2>
        <p>{t("design-token-description")}</p>
        <dl class="info-grid">
          <div><dt>{t("design-scss-value")}</dt><dd>{selectedToken.rawValue}</dd></div>
          <div><dt>{t("design-resolved-value")}</dt><dd>{selectedToken.resolvedValue ?? t("design-unresolved")}</dd></div>
          <div><dt>{t("design-category")}</dt><dd>{categoryLabel(selectedToken)}</dd></div>
          <div><dt>{t("design-dependencies")}</dt><dd>{l10n.formatNumber(selectedToken.dependencies.length)}</dd></div>
        </dl>
        {#if selectedToken.dependencies.length > 0}
          <div class="source-card">
            <span>{t("design-token-chain")}</span>
            <code>{selectedToken.dependencies.map((dependency) => `$${dependency}`).join(" → ")}</code>
          </div>
        {/if}
        {#if selectedToken.diagnostic}
          <p class="token-diagnostic" role="alert"><IconAlertTriangle size={14} /> {t("design-token-resolution-failed")}</p>
        {/if}
        <div class="source-card"><span>{t("design-source")}</span><code>{selectedToken.sourcePath}:{selectedToken.sourceLine}</code></div>
        <div class="detail-actions">
          <button class="ui-button primary primary-action" type="button" disabled={!selectedToken.editable} onclick={beginEdit}><IconEdit size={14} /> {t("design-edit")}</button>
          <button class="ui-button secondary-action" type="button" onclick={() => { void openWorkspaceSource(selectedToken.sourcePath); }}>{t("design-open-source")} <IconExternalLink size={13} /></button>
        </div>
      {:else if activeView === "classes" && selectedClass}
        <span class="detail-kicker">{t("design-class-inventory")}</span>
        <h2>.{selectedClass.name}</h2>
        <p>{t("design-class-summary", {
          markup: selectedClass.markupOccurrences,
          selectors: selectedClass.selectorOccurrences,
        })}</p>
        <dl class="info-grid">
          <div><dt>{t("design-markup")}</dt><dd>{l10n.formatNumber(selectedClass.markupOccurrences)}</dd></div>
          <div><dt>{t("design-selectors")}</dt><dd>{l10n.formatNumber(selectedClass.selectorOccurrences)}</dd></div>
        </dl>
        <div class="detail-actions">
          <button class="ui-button primary primary-action" type="button" onclick={beginEdit}><IconEdit size={14} /> {t("design-edit")}</button>
        </div>
        <div class="occurrence-list" aria-label={t("design-class-occurrences")}>
          {#each selectedClass.occurrences.slice(0, 40) as occurrence (`${occurrence.file}:${occurrence.range.start}`)}
            <button type="button" onclick={() => { void openWorkspaceSource(occurrence.file); }}>
              <span>{occurrence.kind === "markup" ? t("design-markup") : t("design-selectors")}</span>
              <code>{occurrence.file}:{occurrence.range.line}:{occurrence.range.column}</code>
            </button>
          {/each}
        </div>
      {:else if activeView === "styles" && selectedStyle}
        <span class="detail-kicker">{t("design-stylesheet-kicker", { scope: selectedStyle.scope })}</span>
        <h2>{selectedStyle.file.split("/").at(-1)}</h2>
        <p>{t("design-stylesheet-summary", { count: styleUsageCount(selectedStyle) })}</p>
        <div class="source-card"><span>{t("design-path")}</span><code>{selectedStyle.file}</code></div>
        <div class="detail-actions">
          <button class="ui-button primary primary-action" type="button" onclick={beginEdit}><IconEdit size={14} /> {t("design-edit")}</button>
          <button class="ui-button secondary-action" type="button" onclick={() => { void openWorkspaceSource(selectedStyle.file); }}>{t("design-open-editor")} <IconExternalLink size={13} /></button>
        </div>
      {:else if activeView === "fonts" && selectedFont}
        <span class="detail-kicker">{t("design-font-inventory")}</span>
        <h2>{selectedFont.family}</h2>
        <p>{t("design-font-description")}</p>
        <div class="font-preview" aria-label={t("design-font-preview-label", { family: selectedFont.family })}>
          <strong>{t("design-font-preview-text")}</strong>
          <span>{fontPreviewLoading
            ? t("design-font-preview-loading")
            : selectedFontPreviewFile?.subfamily ?? t("design-font-preview-real")}</span>
        </div>
        {#if fontPreviewError}
          <p class="font-preview-error"><IconAlertTriangle size={13} /> {t("design-font-preview-error", { message: fontPreviewError })}</p>
        {/if}
        <dl class="info-grid">
          <div><dt>{t("design-origin")}</dt><dd>{selectedFont.origin === "local" ? t("design-origin-local") : selectedFont.themeName ?? t("design-origin-theme")}</dd></div>
          <div><dt>{t("design-files")}</dt><dd>{l10n.formatNumber(selectedFont.files.length)}</dd></div>
          <div>
            <dt>{t("design-css-registration")}</dt>
            <dd>{selectedFont.registration.registered
              ? (selectedFont.registration.managed
                ? t("design-registration-managed")
                : t("design-registration-detected"))
              : t("design-registration-missing")}</dd>
          </div>
          <div><dt>{t("design-font-display-policy")}</dt><dd>{selectedFont.registration.displayModes.join(", ") || "—"}</dd></div>
          <div><dt>{t("design-font-variable")}</dt><dd>{selectedFont.files.some((file) => file.axes.length > 0) ? t("design-yes") : t("design-no")}</dd></div>
          <div><dt>{t("design-license")}</dt><dd>{selectedFont.license.description || selectedFont.license.url ? t("design-license-metadata") : t("design-license-undeclared")}</dd></div>
        </dl>
        <div class="source-card"><span>{t("design-directory")}</span><code>{selectedFont.directory}</code></div>
        {#if selectedFont.registration.stylesheets.length}
          <div class="source-card">
            <span>{t("design-font-face-declarations")}</span>
            <code>{selectedFont.registration.stylesheets.join(", ")}</code>
          </div>
        {/if}
        <section class="font-delivery-actions" aria-labelledby="font-delivery-title">
          <div>
            <span id="font-delivery-title">{t("design-browser-delivery")}</span>
            <small>{t("design-browser-delivery-description")}</small>
          </div>
          <label>
            <span>{t("design-font-display-policy")}</span>
            <select
              value={selectedFont.registration.displayModes.length === 1
                ? selectedFont.registration.displayModes[0]
                : ""}
              disabled={mutating || !selectedFont.registration.managed}
              onchange={(event) => {
                const display = event.currentTarget.value as "auto" | "block" | "swap" | "fallback" | "optional";
                if (display) void changeSelectedFontDisplay(display);
              }}
            >
              {#if selectedFont.registration.displayModes.length !== 1}
                <option value="">{t("design-choose-policy")}</option>
              {/if}
              <option value="swap">{t("design-display-swap")}</option>
              <option value="optional">{t("design-display-optional")}</option>
              <option value="fallback">{t("design-display-fallback")}</option>
              <option value="block">{t("design-display-block")}</option>
              <option value="auto">{t("design-display-auto")}</option>
            </select>
          </label>
          {#if !selectedFont.registration.managed}
            <small>{t("design-display-managed-only")}</small>
          {/if}
        </section>
        {#if selectedFont.license.description || selectedFont.license.url}
          <div class="font-license">
            <span>{t("design-font-license-included")}</span>
            {#if selectedFont.license.description}<p>{selectedFont.license.description}</p>{/if}
            {#if selectedFont.license.url}<code>{selectedFont.license.url}</code>{/if}
          </div>
        {/if}
        <section class="font-role-actions" aria-labelledby="font-role-actions-title">
          <div>
            <span id="font-role-actions-title">{t("design-use-family-for")}</span>
            <small>{t("design-role-description")}</small>
          </div>
          <div>
            {#each fontRoles as role (role.id)}
              <button
                type="button"
                class:active={role.family === selectedFont.family}
                disabled={mutating || !role.assignable || !selectedFont.registration.registered}
                title={role.diagnostic ?? t("design-assign-role", {
                  family: selectedFont.family,
                  role: role.label,
                })}
                onclick={() => { void assignSelectedFontToRole(role.id); }}
              >
                <IconTypography size={14} />
                <span>{role.label}</span>
                {#if role.family === selectedFont.family}<IconCircleCheck size={14} />{/if}
              </button>
            {/each}
          </div>
        </section>
        {#if selectedFontDiagnostics.length}
          <div class="font-diagnostics" aria-label={t("design-font-diagnostics-label")}>
            {#each selectedFontDiagnostics as diagnostic (`${diagnostic.code}:${diagnostic.file ?? diagnostic.family ?? "global"}`)}
              <p class:error={diagnostic.severity === "error"} class:warning={diagnostic.severity === "warning"}>
                <IconAlertTriangle size={14} />
                <span>{errorMessage(diagnostic.messageDiagnostic)}</span>
              </p>
            {/each}
          </div>
        {/if}
        {#if formError}<p class="form-error font-action-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
        <div class="font-files" aria-label={t("design-family-variants-label")}>
          {#each selectedFont.files as file (file.file)}
            <div>
              <span>
                <strong>{file.subfamily ?? (file.weightRange ? `${file.weightRange.start}–${file.weightRange.end}` : file.weight ?? 400)}</strong>
                {file.style ?? "normal"}
              </span>
              <small>
                {file.format.toUpperCase()} · {Math.max(1, Math.round(file.sizeBytes / 1024))} KB
                {file.textOptimized ? ` · ${t("design-exact-character-set")}` : ""}
                {file.axes.length
                  ? ` · ${file.axes.map((axis) => `${axis.tag} ${axis.min}–${axis.max} (${t("design-axis-default", { value: axis.default })})`).join(" · ")}`
                  : ""}
              </small>
              <button
                type="button"
                class:active={file.preload.preloaded}
                disabled={mutating
                  || !selectedFont.registration.registered
                  || (file.preload.preloaded && !file.preload.managed)}
                title={file.preload.templates.length
                  ? t("design-preload-template", { templates: file.preload.templates.join(", ") })
                  : t("design-preload-add-help")}
                onclick={() => { void toggleFontPreload(file.file, !file.preload.preloaded); }}
              >
                {#if file.preload.preloaded}<IconCircleCheck size={13} />{/if}
                {file.preload.preloaded
                  ? (file.preload.managed ? t("design-preload-active") : t("design-preload-external"))
                  : t("design-preload")}
              </button>
            </div>
          {/each}
        </div>
        {#if selectedFont.origin === "local"}
          <section class="font-removal" aria-labelledby="font-removal-title">
            <div>
              <span id="font-removal-title">{t("design-controlled-removal")}</span>
              <small>{t("design-removal-description")}</small>
            </div>
            {#if fontRemovalPlan}
              <dl>
                <div><dt>{t("design-fonts")}</dt><dd>{l10n.formatNumber(fontRemovalPlan.files.length)}</dd></div>
                <div><dt>{t("design-stylesheets")}</dt><dd>{l10n.formatNumber(fontRemovalPlan.stylesheetPaths.length)}</dd></div>
                <div><dt>{t("design-licenses")}</dt><dd>{l10n.formatNumber(fontRemovalPlan.licenseFiles.length)}</dd></div>
              </dl>
              {#each fontRemovalPlan.blockedReasons as reason}
                <p class="blocked"><IconAlertTriangle size={13} /> {reason}</p>
              {/each}
              {#each fontRemovalPlan.warnings as warning}
                <p><IconAlertTriangle size={13} /> {warning}</p>
              {/each}
              <div class="font-removal-actions">
                <button
                  type="button"
                  disabled={mutating}
                  onclick={() => { fontRemovalPlan = null; }}
                >
                  {t("design-cancel")}
                </button>
                <button
                  class="ui-button danger"
                  type="button"
                  disabled={mutating || !fontRemovalPlan.changed || fontRemovalPlan.blockedReasons.length > 0}
                  onclick={() => { void confirmSelectedFontRemoval(); }}
                >
                  <IconTrash size={13} />
                  {mutating ? t("design-removing-rust") : t("design-confirm-removal")}
                </button>
              </div>
            {:else}
              <button
                type="button"
                disabled={mutating || fontRemovalPlanning}
                onclick={() => { void planSelectedFontRemoval(); }}
              >
                <IconTrash size={13} />
                {fontRemovalPlanning ? t("design-analyzing-rust") : t("design-analyze-removal")}
              </button>
            {/if}
          </section>
        {/if}
      {:else}
        <div class="workspace-state">{t("design-select-resource")}</div>
      {/if}
    </aside>
    </div>
  {/if}
</section>

<style>
  dt { color: var(--wb-text-muted); font-size: 12px; font-weight: 650; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 650; }
  .class-row, .style-row, .font-row, .primary-action, .secondary-action, .detail-heading, .form-error, .form-actions, .detail-actions, .token-diagnostic { display: flex; align-items: center; }
  .workspace-body { display: grid; grid-template-columns: minmax(340px, 1fr) minmax(290px, .58fr); min-width: 0; min-height: 0; }
  .class-row, .style-row { display: grid; gap: 9px; width: 100%; min-height: 52px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .class-row, .style-row { grid-template-columns: 34px minmax(0, 1fr) auto 70px; }
  .class-row > span:nth-child(2), .style-row > span:nth-child(2), .font-row > div { display: grid; gap: 3px; min-width: 0; }
  .class-row strong, .style-row strong, .font-row strong { color: var(--text-strong); font-size: 12px; }
  .class-row small, .style-row small, .font-row small { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .class-row code, .style-row code { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-align: right; text-overflow: ellipsis; white-space: nowrap; }
  .resource-icon { display: grid; width: 29px; height: 29px; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .font-row { display: grid; grid-template-columns: 34px minmax(0, 1fr) auto 140px; gap: 8px; width: 100%; min-height: 52px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .font-row > span { color: var(--wb-text-muted); font-size: 12px; }
  .font-registration { font-weight: 700; text-align: right; }
  .font-registration:not(.missing) { color: var(--wb-accent-strong); }
  .font-registration.missing { color: var(--danger); }
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
  .font-source-switch { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .font-source-switch .ui-tab { width: 100%; min-width: 0; }
  .google-source { display: grid; gap: 4px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .google-source-title { display: flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 800; }
  .google-source p { margin: 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.4; }
  .font-search-field { display: grid; gap: 5px; color: var(--wb-text-muted); font-size: 12px; font-weight: 700; }
  .google-search { display: grid; grid-template-columns: minmax(0, 1fr) auto; }
  .google-search input { min-width: 0; height: 34px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-right: 0; border-radius: 6px 0 0 6px; color: var(--text-strong); background: var(--wb-surface-document); font-size: 12px; }
  .google-search button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-width: 82px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: 0 6px 6px 0; color: var(--wb-text-primary); background: var(--wb-control-hover); font-size: 12px; font-weight: 700; }
  .google-results { display: grid; max-height: 250px; overflow: auto; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .google-results > button { --ui-entity-border-color: var(--wb-border-subtle); display: grid; grid-template-columns: 34px minmax(0, 1fr) 18px; gap: 8px; align-items: center; min-height: 49px; padding: 6px 8px; border: 0; border-bottom: 1px solid var(--wb-border-subtle); color: var(--wb-text-primary); background: transparent; text-align: left; }
  .google-results > button:last-of-type { border-bottom: 0; }
  .google-results > button > span:nth-child(2) { display: grid; gap: 2px; min-width: 0; }
  .google-results strong, .google-results small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .google-results strong { color: var(--text-strong); font-size: 12px; }
  .google-results small { color: var(--wb-text-muted); font-size: 11px; font-weight: 500; }
  .google-results :global(svg) { color: var(--wb-accent-strong); }
  .google-font-sample { display: grid; width: 31px; height: 31px; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 16px; font-weight: 650; }
  .google-state { display: grid; min-height: 58px; padding: 10px; place-items: center; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .font-install-options { display: grid; gap: 7px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .font-install-options > span { color: var(--wb-text-muted); font-size: 12px; font-weight: 700; }
  .weight-options { display: flex; flex-wrap: wrap; gap: 5px; }
  .weight-options button { min-width: 43px; min-height: 28px; padding: 0 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; }
  .weight-options button.selected { border-color: var(--wb-accent); color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-weight: 750; }
  .axis-options { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 5px; }
  .axis-options button { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: 6px; min-height: 31px; padding: 0 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); text-align: left; }
  .axis-options button.selected { border-color: var(--wb-accent); color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .axis-options strong { font-size: 11px; text-transform: uppercase; }
  .axis-options small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .font-install-options .axis-help { color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  .font-install-options .font-character-set { display: grid; gap: 5px; padding-top: 3px; color: var(--wb-text-muted); font-size: 11px; font-weight: 700; }
  .font-character-set textarea { width: 100%; min-height: 56px; padding: 7px 8px; resize: vertical; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font: inherit; font-weight: 500; line-height: 1.4; }
  .font-character-set small { color: var(--warning); font-size: 11px; font-weight: 500; line-height: 1.4; }
  .resource-form .check-field { display: flex; align-items: center; gap: 7px; min-height: 32px; }
  .check-field input { width: 15px; height: 15px; accent-color: var(--wb-accent); }
  .local-font-picker { display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 34px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; font-weight: 700; }
  .local-font-plan { display: grid; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .local-plan-summary { display: grid; gap: 3px; padding: 9px; border-bottom: 1px solid var(--wb-border-subtle); }
  .local-plan-summary strong { color: var(--text-strong); font-size: 12px; }
  .local-plan-summary small { color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  .local-plan-file { display: grid; gap: 5px; padding: 8px 9px; border-bottom: 1px solid var(--wb-border-subtle); }
  .local-plan-file:last-child { border-bottom: 0; }
  .local-plan-file > span { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .local-plan-file strong { color: var(--text-strong); font-size: 12px; }
  .local-plan-file small { color: var(--wb-text-muted); font-size: 11px; }
  .local-plan-file code { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .plan-message { display: flex; align-items: flex-start; gap: 6px; margin: 0; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 11px; line-height: 1.4; }
  .plan-message.warning { color: var(--warning); }
  .source-card { display: grid; gap: 4px; margin-top: 9px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .source-card span { color: var(--wb-text-muted); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .source-card code { overflow: hidden; color: var(--wb-text-primary); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .font-files { display: grid; margin-top: 10px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .font-files > div { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; column-gap: 8px; min-height: 46px; padding: 7px 9px; border-bottom: 1px solid var(--wb-border-subtle); }
  .font-files > div:last-child { border-bottom: 0; }
  .font-files span { color: var(--wb-text-primary); font-size: 12px; }
  .font-files small { grid-column: 1; overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .font-files button { display: inline-flex; grid-column: 2; grid-row: 1 / span 2; align-items: center; justify-content: center; gap: 4px; min-height: 27px; padding: 0 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; font-weight: 700; }
  .font-files button.active { border-color: var(--wb-accent); color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .font-license { display: grid; gap: 5px; margin-top: 9px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .font-license span { color: var(--wb-accent-strong); font-size: 11px; font-weight: 800; text-transform: uppercase; }
  .font-license p { display: -webkit-box; margin: 0; overflow: hidden; color: var(--wb-text-primary); font-size: 11px; line-height: 1.45; -webkit-box-orient: vertical; -webkit-line-clamp: 4; line-clamp: 4; }
  .font-license code { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .font-preview { display: grid; gap: 5px; margin-top: 11px; padding: 13px; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .font-preview strong { overflow: hidden; color: var(--text-strong); font-family: "Pana Studio Font Preview", system-ui, sans-serif; font-size: 27px; font-weight: 400; line-height: 1.15; text-overflow: ellipsis; white-space: nowrap; }
  .font-preview span { color: var(--wb-text-muted); font-size: 11px; }
  .font-preview-error { display: flex; align-items: flex-start; gap: 5px; margin: 6px 0 0; color: var(--warning); font-size: 11px; line-height: 1.4; }
  .font-role-overview { display: grid; gap: 7px; margin-bottom: 8px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-chrome); }
  .font-role-overview > header { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .font-role-overview > header strong { color: var(--text-strong); font-size: 12px; }
  .font-role-overview > header small { color: var(--wb-text-muted); font-size: 11px; }
  .font-role-overview > div { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 5px; }
  .font-role-overview > div > span { display: grid; gap: 2px; min-width: 0; padding: 6px 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-document); }
  .font-role-overview > div > span small { color: var(--wb-accent-strong); font-size: 11px; font-weight: 800; text-transform: uppercase; }
  .font-role-overview > div > span strong { overflow: hidden; color: var(--text-strong); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .font-role-overview > div > span.missing strong { color: var(--danger); }
  .font-role-actions { display: grid; gap: 8px; margin-top: 9px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .font-role-actions > div:first-child { display: grid; gap: 3px; }
  .font-role-actions > div:first-child span { color: var(--text-strong); font-size: 12px; font-weight: 750; }
  .font-role-actions > div:first-child small { color: var(--wb-text-muted); font-size: 11px; }
  .font-role-actions > div:last-child { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 5px; }
  .font-role-actions button { display: grid; grid-template-columns: 16px minmax(0, 1fr) 16px; align-items: center; gap: 5px; min-height: 31px; padding: 0 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; text-align: left; }
  .font-role-actions button.active { border-color: var(--wb-accent); color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-weight: 750; }
  .font-delivery-actions { display: grid; gap: 8px; margin-top: 9px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .font-delivery-actions > div { display: grid; gap: 3px; }
  .font-delivery-actions > div span { color: var(--text-strong); font-size: 12px; font-weight: 750; }
  .font-delivery-actions small { color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  .font-delivery-actions label { display: grid; grid-template-columns: minmax(100px, 1fr) minmax(150px, 1.2fr); align-items: center; gap: 8px; color: var(--wb-text-muted); font-size: 11px; }
  .font-delivery-actions select { min-width: 0; height: 30px; padding: 0 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; }
  .font-diagnostics { display: grid; gap: 5px; margin-top: 9px; }
  .font-diagnostics p { display: flex; align-items: flex-start; gap: 6px; margin: 0; padding: 7px 8px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-muted); background: var(--wb-surface-document); font-size: 11px; line-height: 1.4; }
  .font-diagnostics p.warning { color: var(--warning); }
  .font-diagnostics p.error { color: var(--danger); }
  .font-diagnostics :global(svg) { flex: 0 0 auto; margin-top: 1px; }
  .font-removal { display: grid; gap: 8px; margin-top: 10px; padding: 9px; border: 1px solid color-mix(in srgb, var(--danger) 30%, var(--wb-border-subtle)); border-radius: 7px; background: var(--wb-surface-document); }
  .font-removal > div:first-child { display: grid; gap: 3px; }
  .font-removal > div:first-child span { color: var(--text-strong); font-size: 12px; font-weight: 750; }
  .font-removal > div:first-child small { color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  .font-removal > button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-height: 30px; border: 1px solid color-mix(in srgb, var(--danger) 45%, var(--wb-border-subtle)); border-radius: 5px; color: var(--danger); background: var(--wb-surface-chrome); font-size: 11px; font-weight: 750; }
  .font-removal dl { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 5px; margin: 0; }
  .font-removal dl div { min-width: 0; padding: 6px 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-chrome); }
  .font-removal dt { font-size: 11px; }
  .font-removal dd { font-size: 12px; }
  .font-removal p { display: flex; align-items: flex-start; gap: 5px; margin: 0; color: var(--warning); font-size: 11px; line-height: 1.4; }
  .font-removal p.blocked { color: var(--danger); }
  .font-removal p :global(svg) { flex: 0 0 auto; margin-top: 1px; }
  .font-removal-actions { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; }
  .font-removal-actions button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; min-height: 30px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; font-weight: 700; }
  .font-removal-actions button.danger { border-color: color-mix(in srgb, var(--danger) 55%, var(--wb-border-subtle)); color: #fff; background: var(--danger); }
  .font-action-error { margin-top: 9px; }
  .form-error { align-items: flex-start; gap: 6px; margin: 0; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger) 36%, var(--wb-border-subtle)); border-radius: 6px; color: var(--danger); background: color-mix(in srgb, var(--danger) 7%, var(--wb-surface-document)); font-size: 12px; line-height: 1.4; }
  .form-error :global(svg) { flex: 0 0 auto; margin-top: 1px; }
  .token-diagnostic { align-items: flex-start; gap: 6px; margin: 10px 0 0; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger) 35%, var(--wb-border-subtle)); border-radius: 6px; color: var(--danger-strong, #b42318); background: color-mix(in srgb, var(--danger) 7%, var(--wb-surface-document)); font-size: 11px; line-height: 1.4; }
  .token-diagnostic :global(svg) { flex: 0 0 auto; margin-top: 1px; }
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
  .secondary-action { border-color: var(--wb-border-subtle); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  button:disabled { opacity: .5; }
  button:not(:disabled) { cursor: pointer; }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .workspace-state { display: grid; min-height: 180px; place-items: center; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .workspace-state.error { color: var(--danger); }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .resource-detail { display: none; } .resource-list { border-right: 0; } }
</style>
