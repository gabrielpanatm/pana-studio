import {
  CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION,
  type CssInspectorContextResolution,
  type CssRuleContext,
  type CssViewport,
  type EditableStyles,
  type ScssVariable,
} from "$lib/css/contracts";
import {
  DESIGN_CLASS_INVENTORY_SCHEMA_VERSION,
  DESIGN_CLASS_RENAME_SCHEMA_VERSION,
  type DesignClassInventorySnapshot,
  type DesignClassRenameReceipt,
  type DesignTokenCatalogSnapshot,
  type ThemeStyleCatalogSnapshot,
  type ThemeStyleDraftPreview,
  type ThemeStylePropertyInput,
  type ThemeStyleTargetSnapshot,
} from "$lib/css/design-system-contract";
import type {
  CssMutationCommandReceipt,
  PageCssWriteResult,
  ReusableCssWriteResult,
} from "$lib/css/mutation-contract";
import { t } from "$lib/i18n/runtime.svelte";
import type { SelectionMutationIdentity } from "$lib/preview/contracts";
import {
  type FileBufferCommandReceipt,
  type FileBufferRequestIdentity,
  PROJECT_WORKSPACE_SCHEMA_VERSION,
  type WorkspaceEntryMutationReceipt,
} from "$lib/project/workspace-contract";
import { invoke } from "@tauri-apps/api/core";

export type CssRequestIdentity = FileBufferRequestIdentity;

export function createCssRequestIdentity(
  projectRoot: string,
  runtimeSessionId: string,
): CssRequestIdentity {
  const expectedProjectRoot = projectRoot.trim();
  const expectedSessionId = runtimeSessionId.trim();
  if (!expectedProjectRoot || !expectedSessionId) {
    throw new Error(t("io-css-identity-invalid"));
  }
  return { expectedProjectRoot, expectedSessionId };
}

export function cssRequestIdentityMatches(
  identity: CssRequestIdentity,
  projectRoot: string,
  runtimeSessionId: string,
): boolean {
  return identity.expectedProjectRoot === projectRoot
    && identity.expectedSessionId === runtimeSessionId;
}

function requireCssIdentity(identity: CssRequestIdentity) {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    throw new Error(t("io-css-identity-invalid"));
  }
}

async function invokeBoundCss<T>(
  command: string,
  args: Record<string, unknown>,
  identity: CssRequestIdentity,
  expectedWorkspaceRevision?: number,
): Promise<T> {
  requireCssIdentity(identity);
  const receipt = await invoke<FileBufferCommandReceipt<T>>(command, { ...args, identity });
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || !Number.isSafeInteger(receipt.workspaceRevision)
    || receipt.workspaceRevision < 0
  ) {
    throw new Error(
      t("io-css-stale-receipt", {
        command,
        expectedRoot: identity.expectedProjectRoot,
        expectedSession: identity.expectedSessionId,
        actualRoot: receipt.projectRoot,
        actualSession: receipt.runtimeSessionId,
      }),
    );
  }
  if (
    expectedWorkspaceRevision !== undefined
    && receipt.workspaceRevision !== expectedWorkspaceRevision
  ) {
    throw new Error(
      t("io-css-workspace-revision-mismatch", {
        command,
        actual: receipt.workspaceRevision,
        expected: expectedWorkspaceRevision,
      }),
    );
  }
  return receipt.payload;
}

