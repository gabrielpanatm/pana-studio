import type {
  BundledFontCatalogFamily,
  FontFamilyRemovalPlan,
  FontRoleId,
  GoogleFontCatalogFamily,
  LocalFontImportPlan,
} from "$lib/fonts/contracts";
import {
  applyLocalFontImport,
  assignFontRole,
  chooseFontFiles,
  downloadGoogleFontFamily,
  getBundledFontCatalog,
  getBundledFontPreview,
  getFontPreviewAsset,
  installBundledFontFamily,
  planFontFamilyRemoval,
  planLocalFontImport,
  removeFontFamily,
  searchGoogleFonts,
  setFontDisplay,
  setFontPreload,
} from "$lib/fonts/io";
import type { FontManagerState } from "$lib/fonts/manager-state.svelte";
import { t } from "$lib/i18n/runtime.svelte";
import type { ProjectWorkspaceIdentity } from "$lib/project/workspace-contract";
import { workspaceMutationAuthorityReceipt } from "$lib/session/workspace-mutation-coordinator";
import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import { errorMessage } from "$lib/util";
import type { DetailMode } from "../contracts";

export type FontCreateSource = "google" | "bundled" | "local";

export class FontManagerController {
  query = $state("");
  selectedFontKey = $state("");
  detailMode = $state<DetailMode>("info");
  fontRemovalPlan = $state<FontFamilyRemovalPlan | null>(null);
  fontRemovalPlanning = $state(false);
  fontPreviewError = $state("");
  fontPreviewLoading = $state(false);
  formName = $state("");
  formWeights = $state("400, 700");
  formGoogleStyles = $state<string[]>(["normal"]);
  formVariableFont = $state(false);
  formGoogleAxes = $state<string[]>([]);
  formGoogleCharacterSet = $state("");
  googleFontQuery = $state("");
  googleFontResults = $state<GoogleFontCatalogFamily[]>([]);
  googleFontLoading = $state(false);
  googleFontError = $state("");
  bundledFontCatalog = $state<BundledFontCatalogFamily[]>([]);
  bundledFontLoading = $state(false);
  bundledFontError = $state("");
  bundledFontQuery = $state("");
  bundledFontCategory = $state("all");
  selectedBundledFontId = $state("");
  bundledFontPreviewLoading = $state(false);
  bundledFontPreviewError = $state("");
  fontCreateSource = $state<FontCreateSource>("google");
  localFontPaths = $state<string[]>([]);
  localFontPlan = $state<LocalFontImportPlan | null>(null);
  localFontPlanning = $state(false);
  formError = $state("");
  mutating = $state(false);

  private fontPreviewSequence = 0;
  private fontPreviewStyle: HTMLStyleElement | null = null;
  private googleFontSearchSequence = 0;
  private bundledFontCatalogSequence = 0;
  private bundledFontPreviewSequence = 0;
  private bundledFontPreviewStyle: HTMLStyleElement | null = null;
  private localFontPlanSequence = 0;

  constructor(
    readonly manager: FontManagerState,
    private readonly workspaceMutations: ProjectWorkspaceMutationService,
    private readonly globalStatus: GlobalStatusState,
  ) {}

  get graph() { return this.manager.snapshot?.graph ?? null; }
  get roles() { return this.manager.snapshot?.roles ?? []; }
  get diagnostics() { return this.manager.snapshot?.diagnostics ?? []; }

  get visibleFonts() {
    const query = this.query.trim().toLocaleLowerCase();
    return (this.graph?.families ?? []).filter((family) => (
      !query || `${family.family} ${family.directories.join(" ")}`.toLocaleLowerCase().includes(query)
    ));
  }

  get selectedFont() {
    return (this.graph?.families ?? []).find((family) => family.id === this.selectedFontKey)
      ?? this.visibleFonts[0]
      ?? null;
  }