async function invokeBoundCssMutation<T>(
  command: string,
  args: Record<string, unknown>,
  identity: CssRequestIdentity,
): Promise<CssMutationCommandReceipt<T>> {
  requireCssIdentity(identity);
  const receipt = await invoke<CssMutationCommandReceipt<T>>(command, { ...args, identity });
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || !Number.isSafeInteger(receipt.workspaceRevision)
    || receipt.workspaceRevision < 0
    || receipt.workspaceRevision !== receipt.authority.revisionAfter
    || receipt.authority.projectRoot !== identity.expectedProjectRoot
    || receipt.authority.sessionId !== identity.expectedSessionId
  ) {
    throw new Error(t("io-css-foreign-session-receipt", { command }));
  }
  const authority = receipt.authority;
  if (
    !Array.isArray(authority.touchedFiles)
    || !Array.isArray(authority.writtenFiles)
    || !Array.isArray(authority.removedFiles)
    || !Array.isArray(authority.documents)
  ) {
    throw new Error(t("io-css-authority-manifests-invalid", { command }));
  }
  const sortedTouched = [...new Set(authority.touchedFiles)].sort();
  const projectedPaths = [
    ...authority.writtenFiles.map((file) => file.relativePath),
    ...authority.removedFiles,
  ].sort();
  const documentPaths = authority.documents.map((projection) => projection.relativePath);
  if (
    authority.schemaVersion !== 2
    || !authority.operationId.trim()
    || !Number.isSafeInteger(authority.revisionBefore)
    || !Number.isSafeInteger(authority.revisionAfter)
    || authority.revisionBefore < 0
    || authority.revisionAfter < 0
    || JSON.stringify(sortedTouched) !== JSON.stringify(authority.touchedFiles)
    || JSON.stringify(projectedPaths) !== JSON.stringify(authority.touchedFiles)
    || JSON.stringify(documentPaths) !== JSON.stringify(authority.touchedFiles)
  ) {
    throw new Error(t("io-css-authority-receipt-invalid", { command }));
  }
  if (
    authority.status === "noop"
    && (
      authority.revisionAfter !== authority.revisionBefore
      || authority.touchedFiles.length !== 0
      || authority.writtenFiles.length !== 0
      || authority.removedFiles.length !== 0
      || authority.documents.length !== 0
      || authority.workspaceMutation !== null
    )
  ) {
    throw new Error(t("io-css-authority-noop-effects", { command }));
  }
  if (
    authority.status === "staged"
    && (
      authority.revisionAfter !== authority.revisionBefore + 1
      || authority.touchedFiles.length === 0
      || authority.workspaceMutation?.schemaVersion !== PROJECT_WORKSPACE_SCHEMA_VERSION
      || !authority.workspaceMutation.changed
      || authority.workspaceMutation.revisionBefore !== authority.revisionBefore
      || authority.workspaceMutation.revisionAfter !== authority.revisionAfter
      || authority.workspaceMutation.dirty !== authority.dirty
      || JSON.stringify(authority.workspaceMutation.touchedFiles) !== JSON.stringify(authority.touchedFiles)
    )
  ) {
    throw new Error(t("io-css-authority-staged-mismatch", { command }));
  }
  if (authority.status !== "noop" && authority.status !== "staged") {
    throw new Error(t("io-css-authority-status-invalid", { command }));
  }
  for (const projection of authority.documents) {
    const written = authority.writtenFiles.find((file) => file.relativePath === projection.relativePath);
    const removed = authority.removedFiles.includes(projection.relativePath);
    if (projection.snapshot === null) {
      if (!removed || written) {
        throw new Error(t("io-css-authority-delete-projection-invalid", { command }));
      }
      continue;
    }
    const snapshot = projection.snapshot;
    const file = authority.workspaceMutation?.files.find(
      (candidate) => candidate.relativePath === projection.relativePath,
    );
    if (
      removed
      || !written
      || written.contents !== snapshot.text
      || snapshot.relativePath !== projection.relativePath
      || !file
      || file.currentHash !== snapshot.hash
      || file.currentBytes !== snapshot.bytes
      || file.revision !== snapshot.revision
      || file.dirty !== snapshot.dirty
    ) {
      throw new Error(t("io-css-authority-file-buffer-mismatch", { command }));
    }
  }
  return receipt;
}

export function getScssVariables(
  identity: CssRequestIdentity,
  expectedWorkspaceRevision?: number,
): Promise<ScssVariable[]> {
  return invokeBoundCss<ScssVariable[]>(
    "get_scss_variables",
    {},
    identity,
    expectedWorkspaceRevision,
  );
}

export function readDesignTokenCatalog(
  identity: CssRequestIdentity,
  expectedWorkspaceRevision?: number,
): Promise<DesignTokenCatalogSnapshot> {
  return invokeBoundCss<DesignTokenCatalogSnapshot>(
    "read_design_token_catalog",
    {},
    identity,
    expectedWorkspaceRevision,
  );
}

export function readThemeStyleCatalog(
  identity: CssRequestIdentity,
  expectedWorkspaceRevision?: number,
): Promise<ThemeStyleCatalogSnapshot> {
  return invokeBoundCss<ThemeStyleCatalogSnapshot>(
    "read_theme_style_catalog",
    {},
    identity,
    expectedWorkspaceRevision,
  );
}