  get selectedFontDiagnostics() {
    return this.diagnostics.filter((diagnostic) => (
      diagnostic.family === null || diagnostic.family === this.selectedFont?.family
    ));
  }

  get selectedGoogleFont() {
    return this.googleFontResults.find((family) => family.family === this.formName) ?? null;
  }

  get bundledFontCategories() {
    return [...new Set(this.bundledFontCatalog.map((family) => family.category))].sort();
  }

  get visibleBundledFonts() {
    const search = this.bundledFontQuery.trim().toLocaleLowerCase();
    return this.bundledFontCatalog.filter((family) => (
      (this.bundledFontCategory === "all" || family.category === this.bundledFontCategory)
      && (!search || `${family.family} ${family.category}`.toLocaleLowerCase().includes(search))
    ));
  }

  get selectedBundledFont() {
    return this.bundledFontCatalog.find((family) => family.id === this.selectedBundledFontId) ?? null;
  }

  get selectedFontPreviewFile() {
    if (!this.selectedFont) return null;
    return [...this.selectedFont.files].sort((left, right) => {
      const score = (file: typeof left) => (file.extension === "woff2" ? 0 : 20)
        + ((file.declaredStyle ?? file.style) === "normal" ? 0 : 5)
        + ((file.declaredWeight ?? file.weight) === 400
          || (() => {
            const range = file.declaredWeightRange ?? file.weightRange;
            return Boolean(range && range.start <= 400 && range.end >= 400);
          })() ? 0 : 2);
      return score(left) - score(right);
    })[0] ?? null;
  }

  get formReady() {
    if (this.fontCreateSource === "local") {
      return Boolean(this.localFontPlan?.changed
        && this.localFontPlan.conflicts.length === 0
        && !this.localFontPlanning);
    }
    if (this.fontCreateSource === "bundled") return Boolean(this.selectedBundledFont);
    return Boolean(this.formName.trim() && this.formGoogleStyles.length > 0);
  }

  workspaceIdentity(): ProjectWorkspaceIdentity {
    const snapshot = this.workspaceMutations.snapshot;
    if (!snapshot) throw new Error(t("design-workspace-not-ready"));
    return {
      expectedProjectRoot: snapshot.projectRoot,
      expectedSessionId: snapshot.runtimeSessionId,
      expectedRevision: snapshot.revision,
    };
  }

  resetPanel() {
    this.detailMode = "info";
    this.formName = "";
    this.formWeights = "400, 700";
    this.formGoogleStyles = ["normal"];
    this.formVariableFont = false;
    this.formGoogleAxes = [];
    this.formGoogleCharacterSet = "";
    this.googleFontQuery = "";
    this.googleFontResults = [];
    this.googleFontLoading = false;
    this.googleFontError = "";
    this.googleFontSearchSequence += 1;
    this.bundledFontQuery = "";
    this.bundledFontCategory = "all";
    this.selectedBundledFontId = "";
    this.bundledFontCatalogSequence += 1;
    this.bundledFontLoading = false;
    this.bundledFontError = "";
    this.clearBundledFontPreview();
    this.fontCreateSource = "google";
    this.localFontPaths = [];
    this.localFontPlan = null;
    this.localFontPlanning = false;
    this.localFontPlanSequence += 1;
    this.fontRemovalPlan = null;
    this.fontRemovalPlanning = false;
    this.formError = "";
  }

  selectFont(id: string) {
    this.selectedFontKey = id;
    this.resetPanel();
  }

  beginCreate() {
    if (this.mutating) return;
    this.resetPanel();
    this.detailMode = "create";
    void this.searchGoogleFontCatalog("");
  }

  dispose() {
    this.clearFontPreview();
    this.clearBundledFontPreview();
  }

  clearFontPreview() {
    this.fontPreviewSequence += 1;
    this.fontPreviewLoading = false;
    this.fontPreviewError = "";
    this.fontPreviewStyle?.remove();
    this.fontPreviewStyle = null;
  }

  async loadSelectedFontPreview(file: string, workspaceRevision: number) {
    const requestId = ++this.fontPreviewSequence;
    this.fontPreviewLoading = true;
    this.fontPreviewError = "";
    this.fontPreviewStyle?.remove();
    this.fontPreviewStyle = null;
    try {
      const asset = await getFontPreviewAsset(file, this.workspaceIdentity());
      if (
        requestId !== this.fontPreviewSequence
        || this.workspaceMutations.snapshot?.revision !== workspaceRevision
        || this.selectedFontPreviewFile?.file !== file
      ) return;
      const style = document.createElement("style");
      style.dataset.panaFontPreview = asset.contentHash;
      style.textContent = `@font-face { font-family: "Pana Studio Font Preview"; src: url("${asset.dataUrl}") format("${asset.format}"); font-weight: 100 900; font-style: normal; font-display: swap; }`;
      document.head.append(style);
      this.fontPreviewStyle = style;
    } catch (cause) {
      if (requestId !== this.fontPreviewSequence) return;
      this.fontPreviewError = errorMessage(cause);
      this.fontPreviewStyle?.remove();
      this.fontPreviewStyle = null;
    } finally {
      if (requestId === this.fontPreviewSequence) this.fontPreviewLoading = false;
    }
  }

  clearBundledFontPreview() {
    this.bundledFontPreviewSequence += 1;
    this.bundledFontPreviewLoading = false;
    this.bundledFontPreviewError = "";
    this.bundledFontPreviewStyle?.remove();
    this.bundledFontPreviewStyle = null;
  }

  async loadBundledFontPreview(family: BundledFontCatalogFamily) {
    const requestId = ++this.bundledFontPreviewSequence;
    this.bundledFontPreviewLoading = true;
    this.bundledFontPreviewError = "";
    this.bundledFontPreviewStyle?.remove();
    this.bundledFontPreviewStyle = null;
    try {
      const preview = await getBundledFontPreview(family.id, "normal");
      if (requestId !== this.bundledFontPreviewSequence || this.selectedBundledFontId !== family.id) return;
      const style = document.createElement("style");
      style.dataset.panaBundledFontPreview = preview.faces.map((face) => face.contentHash).join(":");
      style.textContent = preview.faces.map((face) => (
        `@font-face { font-family: "Pana Studio Bundled Font Preview"; src: url("${face.dataUrl}") format("${face.format}"); font-weight: ${face.weightRange.start} ${face.weightRange.end}; font-style: ${face.style}; font-display: swap; unicode-range: ${face.unicodeRange}; }`
      )).join("\n");
      document.head.append(style);
      this.bundledFontPreviewStyle = style;
    } catch (cause) {
      if (requestId !== this.bundledFontPreviewSequence) return;
      this.bundledFontPreviewError = errorMessage(cause);
    } finally {
      if (requestId === this.bundledFontPreviewSequence) this.bundledFontPreviewLoading = false;
    }
  }

  async searchGoogleFontCatalog(search = this.googleFontQuery) {
    const requestId = ++this.googleFontSearchSequence;
    this.googleFontLoading = true;
    this.googleFontError = "";
    try {
      const results = await searchGoogleFonts(search.trim(), 30, 0);
      if (requestId === this.googleFontSearchSequence) this.googleFontResults = results;
    } catch (cause) {
      if (requestId !== this.googleFontSearchSequence) return;
      this.googleFontResults = [];
      this.googleFontError = errorMessage(cause);
    } finally {
      if (requestId === this.googleFontSearchSequence) this.googleFontLoading = false;
    }
  }