export function previewThemeStyleDraft(
  targetId: string,
  properties: ThemeStylePropertyInput[],
  expectedWorkspaceRevision: number,
  identity: CssRequestIdentity,
): Promise<ThemeStyleDraftPreview> {
  return invokeBoundCss<ThemeStyleDraftPreview>(
    "preview_theme_style_draft",
    { targetId, properties, expectedWorkspaceRevision },
    identity,
    expectedWorkspaceRevision,
  );
}

export function applyThemeStyleDraft(
  targetId: string,
  properties: ThemeStylePropertyInput[],
  expectedWorkspaceRevision: number,
  identity: CssRequestIdentity,
): Promise<CssMutationCommandReceipt<ThemeStyleTargetSnapshot>> {
  return invokeBoundCssMutation<ThemeStyleTargetSnapshot>(
    "apply_theme_style_draft",
    { targetId, properties, expectedWorkspaceRevision },
    identity,
  );
}

export async function readDesignClassInventory(): Promise<DesignClassInventorySnapshot> {
  const snapshot = await invoke<DesignClassInventorySnapshot>("read_design_class_inventory");
  if (snapshot.schemaVersion !== DESIGN_CLASS_INVENTORY_SCHEMA_VERSION) {
    throw new Error(
      t("io-schema-mismatch", {
        resource: t("io-resource-design-class"),
        actual: snapshot.schemaVersion,
        expected: DESIGN_CLASS_INVENTORY_SCHEMA_VERSION,
      }),
    );
  }
  return snapshot;
}

export async function createDesignClass(
  name: string,
  relativePath: string,
  identity: CssRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  requireCssIdentity(identity);
  const receipt = await invoke<WorkspaceEntryMutationReceipt>("create_design_class", {
    name,
    relativePath,
    identity,
  });
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(
      t("io-project-file-stale-receipt", {
        operation: "create_design_class",
        expectedRoot: identity.expectedProjectRoot,
        expectedSession: identity.expectedSessionId,
        actualRoot: receipt.projectRoot,
        actualSession: receipt.runtimeSessionId,
      }),
    );
  }
  return receipt;
}

export async function renameDesignClass(
  oldName: string,
  newName: string,
  identity: CssRequestIdentity,
): Promise<DesignClassRenameReceipt> {
  requireCssIdentity(identity);
  const receipt = await invoke<DesignClassRenameReceipt>("rename_design_class", {
    oldName,
    newName,
    identity,
  });
  if (receipt.schemaVersion !== DESIGN_CLASS_RENAME_SCHEMA_VERSION) {
    throw new Error(
      t("io-schema-mismatch", {
        resource: t("io-resource-design-class-rename"),
        actual: receipt.schemaVersion,
        expected: DESIGN_CLASS_RENAME_SCHEMA_VERSION,
      }),
    );
  }
  return receipt;
}

export function setScssVariable(
  relativePath: string,
  name: string,
  value: string,
  identity: CssRequestIdentity,
): Promise<CssMutationCommandReceipt<void>> {
  return invokeBoundCssMutation<void>("set_scss_variable", { relativePath, name, value }, identity);
}

export function createScssVariable(
  relativePath: string,
  name: string,
  value: string,
  identity: CssRequestIdentity,
): Promise<CssMutationCommandReceipt<void>> {
  return invokeBoundCssMutation<void>(
    "create_scss_variable",
    { relativePath, name, value },
    identity,
  );
}

function isCssBackgroundProjection(value: unknown): value is CssRuleContext["background"] {
  if (!value || typeof value !== "object") return false;
  const background = value as {
    schemaVersion?: unknown;
    color?: unknown;
    layers?: unknown;
    shorthand?: unknown;
    opaqueProperties?: unknown;
    structurallyEditable?: unknown;
  };
  return background.schemaVersion === 1
    && (background.color === null || typeof background.color === "string")
    && Array.isArray(background.layers)
    && (background.shorthand === null || typeof background.shorthand === "string")
    && Boolean(background.opaqueProperties)
    && typeof background.opaqueProperties === "object"
    && typeof background.structurallyEditable === "boolean";
}

function isCssGridProjection(value: unknown): value is CssRuleContext["grid"] {
  if (!value || typeof value !== "object") return false;
  const grid = value as {
    schemaVersion?: unknown;
    templateColumns?: unknown;
    templateRows?: unknown;
    templateAreas?: unknown;
    opaqueProperties?: unknown;
    structurallyEditable?: unknown;
  };
  return grid.schemaVersion === 1
    && Boolean(grid.templateColumns) && typeof grid.templateColumns === "object"
    && Boolean(grid.templateRows) && typeof grid.templateRows === "object"
    && Boolean(grid.templateAreas) && typeof grid.templateAreas === "object"
    && Boolean(grid.opaqueProperties) && typeof grid.opaqueProperties === "object"
    && typeof grid.structurallyEditable === "boolean";
}