  async loadBundledFontCatalog() {
    if (this.bundledFontCatalog.length > 0 || this.bundledFontLoading) return;
    const requestId = ++this.bundledFontCatalogSequence;
    this.bundledFontLoading = true;
    this.bundledFontError = "";
    try {
      const catalog = await getBundledFontCatalog();
      if (requestId === this.bundledFontCatalogSequence) this.bundledFontCatalog = catalog;
    } catch (cause) {
      if (requestId === this.bundledFontCatalogSequence) this.bundledFontError = errorMessage(cause);
    } finally {
      if (requestId === this.bundledFontCatalogSequence) this.bundledFontLoading = false;
    }
  }

  selectBundledFont(font: BundledFontCatalogFamily) {
    if (this.mutating) return;
    this.selectedBundledFontId = font.id;
    this.formName = font.family;
    void this.loadBundledFontPreview(font);
  }

  selectFontCreateSource(source: FontCreateSource) {
    if (this.mutating || this.localFontPlanning || this.fontCreateSource === source) return;
    this.fontCreateSource = source;
    this.formError = "";
    if (source !== "bundled") this.clearBundledFontPreview();
    if (source === "google" && this.googleFontResults.length === 0) void this.searchGoogleFontCatalog("");
    else if (source === "bundled") void this.loadBundledFontCatalog();
  }

  async chooseAndPlanLocalFonts() {
    if (this.mutating || this.localFontPlanning) return;
    this.formError = "";
    const paths = await chooseFontFiles();
    if (paths.length === 0) return;
    const requestId = ++this.localFontPlanSequence;
    this.localFontPaths = paths;
    this.localFontPlan = null;
    this.localFontPlanning = true;
    try {
      const plan = await planLocalFontImport(paths, this.workspaceIdentity());
      if (requestId === this.localFontPlanSequence) this.localFontPlan = plan;
    } catch (cause) {
      if (requestId === this.localFontPlanSequence) this.formError = errorMessage(cause);
    } finally {
      if (requestId === this.localFontPlanSequence) this.localFontPlanning = false;
    }
  }

  availableGoogleStyles(font: GoogleFontCatalogFamily) {
    const styles: string[] = [];
    if (font.variants.some((variant) => !variant.endsWith("italic"))) styles.push("normal");
    if (font.variants.some((variant) => variant.endsWith("italic") || variant === "italic")) styles.push("italic");
    return styles.length ? styles : ["normal"];
  }

  selectGoogleFont(font: GoogleFontCatalogFamily) {
    this.formName = font.family;
    this.formVariableFont = false;
    this.formGoogleAxes = [];
    this.formGoogleCharacterSet = "";
    const preferred = [400, 700].filter((weight) => font.weights.includes(weight));
    this.formWeights = (preferred.length ? preferred : font.weights.slice(0, 2)).join(", ");
    const styles = this.availableGoogleStyles(font);
    this.formGoogleStyles = styles.includes("normal") ? ["normal"] : [styles[0] ?? "normal"];
  }

  toggleGoogleStyle(style: string) {
    const selected = new Set(this.formGoogleStyles);
    if (selected.has(style)) selected.delete(style); else selected.add(style);
    this.formGoogleStyles = ["normal", "italic"].filter((entry) => selected.has(entry));
  }

  selectedGoogleWeights() {
    return this.formWeights.split(",").map((weight) => Number.parseInt(weight.trim(), 10)).filter(Number.isInteger);
  }

  toggleGoogleWeight(weight: number) {
    const selected = new Set(this.selectedGoogleWeights());
    if (selected.has(weight)) selected.delete(weight); else selected.add(weight);
    this.formWeights = [...selected].sort((left, right) => left - right).join(", ");
  }

  setVariableFont(enabled: boolean) {
    this.formVariableFont = enabled;
    if (!enabled) { this.formGoogleAxes = []; return; }
    const axis = this.selectedGoogleFont?.axes.find((entry) => entry.tag === "wght");
    if (axis) this.formWeights = `${Math.round(axis.start)}, ${Math.round(axis.end)}`;
  }

  advancedGoogleAxes(font: GoogleFontCatalogFamily) {
    return font.axes.filter((axis) => !["ital", "wght"].includes(axis.tag.toLocaleLowerCase("en")));
  }

  toggleGoogleAxis(tag: string) {
    const selected = new Set(this.formGoogleAxes);
    if (selected.has(tag)) selected.delete(tag); else selected.add(tag);
    this.formGoogleAxes = [...selected].sort();
  }

  async installSelectedFont() {
    if (this.mutating) return;
    this.formError = "";
    this.mutating = true;
    try {
      if (this.fontCreateSource === "local") await this.installLocalFonts();
      else if (this.fontCreateSource === "bundled") await this.installBundledFont();
      else await this.installGoogleFont();
      this.resetPanel();
    } catch (cause) {
      this.formError = errorMessage(cause);
    } finally {
      this.mutating = false;
    }
  }

  private async installLocalFonts() {
    if (!this.localFontPlan || this.localFontPaths.length === 0) throw new Error(t("design-local-fonts-required"));
    if (this.localFontPlan.conflicts.length > 0) throw new Error(t("design-local-font-conflicts"));
    const receipt = await applyLocalFontImport(this.localFontPaths, this.localFontPlan.planToken, this.workspaceIdentity());
    const settlement = await this.workspaceMutations.settle(
      workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
      { warningLabel: t("design-operation-local-font-import") },
    );
    await this.manager.refresh(true);
    const installed = receipt.plan.families[0];
    if (installed) this.selectedFontKey = installed.id;
    this.globalStatus.set(settlement.warnings.length > 0
      ? t("design-local-files-warning", { count: receipt.plan.files.length })
      : t("design-local-files-success", { count: receipt.plan.files.length }), "unsaved");
  }

  private async installBundledFont() {
    const selected = this.selectedBundledFont;
    if (!selected) throw new Error(t("design-bundled-font-required"));
    const receipt = await installBundledFontFamily(selected.id, this.workspaceIdentity());
    const settlement = await this.workspaceMutations.settle(
      workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
      { warningLabel: t("design-operation-bundled-font-install") },
    );
    await this.manager.refresh(true);
    this.selectedFontKey = receipt.family.id;
    this.globalStatus.set(settlement.warnings.length > 0
      ? t("design-bundled-font-warning", { family: receipt.family.family })
      : t("design-bundled-font-success", { family: receipt.family.family }), "unsaved");
  }

  private async installGoogleFont() {
    const weights = this.selectedGoogleWeights().filter((weight) => weight >= 1 && weight <= 1000);
    if (!this.formName.trim()) throw new Error(t("design-font-family-required"));
    if (!this.formVariableFont && weights.length === 0) throw new Error(t("design-font-weight-required"));
    const receipt = await downloadGoogleFontFamily(
      this.formName.trim(), weights, this.formGoogleStyles, this.formVariableFont,
      (this.selectedGoogleFont?.axes ?? []).filter((axis) => this.formGoogleAxes.includes(axis.tag)),
      this.formGoogleCharacterSet.trim() ? this.formGoogleCharacterSet : null,
      this.workspaceIdentity(),
    );
    const settlement = await this.workspaceMutations.settle(
      workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
      { warningLabel: t("design-operation-google-font-install") },
    );
    await this.manager.refresh(true);
    this.selectedFontKey = receipt.result.family.id;
    this.globalStatus.set(settlement.warnings.length > 0
      ? t("design-google-font-warning", { family: this.formName.trim() })
      : t("design-google-font-success", { family: this.formName.trim() }), "unsaved");
  }

  async assignSelectedFontToRole(roleId: FontRoleId) {
    const selected = this.selectedFont;
    if (this.mutating || !selected) return;
    await this.mutateManager("design-operation-font-assignment", async () => {
      const receipt = await assignFontRole(roleId, selected.id, this.workspaceIdentity());
      return {
        receipt,
        success: t("design-font-assigned-success", { family: selected.family, role: receipt.role.label }),
        warning: t("design-font-assigned-warning", { family: selected.family, role: receipt.role.label }),
      };
    });
  }