export async function resolveCssInspectorContext(options: {
  templatePath: string | null;
  selector: string;
  viewport: CssViewport;
  fallbackFile: string | null;
  expectedWorkspaceRevision: number;
  expectedSelection: SelectionMutationIdentity;
}, identity: CssRequestIdentity): Promise<CssInspectorContextResolution> {
  const resolution = await invokeBoundCss<CssInspectorContextResolution>(
    "resolve_css_inspector_context",
    options,
    identity,
    options.expectedWorkspaceRevision,
  );
  const expectedRevision = options.expectedSelection.selectionRevision;
  if (
    resolution.schemaVersion !== CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION
    || resolution.selectionRevision !== expectedRevision
    || resolution.selector !== options.selector.trim()
    || resolution.viewport !== options.viewport
    || !["existing", "creation", "ambiguous"].includes(resolution.state)
    || !Array.isArray(resolution.candidates)
  ) {
    throw new Error("[css_inspector_invalid_receipt] Rust a returnat o rezoluție CSS inconsistentă.");
  }
  for (const candidate of resolution.candidates) {
    if (
      !candidate.file
      || candidate.ruleContext.file !== candidate.file
      || candidate.ruleContext.selector !== resolution.selector
      || candidate.ruleContext.viewport !== resolution.viewport
      || !isCssBackgroundProjection(candidate.ruleContext.background)
      || !isCssGridProjection(candidate.ruleContext.grid)
    ) {
      throw new Error("[css_inspector_invalid_receipt] Candidatul CSS nu corespunde rezoluției.");
    }
  }
  if (resolution.state === "ambiguous") {
    if (
      resolution.target !== null
      || resolution.ruleContext !== null
      || resolution.candidates.length < 2
    ) {
      throw new Error("[css_inspector_invalid_receipt] Ambiguitatea CSS nu este completă.");
    }
    return resolution;
  }
  if (
    !resolution.target
    || !resolution.ruleContext
    || !Array.isArray(resolution.target.consumerFiles)
    || !Array.isArray(resolution.target.consumerTemplates)
    || resolution.target.file !== resolution.ruleContext.file
    || resolution.target.selector !== resolution.selector
    || resolution.ruleContext.selector !== resolution.selector
    || resolution.ruleContext.viewport !== resolution.viewport
    || !isCssBackgroundProjection(resolution.ruleContext.background)
    || !isCssGridProjection(resolution.ruleContext.grid)
    || (resolution.state === "existing" && resolution.candidates.length !== 1)
    || (resolution.state === "creation" && resolution.candidates.length > 1)
  ) {
    throw new Error("[css_inspector_invalid_receipt] Ținta CSS nu corespunde contextului atomic.");
  }
  return resolution;
}

export function setCssRuleAtViewport(options: {
  relativePath: string;
  selector: string;
  properties: Partial<Record<keyof EditableStyles | string, string>>;
  viewport: CssViewport;
  expectedSelection?: SelectionMutationIdentity | null;
}, identity: CssRequestIdentity): Promise<CssMutationCommandReceipt<void>> {
  return invokeBoundCssMutation<void>("set_css_rule_at_viewport", options, identity);
}

export function setPageCssRuleAtViewport(options: {
  templatePath: string;
  relativePath: string;
  selector: string;
  properties: Partial<Record<keyof EditableStyles | string, string>>;
  viewport: CssViewport;
  expectedSelection?: SelectionMutationIdentity | null;
}, identity: CssRequestIdentity): Promise<CssMutationCommandReceipt<PageCssWriteResult>> {
  return invokeBoundCssMutation<PageCssWriteResult>("set_page_css_rule_at_viewport", options, identity);
}

export function setReusableCssRuleAtViewport(options: {
  templatePath: string;
  relativePath: string;
  selector: string;
  properties: Partial<Record<keyof EditableStyles | string, string>>;
  viewport: CssViewport;
  expectedSelection?: SelectionMutationIdentity | null;
}, identity: CssRequestIdentity): Promise<CssMutationCommandReceipt<ReusableCssWriteResult>> {
  return invokeBoundCssMutation<ReusableCssWriteResult>(
    "set_reusable_css_rule_at_viewport",
    options,
    identity,
  );
}