  async changeSelectedFontDisplay(display: "auto" | "block" | "swap" | "fallback" | "optional") {
    const selected = this.selectedFont;
    if (this.mutating || !selected) return;
    await this.mutateManager("design-operation-font-display", async () => {
      const receipt = await setFontDisplay(selected.id, display, this.workspaceIdentity());
      return {
        receipt,
        success: t("design-font-display-success", { family: selected.family, display }),
        warning: t("design-font-display-warning", { family: selected.family, display }),
      };
    });
  }

  async toggleFontPreload(file: string, enabled: boolean) {
    if (this.mutating) return;
    await this.mutateManager("design-operation-font-preload", async () => {
      const receipt = await setFontPreload(file, enabled, this.workspaceIdentity());
      const state = enabled ? t("design-preload-enabled") : t("design-preload-disabled");
      return {
        receipt,
        success: t("design-preload-success", { state, file: file.split("/").at(-1) ?? file }),
        warning: t("design-preload-warning", { state, file: file.split("/").at(-1) ?? file }),
      };
    });
  }

  private async mutateManager(
    operationKey: string,
    action: () => Promise<{
      receipt: { mutation: Parameters<typeof workspaceMutationAuthorityReceipt>[0]; workspace: Parameters<typeof workspaceMutationAuthorityReceipt>[1]; manager: NonNullable<FontManagerState["snapshot"]> };
      success: string;
      warning: string;
    }>,
  ) {
    this.formError = "";
    this.mutating = true;
    try {
      const { receipt, success, warning } = await action();
      const settlement = await this.workspaceMutations.settle(
        workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
        { warningLabel: t(operationKey as Parameters<typeof t>[0]) },
      );
      this.manager.replace(receipt.manager);
      this.globalStatus.set(settlement.warnings.length > 0 ? warning : success, "unsaved");
    } catch (cause) {
      this.formError = errorMessage(cause);
    } finally {
      this.mutating = false;
    }
  }

  async planSelectedFontRemoval() {
    const selected = this.selectedFont;
    if (this.mutating || this.fontRemovalPlanning || !selected) return;
    const workspaceRevision = this.workspaceMutations.snapshot?.revision;
    if (workspaceRevision === undefined) return;
    this.formError = "";
    this.fontRemovalPlan = null;
    this.fontRemovalPlanning = true;
    try {
      const plan = await planFontFamilyRemoval(selected.id, this.workspaceIdentity());
      if (this.selectedFont?.id === selected.id && this.workspaceMutations.snapshot?.revision === workspaceRevision) {
        this.fontRemovalPlan = plan;
      }
    } catch (cause) {
      this.formError = errorMessage(cause);
    } finally {
      this.fontRemovalPlanning = false;
    }
  }

  async confirmSelectedFontRemoval() {
    const selected = this.selectedFont;
    if (this.mutating || !selected || !this.fontRemovalPlan) return;
    this.formError = "";
    this.mutating = true;
    try {
      const receipt = await removeFontFamily(this.fontRemovalPlan.familyId, this.fontRemovalPlan.planToken, this.workspaceIdentity());
      const settlement = await this.workspaceMutations.settle(
        workspaceMutationAuthorityReceipt(receipt.mutation, receipt.workspace),
        { warningLabel: t("design-operation-font-remove") },
      );
      this.manager.replace(receipt.manager);
      this.selectedFontKey = "";
      this.fontRemovalPlan = null;
      this.clearFontPreview();
      this.globalStatus.set(settlement.warnings.length > 0
        ? t("design-font-removed-warning", { family: selected.family })
        : t("design-font-removed-success", { family: selected.family }), "unsaved");
    } catch (cause) {
      this.formError = errorMessage(cause);
    } finally {
      this.mutating = false;
    }
  }
}
