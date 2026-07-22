export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export type StyleRow = {
  label: string;
  value: string;
};

export type CssVariableRow = {
  name: string;
  value: string;
};

export type CssRuleMatch = {
  selector: string;
  source: string;
  media: string | null;
  declarations: number;
  kind: string;
  score: number;
};

export type CssSelectorOption = {
  selector: string;
  label: string;
  source: "class" | "compound" | "id" | "tag" | "matched";
  detail: string;
};

export type PageSection = {
  selector: string;
  label: string;
  tag: string;
  depth: number;
  sourceLocation?: SourceEditLocation | null;
  sourceId?: string | null;
  templateSourceId?: string | null;
  sessionId?: string | null;
};

export type DomNodeLink = {
  selector: string;
  label: string;
  tag: string;
};

export type SelectionInfo = {
  selector: string;
  cssSelector: string;
  domPath: string;
  tag: string;
  id: string;
  href: string;
  title: string;
  alt: string;
  classes: string[];
  text: string;
  rawText: string;
  hasChildElements: boolean;
  rect: {
    width: string;
    height: string;
    top: string;
    left: string;
  };
  styles: StyleRow[];
  variables: CssVariableRow[];
  matchedRules: CssRuleMatch[];
  imageSrc: string | null;
  zolaImage: ZolaImagePresentation | null;
  attributes: Record<string, string>;
  parentNode: DomNodeLink | null;
  childNodes: DomNodeLink[];
  sourceLocation: SourceEditLocation | null;
  sourceId: string | null;
  templateSourceId: string | null;
  sessionId: string | null;
};

export type ZolaImageOperation = "fit_width" | "fit" | "fill";
export type ZolaImageFormat = "auto" | "webp" | "avif" | "jpg" | "png";

export type ZolaImagePresentation = {
  sourceUrl: string;
  sourcePath: string;
  width: number;
  height: number | null;
  operation: ZolaImageOperation;
  format: ZolaImageFormat;
  quality: number;
};

export type PreviewSelectionState =
  | { kind: "none" }
  | {
      kind: "html";
      selector: string | null;
      sourceId: string | null;
      templateSourceId: string | null;
      sessionId: string | null;
      selection: SelectionInfo;
      editable: boolean;
    }
  | {
      kind: "tera";
      selector: string | null;
      sourceId: string;
      templateSourceId: string | null;
      origin: SourceOrigin | "current" | "unknown" | null;
      themeName: string | null;
      canSelectHtml?: boolean;
      editable: boolean;
    };

export type EditableStyles = {
  color: string;
  backgroundColor: string;
  fontSize: string;
  lineHeight: string;
  textAlign: string;
  margin: string;
  padding: string;
  borderRadius: string;
  display: string;
  flexDirection: string;
  gap: string;
  justifyContent: string;
  alignItems: string;
};

export type EditableAttributes = Record<string, string>;

export type SaveState = "idle" | "unsaved" | "saving" | "saved" | "restored" | "error";

export type InspectorPendingArea = "html" | "css" | "vars" | "js";
export type HtmlPendingArea = "tag" | "attributes" | "text" | "image" | "classes" | "structure";

export type ScssVariable = {
  name: string;
  value: string;
  file: string;
};

export type FontOrigin = "local" | "theme";

export type FontRoot = {
  relativePath: string;
  origin: FontOrigin;
  themeName: string | null;
  exists: boolean;
};

export type LocalFontFile = {
  file: string;
  fileName: string;
  sizeBytes: number;
  extension: string;
  format: string;
  weight: number | null;
  weightRange: FontWeightRange | null;
  style: string | null;
  unicodeRange: string | null;
};

export type FontWeightRange = {
  start: number;
  end: number;
};

export type LocalFontFamily = {
  family: string;
  directory: string;
  origin: FontOrigin;
  themeName: string | null;
  files: LocalFontFile[];
};

export type FontInventory = {
  roots: FontRoot[];
  families: LocalFontFamily[];
};

export type GoogleFontDownloadResult = {
  family: LocalFontFamily;
  fontFaceCss: string;
  cssUrl: string;
  variable: boolean;
};

export type GoogleFontAxis = {
  tag: string;
  start: number;
  end: number;
};

export type GoogleFontCatalogFamily = {
  family: string;
  category: string | null;
  variants: string[];
  weights: number[];
  subsets: string[];
  axes: GoogleFontAxis[];
};

export type CssProperty = {
  property: string;
  value: string;
};

export type CssRuleContext = {
  file: string;
  selector: string;
  viewport: "desktop" | "tablet" | "mobile";
  resolvedBreakpoint: string | null;
  baseRules: CssProperty[];
  viewportRules: CssProperty[];
  hasBaseRule: boolean;
  hasViewportRule: boolean;
};

export type PageCssTarget = {
  file: string;
  selector: string;
  targetKind: "existing" | "page" | "fallback" | string;
  exists: boolean;
  linked: boolean;
  href: string | null;
  templatePath: string | null;
  pageOwned: boolean;
  reason: string;
};

export type WrittenProjectFile = {
  relativePath: string;
  contents: string;
};

export type PageCssWriteResult = {
  file: string;
  href: string;
  stylesheetCreated: boolean;
  templateUpdated: boolean;
  writtenFiles: WrittenProjectFile[];
};

export type PageCssCleanupResult = {
  stylesheetDeleted: boolean;
  templateUpdated: boolean;
  writtenFiles: WrittenProjectFile[];
};

export type SiteTemplateWriteOrigin = "local" | "theme";

export type AcceptedProjectDiskManifest = {
  schemaVersion: number;
  generation: number;
  runtimeSessionId: string;
  projectRoot: string;
  manifest: ProjectDiskManifest;
};

export type SiteStructureAuthorityStatus = "noop" | "staged";

export type SiteStructureAuthorityReceipt = {
  schemaVersion: number;
  operationId: string;
  status: SiteStructureAuthorityStatus;
  projectRoot: string;
  sessionId: string;
  revisionBefore: number;
  revisionAfter: number;
  dirty: boolean;
  touchedFiles: string[];
};

export type SitePageStructureReceipt = {
  slug: string;
  contentPath: string;
  templatePath: string;
  pageTemplate: string;
  origin: SiteTemplateWriteOrigin;
  themeName: string | null;
  created: string[];
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  authority: SiteStructureAuthorityReceipt;
};

export type SiteArchiveStructureReceipt = {
  slug: string;
  contentPath: string;
  templatePath: string;
  archiveTemplate: string;
  origin: SiteTemplateWriteOrigin;
  themeName: string | null;
  created: string[];
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  authority: SiteStructureAuthorityReceipt;
};

export type SiteSingleStructureReceipt = {
  sectionSlug: string;
  itemSlug: string;
  itemPath: string;
  templatePath: string;
  singleTemplate: string;
  origin: SiteTemplateWriteOrigin;
  themeName: string | null;
  created: string[];
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  authority: SiteStructureAuthorityReceipt;
};

export type SitePartialStructureReceipt = {
  path: string;
  templateName: string;
  origin: SiteTemplateWriteOrigin;
  themeName: string | null;
  created: boolean;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  authority: SiteStructureAuthorityReceipt;
};

export type SitePartialIncludeReceipt = {
  targetFile: string;
  partialTemplateName: string;
  changed: boolean;
  includeChanged: boolean;
  partialCreated: boolean;
  partialPath: string | null;
  reason: string;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  authority: SiteStructureAuthorityReceipt;
};

export type ProjectAppConfig = {
  projectPath: string;
  cachebustAssets: boolean;
};

export type ZolaProjectSettings = {
  configPath: string;
  baseUrl: string;
  title: string;
  description: string;
  defaultLanguage: string;
  author: string;
  compileSass: boolean;
  minifyHtml: boolean;
  outputDir: string;
  generateSitemap: boolean;
  generateRobotsTxt: boolean;
  excludePaginatedPagesInSitemap: boolean;
  generateFeeds: boolean;
  feedFilenames: string[];
  feedLimit: number | null;
  renderEmoji: boolean;
  smartPunctuation: boolean;
  insertAnchorLinks: string;
  lazyAsyncImage: boolean;
  githubAlerts: boolean;
  bottomFootnotes: boolean;
  externalLinksTargetBlank: boolean;
  externalLinksNoFollow: boolean;
  externalLinksNoReferrer: boolean;
  buildSearchIndex: boolean;
  searchIndexFormat: string;
  searchIncludeTitle: boolean;
  searchIncludeDescription: boolean;
  searchIncludeDate: boolean;
  searchIncludePath: boolean;
  searchIncludeContent: boolean;
  searchTruncateContentLength: number | null;
};

export type SourceLanguage = "html" | "css" | "scss" | "js" | "markdown" | "plain";
export type CenterView = "preview" | "code" | "markdown" | "site" | "kernel";
export type ProjectPaneTab = "layers" | "files" | "page";
export type InspectorTab = "html" | "css" | "vars" | "js";

export type VersionRepositoryState =
  | "uninitialized"
  | "ready"
  | "invalid"
  | "unsupported"
  | "git_unavailable";

export type VersionFileKind =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "copied"
  | "type_changed"
  | "untracked"
  | "conflicted"
  | "unknown";

export type VersionPublicationStatus = "published" | "published_refresh_required";
export type VersionDiffKind = "unstaged" | "staged" | "commit" | "integration";
export type VersionSyncState =
  | "no_upstream"
  | "upstream_missing"
  | "unborn"
  | "up_to_date"
  | "ahead"
  | "behind"
  | "diverged";

export type VersioningSessionIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

export type VersioningMutationIdentity = VersioningSessionIdentity & {
  expectedStatusToken: string;
  expectedHeadOid: string | null;
};

export type VersionFileStatus = {
  path: string;
  originalPath: string | null;
  kind: VersionFileKind;
  indexStatus: string;
  worktreeStatus: string;
  staged: boolean;
  unstaged: boolean;
  conflicted: boolean;
};

export type VersionRemote = {
  name: string;
  fetchUrl: string;
  pushUrl: string;
  usable: boolean;
  diagnostic: string | null;
};

export type VersionBranch = {
  name: string;
  oid: string | null;
  current: boolean;
  upstreamRef: string | null;
  upstreamOid: string | null;
  ahead: number;
  behind: number;
  syncState: VersionSyncState;
};

export type VersionRemoteBranch = {
  remote: string;
  name: string;
  refName: string;
  oid: string;
};

export type VersionUpstream = {
  localBranch: string;
  remote: string;
  remoteBranch: string;
  refName: string;
  oid: string | null;
  ahead: number;
  behind: number;
  syncState: VersionSyncState;
};

export type VersioningSnapshot = {
  schemaVersion: number;
  projectRoot: string;
  repositoryRoot: string;
  repositoryState: VersionRepositoryState;
  diagnostic: string | null;
  gitVersion: string | null;
  objectFormat: string | null;
  branch: string | null;
  detachedHead: boolean;
  unbornHead: boolean;
  headOid: string | null;
  statusToken: string;
  clean: boolean;
  stagedCount: number;
  unstagedCount: number;
  conflictedCount: number;
  files: VersionFileStatus[];
  userName: string | null;
  userEmail: string | null;
  remotes: VersionRemote[];
  branches: VersionBranch[];
  remoteBranches: VersionRemoteBranch[];
  upstream: VersionUpstream | null;
  syncState: VersionSyncState;
};

export type VersionNetworkOperationKind = "fetch" | "push";
export type VersionNetworkOperationStatus =
  | "started"
  | "progress"
  | "completed"
  | "failed"
  | "cancelled";

export type VersionNetworkProgressEvent = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  operationId: string;
  kind: VersionNetworkOperationKind;
  status: VersionNetworkOperationStatus;
  message: string;
};

export type VersionNetworkReceipt = {
  schemaVersion: number;
  operationId: string;
  kind: VersionNetworkOperationKind;
  remote: string;
  branch: string | null;
  changed: boolean;
  diagnostic: string | null;
  snapshot: VersioningSnapshot;
};

export type VersionNetworkCancelReceipt = {
  schemaVersion: number;
  operationId: string;
  cancellationRequested: boolean;
};

export type VersionSyncComparison = {
  schemaVersion: number;
  localRef: string;
  upstreamRef: string;
  ahead: number;
  behind: number;
  localOnly: VersionHistoryEntry[];
  remoteOnly: VersionHistoryEntry[];
};

export type VersionIntegrationMode = "fast_forward" | "merge";
export type VersionIntegrationRelationship =
  | "same"
  | "fast_forward"
  | "local_ahead"
  | "diverged";
export type VersionIntegrationKind =
  | "fast_forward"
  | "merge_clean"
  | "merge_conflict"
  | "merge_resolved"
  | "switch_branch";

export type VersionIntegrationPlan = {
  schemaVersion: number;
  headOid: string;
  targetRef: string;
  targetOid: string;
  relationship: VersionIntegrationRelationship;
  ahead: number;
  behind: number;
  localOnly: VersionHistoryEntry[];
  targetOnly: VersionHistoryEntry[];
  fastForwardAllowed: boolean;
  mergeAllowed: boolean;
  repositoryClean: boolean;
  diagnostic: string;
};

export type VersionIntegrationStatus =
  | "applied"
  | "noop"
  | "conflict_resolution_required"
  | "recovery_required";

export type VersionIntegrationReceipt = {
  schemaVersion: number;
  status: VersionIntegrationStatus;
  projectRoot: string;
  sessionId: string;
  transactionId: string | null;
  recoveryRef: string | null;
  kind: VersionIntegrationKind | null;
  previousHeadOid: string;
  targetRef: string;
  targetOid: string;
  resultCommitOid: string | null;
  changedPaths: string[];
  conflictPaths: string[];
  diagnostic: string | null;
  snapshot: VersioningSnapshot | null;
  workspace: ProjectWorkspaceSnapshot | null;
};

export type VersionIntegrationRecoveryAction =
  | "finalize"
  | "continue"
  | "rollback"
  | "cleanup";
export type VersionIntegrationRecoveryState =
  | "ready_to_finalize"
  | "conflict_resolution"
  | "ready_to_rollback"
  | "cleanup_required"
  | "manual_review";

export type VersionIntegrationRecoveryItem = {
  transactionId: string;
  recoveryRef: string;
  kind: VersionIntegrationKind;
  previousHeadOid: string;
  targetRef: string;
  targetOid: string;
  resultCommitOid: string | null;
  conflictPaths: string[];
  state: VersionIntegrationRecoveryState;
  availableActions: VersionIntegrationRecoveryAction[];
  diagnostic: string;
};

export type VersionIntegrationRecoveryScan = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  items: VersionIntegrationRecoveryItem[];
};

export type VersionIntegrationRecoveryResolutionReceipt = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  transactionId: string;
  recoveryRef: string;
  action: VersionIntegrationRecoveryAction;
  resolved: boolean;
  diagnostic: string | null;
  snapshot: VersioningSnapshot | null;
  workspace: ProjectWorkspaceSnapshot | null;
};

export type VersioningMutationReceipt = {
  schemaVersion: number;
  changed: boolean;
  touchedPaths: string[];
  snapshot: VersioningSnapshot;
};

export type VersioningCommitReceipt = {
  schemaVersion: number;
  commitOid: string;
  parentOid: string | null;
  message: string;
  publicationStatus: VersionPublicationStatus;
  diagnostic: string | null;
  snapshot: VersioningSnapshot | null;
};

export type VersionHistoryEntry = {
  oid: string;
  shortOid: string;
  parentOids: string[];
  authorName: string;
  authorEmail: string;
  authoredAt: string;
  subject: string;
};

export type VersionHistoryPage = {
  schemaVersion: number;
  offset: number;
  limit: number;
  hasMore: boolean;
  entries: VersionHistoryEntry[];
};

export type VersionDiffInput = {
  kind: VersionDiffKind;
  path?: string | null;
  commitOid?: string | null;
  targetRef?: string | null;
  expectedTargetOid?: string | null;
};

export type VersionDiffReceipt = {
  schemaVersion: number;
  kind: VersionDiffKind;
  path: string | null;
  commitOid: string | null;
  binary: boolean;
  truncated: boolean;
  patch: string;
};

export type VersionPreviewReceipt = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  commitOid: string;
  shortOid: string;
  previewUrl: string;
  fileCount: number;
  totalBytes: number;
};

export type VersionRestoreStatus = "restored" | "noop" | "recovery_required";

export type VersionRestoreReceipt = {
  schemaVersion: number;
  status: VersionRestoreStatus;
  projectRoot: string;
  sessionId: string;
  transactionId: string | null;
  recoveryRef: string | null;
  targetCommitOid: string;
  previousHeadOid: string | null;
  restoreCommitOid: string | null;
  changedPaths: string[];
  diagnostic: string | null;
  snapshot: VersioningSnapshot | null;
  workspace: ProjectWorkspaceSnapshot | null;
};

export type VersionRestoreRecoveryAction = "finalize" | "rollback" | "cleanup";
export type VersionRestoreRecoveryState =
  | "ready_to_finalize"
  | "ready_to_rollback"
  | "cleanup_required"
  | "manual_review";

export type VersionRestoreRecoveryItem = {
  transactionId: string;
  recoveryRef: string;
  targetCommitOid: string;
  previousHeadOid: string;
  restoreCommitOid: string;
  state: VersionRestoreRecoveryState;
  availableActions: VersionRestoreRecoveryAction[];
  diagnostic: string;
};

export type VersionRestoreRecoveryScan = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  items: VersionRestoreRecoveryItem[];
};

export type VersionRestoreRecoveryResolutionReceipt = {
  schemaVersion: number;
  projectRoot: string;
  sessionId: string;
  transactionId: string;
  recoveryRef: string;
  action: VersionRestoreRecoveryAction;
  resolved: boolean;
  diagnostic: string | null;
  snapshot: VersioningSnapshot | null;
  workspace: ProjectWorkspaceSnapshot | null;
};

export type ProjectFileKind = "DIR" | "HTML" | "MD" | "CSS" | "SCSS" | "JS" | "IMAGE" | "OTHER";
export type ProjectFileRole = "page" | "template" | "style" | "script" | "asset";

export type ProjectFile = {
  name: string;
  relativePath: string;
  absolutePath: string;
  kind: ProjectFileKind;
  role: ProjectFileRole;
  previewPath: string | null;
};

export type ProjectScan = {
  root: string;
  previewBaseUrl: string | null;
  previewWarning: string | null;
  activeTheme: string | null;
  files: ProjectFile[];
  isZola: boolean;
  isEmpty: boolean;
  kernelSessionId?: string;
  acceptedDiskGeneration?: number;
  acceptedDiskManifest?: ProjectDiskManifest;
};

export type ProjectOpenRecoveryStatus =
  | "missing"
  | "restorable"
  | "decision_required"
  | "abandoned";

export type ProjectOpenRecoveryConflictReason =
  | "disk_baseline_changed"
  | "project_root_replaced"
  | "recovery_invalid";

export type ProjectOpenRecoveryAssessment = {
  schemaVersion: number;
  status: ProjectOpenRecoveryStatus;
  projectRoot: string;
  assessmentToken: string | null;
  conflictReason: ProjectOpenRecoveryConflictReason | null;
  rootIdentityChanged: boolean | null;
  recoveryRevision: number | null;
  dirtyDocumentCount: number;
  stagedBinaryResourceCount: number;
  deletedBinaryResourceCount: number;
  pageJsDraftCount: number;
  undoCount: number;
  redoCount: number;
  acceptedFileCount: number;
  currentFileCount: number;
  diagnostic: string | null;
};

export type ProjectOpenRecoveryDecisionInput = {
  action: "abandon";
  assessmentToken: string;
};

export type ProjectRootFingerprint = {
  canonicalPath: string;
  modifiedMs: number;
  size: number;
  readonly: boolean;
  unixDevice: string | null;
  unixInode: string | null;
};

export type ProjectSessionScanSummary = {
  isZola: boolean;
  isEmpty: boolean;
  activeTheme: string | null;
  fileCount: number;
  directoryCount: number;
};

export type ProjectSessionSnapshot = {
  schemaVersion: number;
  id: string;
  projectRoot: string;
  zolaRoot: string;
  sessionDir: string;
  manifestPath: string;
  openedAtMs: number;
  lastSeenAtMs: number;
  rootFingerprint: ProjectRootFingerprint;
  scanSummary: ProjectSessionScanSummary;
};

export type TextBufferLanguage =
  | "html"
  | "markdown"
  | "css"
  | "scss"
  | "java_script"
  | "toml"
  | "json"
  | "yaml"
  | "plain";

export type TextBufferRole =
  | "page"
  | "template"
  | "style"
  | "script"
  | "config"
  | "data"
  | "other";

export type FileBufferBaseline = {
  hash: string;
  modifiedMs: number;
  size: number;
  readonly: boolean;
};

export type FileBufferStoreLimits = {
  maxFiles: number;
  maxFileBytes: number;
  maxTotalBytes: number;
};

export type FileBufferRequestIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

export type ProjectWorkspaceIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  expectedRevision: number;
};

export type ProjectWorkspaceHistoryIdentity = ProjectWorkspaceIdentity & {
  expectedTransactionId: string;
};

export type WorkspaceHistoryEntrySnapshot = {
  transactionId: string;
  label: string;
  source: string;
  coalesceKey: string | null;
  createdAtMs: number;
  updatedAtMs: number;
  mutationCount: number;
  documentPaths: string[];
  topologyPaths: string[];
  pageJsPaths: string[];
  retainedBytes: number;
};

export type WorkspaceHistorySnapshot = {
  undoCount: number;
  redoCount: number;
  canUndo: boolean;
  canRedo: boolean;
  retainedBytes: number;
  retainedBytesLimit: number;
  entryLimit: number;
  nextUndo: WorkspaceHistoryEntrySnapshot | null;
  nextRedo: WorkspaceHistoryEntrySnapshot | null;
  undoEntries: WorkspaceHistoryEntrySnapshot[];
  redoEntries: WorkspaceHistoryEntrySnapshot[];
};

export type FileBufferMutationExpectation = {
  expectedRevision: number;
  expectedHash: string;
};

export type FileBufferCommandReceipt<T> = {
  projectRoot: string;
  runtimeSessionId: string;
  payload: T;
};

export type CssMutationStatus = "noop" | "staged";

export type CssMutationAuthorityReceipt = {
  schemaVersion: number;
  operationId: string;
  status: CssMutationStatus;
  projectRoot: string;
  sessionId: string;
  revisionBefore: number;
  revisionAfter: number;
  dirty: boolean;
  touchedFiles: string[];
  writtenFiles: WrittenProjectFile[];
  removedFiles: string[];
  documents: WorkspaceDocumentProjection[];
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
};

export type CssMutationCommandReceipt<T> = FileBufferCommandReceipt<T> & {
  authority: CssMutationAuthorityReceipt;
};

export type FileBufferDiagnosticSeverity = "warning" | "error";

export type FileBufferDiagnostic = {
  severity: FileBufferDiagnosticSeverity;
  code: string;
  relativePath: string | null;
  message: string;
};

export type FileBufferFileSnapshot = {
  relativePath: string;
  absolutePath: string;
  language: TextBufferLanguage;
  role: TextBufferRole;
  baseline: FileBufferBaseline;
  hasDraft: boolean;
  dirty: boolean;
  currentHash: string;
  currentBytes: number;
  revision: number;
};

export type FileBufferTextSnapshot = {
  relativePath: string;
  text: string;
  dirty: boolean;
  hash: string;
  bytes: number;
  revision: number;
};

export type FileBufferChangeCoordinateSpace = "utf16";

export type FileBufferTextChange = {
  from: number;
  to: number;
  insert: string;
};

export type FileBufferChangeSetInput = {
  relativePath: string;
  baseRevision?: number | null;
  baseHash?: string | null;
  coordinateSpace?: FileBufferChangeCoordinateSpace;
  source?: string | null;
  changes: FileBufferTextChange[];
};

export type FileBufferChangeSetResult = {
  relativePath: string;
  source: string | null;
  previousRevision: number;
  revision: number;
  previousHash: string;
  currentHash: string;
  changeCount: number;
  applied: boolean;
  file: FileBufferFileSnapshot;
};

export type FileBufferStoreSnapshot = {
  schemaVersion: number;
  sessionId: string;
  runtimeSessionId: string;
  projectRoot: string;
  loadedAtMs: number;
  fileCount: number;
  loadedFileCount: number;
  skippedFileCount: number;
  dirtyFileCount: number;
  totalLoadedBytes: number;
  limits: FileBufferStoreLimits;
  files: FileBufferFileSnapshot[];
  diagnostics: FileBufferDiagnostic[];
};

export type ProjectWorkspaceMutationReceipt = {
  schemaVersion: number;
  changed: boolean;
  revisionBefore: number;
  revisionAfter: number;
  dirty: boolean;
  transactionId: string | null;
  touchedFiles: string[];
  entry: WorkspaceHistoryEntrySnapshot | null;
  files: FileBufferFileSnapshot[];
  pageJs: PageJsDraftStageReceipt | null;
  history: WorkspaceHistorySnapshot;
};

export const PROJECT_WORKSPACE_SCHEMA_VERSION = 2;
export const PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION = 3;

export type WorkspaceEntryMutationReceipt = {
  schemaVersion: 1;
  projectRoot: string;
  runtimeSessionId: string;
  relativePath: string | null;
  mutation: ProjectWorkspaceMutationReceipt;
  workspace: ProjectWorkspaceSnapshot;
};

export type ProjectWorkspaceSnapshot = {
  schemaVersion: number;
  projectRoot: string;
  runtimeSessionId: string;
  revision: number;
  diskGeneration: number;
  dirty: boolean;
  dirtyDocumentCount: number;
  createdDocumentCount: number;
  createdDocuments: string[];
  deletedDocumentCount: number;
  deletedDocuments: string[];
  stagedBinaryResourceCount: number;
  stagedBinaryResourceBytes: number;
  stagedBinaryResources: string[];
  deletedBinaryResourceCount: number;
  deletedBinaryResources: string[];
  dirtyPageJsCount: number;
  projectModelRevision: string | null;
  projectModelSourceRevision: number | null;
  documents: FileBufferStoreSnapshot;
  pageJs: PageJsDraftStoreSnapshot;
  history: WorkspaceHistorySnapshot;
};

export const WORKBENCH_SCHEMA_VERSION = 1;
export const WORKBENCH_COMMAND_SCHEMA_VERSION = 1;

export type WorkbenchActivity =
  | "editor"
  | "site"
  | "components"
  | "design_system"
  | "assets"
  | "content"
  | "versioning"
  | "audit"
  | "publish";

export type WorkbenchSurface = "visual" | "code" | "markdown";
export type WorkbenchSplit = "none" | "vertical" | "horizontal";
export type WorkbenchGroupId = "primary" | "secondary";
export type WorkbenchBottomPanelView = "problems" | "output" | "terminal" | "timeline";
export type WorkbenchCanvasMode = "fit" | "fixed";
export type WorkbenchCanvasPreset = "desktop" | "tablet" | "mobile" | "custom";

export type WorkbenchIdentity = {
  expectedProjectRoot: string;
  expectedRuntimeSessionId: string;
  expectedRevision: number;
};

export type WorkbenchDocumentSnapshot = {
  documentId: string;
  relativePath: string;
  title: string;
  surface: WorkbenchSurface;
  pinned: boolean;
};

export type WorkbenchGroupSnapshot = {
  groupId: WorkbenchGroupId;
  documents: WorkbenchDocumentSnapshot[];
  activeDocumentId: string | null;
};

export type WorkbenchBottomPanelSnapshot = {
  open: boolean;
  activeView: WorkbenchBottomPanelView;
};

export type WorkbenchCanvasViewportSnapshot = {
  mode: WorkbenchCanvasMode;
  preset: WorkbenchCanvasPreset;
  widthPx: number;
  zoomPercent: number;
  showRulers: boolean;
};

export type WorkbenchSnapshot = {
  schemaVersion: typeof WORKBENCH_SCHEMA_VERSION;
  projectRoot: string;
  projectSessionId: string;
  runtimeSessionId: string;
  revision: number;
  activeActivity: WorkbenchActivity;
  activeGroupId: WorkbenchGroupId;
  split: WorkbenchSplit;
  splitRatioBasisPoints: number;
  canvasViewport: WorkbenchCanvasViewportSnapshot;
  groups: WorkbenchGroupSnapshot[];
  bottomPanel: WorkbenchBottomPanelSnapshot;
};

export type WorkbenchIntent =
  | {
      kind: "open_document";
      relativePath: string;
      groupId?: WorkbenchGroupId;
      surface?: WorkbenchSurface;
      pinned?: boolean;
    }
  | { kind: "activate_document"; documentId: string; groupId: WorkbenchGroupId }
  | { kind: "close_document"; documentId: string; groupId: WorkbenchGroupId }
  | {
      kind: "move_document";
      documentId: string;
      fromGroupId: WorkbenchGroupId;
      toGroupId: WorkbenchGroupId;
      index?: number;
    }
  | {
      kind: "set_document_surface";
      documentId: string;
      groupId: WorkbenchGroupId;
      surface: WorkbenchSurface;
    }
  | { kind: "set_split"; split: WorkbenchSplit }
  | {
      kind: "configure_synchronized_split";
      split: Exclude<WorkbenchSplit, "none">;
      relativePath: string;
      secondarySurface: WorkbenchSurface;
    }
  | { kind: "set_split_ratio"; ratioBasisPoints: number }
  | { kind: "set_canvas_viewport"; viewport: WorkbenchCanvasViewportSnapshot }
  | { kind: "set_activity"; activity: WorkbenchActivity }
  | {
      kind: "set_bottom_panel";
      open: boolean;
      activeView: WorkbenchBottomPanelView;
    };

export type WorkbenchCommandReceipt = {
  schemaVersion: typeof WORKBENCH_COMMAND_SCHEMA_VERSION;
  changed: boolean;
  projectRoot: string;
  runtimeSessionId: string;
  revisionBefore: number;
  revisionAfter: number;
  snapshot: WorkbenchSnapshot;
};

export const COMMAND_CENTER_SCHEMA_VERSION = 1 as const;

export type CommandCenterScope = "all" | "commands" | "files" | "symbols";

export type CommandCenterItemKind =
  | "command"
  | "activity"
  | "file"
  | "page"
  | "component"
  | "style"
  | "asset"
  | "symbol"
  | "diagnostic";

export type CommandCenterAppCommand =
  | "open_project"
  | "close_project"
  | "save"
  | "undo"
  | "redo"
  | "validate"
  | "run_external"
  | "refresh_session"
  | "rescan_project"
  | "toggle_terminal"
  | "show_problems"
  | "show_output"
  | "show_timeline"
  | "split_vertical"
  | "split_horizontal"
  | "close_split"
  | "canvas_fit"
  | "canvas_desktop"
  | "canvas_tablet"
  | "canvas_mobile"
  | "toggle_left_sidebar"
  | "toggle_inspector"
  | "toggle_theme"
  | "open_settings"
  | "open_history"
  | "show_visual"
  | "show_code"
  | "show_markdown";

export type CommandCenterAction =
  | { kind: "set_activity"; activity: WorkbenchActivity }
  | { kind: "open_document"; relativePath: string; surface: WorkbenchSurface }
  | { kind: "app_command"; command: CommandCenterAppCommand };

export type CommandCenterItem = {
  id: string;
  kind: CommandCenterItemKind;
  title: string;
  subtitle: string;
  shortcut: string | null;
  enabled: boolean;
  disabledReason: string | null;
  score: number;
  action: CommandCenterAction;
};

export type CommandCenterSearchRequest = {
  query: string;
  scope: CommandCenterScope;
  limit?: number;
  expectedProjectRoot: string | null;
  expectedSessionId: string | null;
};

export type CommandCenterSearchResponse = {
  schemaVersion: typeof COMMAND_CENTER_SCHEMA_VERSION;
  projectRoot: string | null;
  runtimeSessionId: string | null;
  query: string;
  scope: CommandCenterScope;
  totalMatches: number;
  truncated: boolean;
  results: CommandCenterItem[];
};

export type ProjectWorkspaceSaveStatus = "noop" | "saved";

export type ProjectWorkspaceSaveReceipt = {
  schemaVersion: number;
  transactionId: string | null;
  status: ProjectWorkspaceSaveStatus;
  projectRoot: string;
  runtimeSessionId: string;
  revisionBefore: number;
  revisionAfter: number;
  diskGenerationBefore: number;
  diskGenerationAfter: number;
  writtenFiles: string[];
  removedFiles: string[];
  writeReceipts: WriteReceipt[];
  acceptedManifest: ProjectDiskManifest;
  workspace: ProjectWorkspaceSnapshot;
};

export type WorkspaceHistoryDirection = "undo" | "redo";

export type WorkspaceUndoRedoReceipt = {
  schemaVersion: number;
  direction: WorkspaceHistoryDirection;
  revisionBefore: number;
  revisionAfter: number;
  dirty: boolean;
  entry: WorkspaceHistoryEntrySnapshot;
  documents: WorkspaceDocumentProjection[];
  history: WorkspaceHistorySnapshot;
};

export type WorkspaceDocumentProjection = {
  relativePath: string;
  snapshot: FileBufferTextSnapshot | null;
};

export type ProjectWorkspaceUndoRedoCommandReceipt = {
  schemaVersion: typeof PROJECT_WORKSPACE_UNDO_REDO_COMMAND_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  result: WorkspaceUndoRedoReceipt;
  workspace: ProjectWorkspaceSnapshot;
};

export type KernelDiskConflictStatus = "clean" | "info" | "warning" | "error";

export type KernelDiskConflictKind =
  | "clean"
  | "dirty_only"
  | "metadata_changed"
  | "disk_changed"
  | "missing_on_disk"
  | "readonly"
  | "not_file"
  | "oversized"
  | "unreadable"
  | "invalid_path";

export type KernelDiskConflictSummary = {
  status: KernelDiskConflictStatus;
  verdictReason: string;
  trackedFileCount: number;
  cleanCount: number;
  dirtyOnlyCount: number;
  metadataChangedCount: number;
  diskChangedCount: number;
  missingOnDiskCount: number;
  readonlyCount: number;
  notFileCount: number;
  oversizedCount: number;
  unreadableCount: number;
  invalidPathCount: number;
  conflictCount: number;
  blockingCount: number;
};

export type KernelDiskConflictFileSnapshot = {
  relativePath: string;
  absolutePath: string;
  language: TextBufferLanguage;
  role: TextBufferRole;
  status: KernelDiskConflictStatus;
  kind: KernelDiskConflictKind;
  message: string;
  baseline: FileBufferBaseline;
  disk: FileBufferBaseline | null;
  hasDraft: boolean;
  dirty: boolean;
  revision: number;
};

export type KernelDiskConflictSnapshot = {
  schemaVersion: number;
  sessionId: string;
  projectRoot: string;
  scannedAtMs: number;
  maxFileBytes: number;
  summary: KernelDiskConflictSummary;
  files: KernelDiskConflictFileSnapshot[];
};

export type KernelExternalDiskReconcileStatus =
  | "applied"
  | "noop"
  | "blocked"
  | "reload_required"
  | "stale_evidence";

export type KernelExternalDiskReconcileItemOutcome =
  | "content_rebased"
  | "metadata_refreshed"
  | "unchanged"
  | "blocked"
  | "reload_required"
  | "stale_evidence";

export type KernelExternalDiskReconcileInput = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  observedManifest: ProjectDiskManifest;
  relativePaths: string[];
  activeRelativePath?: string | null;
};

export type KernelExternalDiskReconcileItemReceipt = {
  relativePath: string;
  outcome: KernelExternalDiskReconcileItemOutcome;
  beforeRevision: number | null;
  afterRevision: number | null;
  beforeBaseline: FileBufferBaseline | null;
  observedDiskBaseline: FileBufferBaseline | null;
  beforeCurrentHash: string | null;
  afterCurrentHash: string | null;
  diagnostic: string | null;
};

export type KernelExternalDiskReconcileDiagnostic = {
  code: string;
  relativePath: string | null;
  message: string;
  blocking: boolean;
};

export type KernelExternalDiskProjectionHints = {
  projectRescan: boolean;
  sourceGraph: boolean;
  preview: boolean;
  pageJs: boolean;
  scss: boolean;
  history: boolean;
  selection: boolean;
};

export type KernelExternalDiskReconcileReceipt = {
  schemaVersion: number;
  operationId: string;
  sessionId: string;
  projectRoot: string;
  status: KernelExternalDiskReconcileStatus;
  verdictReason: string;
  startedAtMs: number;
  completedAtMs: number;
  requestedCount: number;
  targetCount: number;
  reconciledCount: number;
  metadataRefreshedCount: number;
  unchangedCount: number;
  totalBytesRead: number;
  requestedPaths: string[];
  effectivePaths: string[];
  invalidatedPaths: string[];
  blockedPaths: string[];
  reloadRequiredPaths: string[];
  historyInvalidated: boolean;
  sourceGraphInvalidated: boolean;
  activeFile: FileBufferTextSnapshot | null;
  acceptedDiskGeneration: number | null;
  workspaceRevision: number | null;
  acceptedManifest: ProjectDiskManifest | null;
  projectionHints: KernelExternalDiskProjectionHints;
  items: KernelExternalDiskReconcileItemReceipt[];
  diagnostics: KernelExternalDiskReconcileDiagnostic[];
};

export type WriteReceipt = {
  id: string;
  category: string;
  owner: string;
  operation: string;
  target: string;
  bytesWritten: number;
  startedAtMs: number;
  completedAtMs: number;
  status: string;
};

export type KernelProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult = {
  receipt: KernelProjectTransitionDecisionRetentionRecoveryReceipt;
  recoveryCoordinator: RecoveryCoordinatorScan;
};

export type ProjectWorkspaceSaveHotJournalDiskState =
  | "before_state"
  | "planned_state"
  | "mixed_state"
  | "conflict_state";

export type ProjectWorkspaceSaveHotJournalFileDiskState =
  | "before"
  | "planned"
  | "conflict"
  | "unreadable";

export type ProjectWorkspaceSaveJournalContentKind = "text" | "binary";

export type ProjectWorkspaceSaveRecoveryAction =
  | "clear_stale_journal"
  | "rollback_to_before"
  | "manual_review_mixed_state"
  | "manual_review_conflict";

export type ProjectWorkspaceSaveRecoveryPlan = {
  action: ProjectWorkspaceSaveRecoveryAction;
  canClearJournal: boolean;
  canRollback: boolean;
  summary: string;
};

export type ProjectWorkspaceSaveHotJournalFile = {
  relativePath: string;
  contentKind: ProjectWorkspaceSaveJournalContentKind;
  existedBefore: boolean;
  existsAfter: boolean;
  beforeHash: string;
  plannedHash: string | null;
  diskHash: string | null;
  diskState: ProjectWorkspaceSaveHotJournalFileDiskState;
  diagnostic: string | null;
};

export type ProjectWorkspaceSaveHotJournal = {
  schemaVersion: number;
  transactionId: string;
  path: string;
  runtimeSessionId: string;
  projectRoot: string;
  revision: number;
  preparedAtMs: number;
  touchedFiles: string[];
  fileCount: number;
  bytesBefore: number;
  diskState: ProjectWorkspaceSaveHotJournalDiskState;
  recoveryPlan: ProjectWorkspaceSaveRecoveryPlan;
  files: ProjectWorkspaceSaveHotJournalFile[];
};

export type ProjectWorkspaceSaveRecoveryReceipt = {
  schemaVersion: number;
  transactionId: string;
  action: ProjectWorkspaceSaveRecoveryAction;
  projectRoot: string;
  restoredFiles: string[];
  alreadyBeforeFiles: string[];
  journalCleared: boolean;
  writeReceipts: WriteReceipt[];
  operatorDiagnostic: string;
};

export type RecoveryCoordinatorStatus = "clean" | "needs_attention" | "unreadable";

export type RecoveryCoordinatorDiagnosticSeverity = "warning" | "error";

export type RecoveryCoordinatorDiagnostic = {
  severity: RecoveryCoordinatorDiagnosticSeverity;
  code: string;
  transactionId: string | null;
  message: string;
};

export type RecoveryJournalFamily =
  | "project_workspace_save"
  | "project_transition_decision_retention";

export type RecoveryJournalFamilyStatus = "needs_attention" | "manual_review_required";

export type RecoveryJournalValueCount = {
  value: string;
  count: number;
};

export type RecoveryJournalFamilySummary = {
  family: RecoveryJournalFamily;
  status: RecoveryJournalFamilyStatus;
  label: string;
  count: number;
  clearableCount: number;
  rollbackCount: number;
  restoreCount: number;
  manualReviewCount: number;
  newestCreatedAtMs: number | null;
  stateCounts: RecoveryJournalValueCount[];
  actionCounts: RecoveryJournalValueCount[];
};

export type RecoveryCoordinatorScan = {
  schemaVersion: number;
  sessionId: string;
  projectRoot: string;
  scannedAtMs: number;
  status: RecoveryCoordinatorStatus;
  hotProjectWorkspaceSaveJournals: ProjectWorkspaceSaveHotJournal[];
  hotProjectTransitionDecisionRetentionJournals: KernelProjectTransitionDecisionRetentionHotJournal[];
  hotJournalFamilies: RecoveryJournalFamilySummary[];
  diagnostics: RecoveryCoordinatorDiagnostic[];
};

export type ProjectWorkspaceSaveRecoveryCommandResult = {
  receipt: ProjectWorkspaceSaveRecoveryReceipt;
  recoveryCoordinator: RecoveryCoordinatorScan;
  workspace: ProjectWorkspaceSnapshot;
};

export type KernelLogLevel = "info" | "warn" | "error";

export type KernelObservabilityLogSourceFilter = "all" | "active" | "archives";

export type KernelObservabilityHealthStatus = "clean" | "warning" | "error";

export type KernelObservabilityLevelCounts = {
  info: number;
  warn: number;
  error: number;
};

export type KernelObservabilitySourceCounts = {
  active: number;
  archived: number;
};

export type KernelObservabilityHealthProblemSnapshot = {
  eventId: string;
  eventName: string;
  owner: string;
  level: KernelLogLevel;
  severityText: string;
  timestampMs: number;
  message: string;
  sourceLabel: string;
};

export type KernelObservabilityModuleHealthSnapshot = {
  owner: string;
  status: KernelObservabilityHealthStatus;
  eventCount: number;
  recoveryCount: number;
  levelCounts: KernelObservabilityLevelCounts;
  latestEventName: string | null;
  latestTimestampMs: number | null;
  latestSeverityText: string | null;
};

export type KernelObservabilityHealthSnapshot = {
  status: KernelObservabilityHealthStatus;
  eventCount: number;
  recoveryCount: number;
  levelCounts: KernelObservabilityLevelCounts;
  sourceCounts: KernelObservabilitySourceCounts;
  moduleCount: number;
  modules: KernelObservabilityModuleHealthSnapshot[];
  latestProblem: KernelObservabilityHealthProblemSnapshot | null;
};

export type KernelObservabilityLogEvent = {
  schemaVersion: number;
  id: string;
  timestampMs: number;
  observedTimestampMs: number;
  level: KernelLogLevel;
  severityText: string;
  severityNumber: number;
  kind: string;
  eventName: string;
  owner: string;
  category: string;
  operation: string;
  target: string | null;
  message: string;
  diagnostic: string | null;
  attributes: Record<string, JsonValue>;
  source: KernelObservabilityLogEventSourceSnapshot;
};

export type KernelLogArchiveSnapshot = {
  index: number;
  path: string;
  exists: boolean;
  bytes: number;
};

export type KernelLogRetentionSnapshot = {
  maxActiveBytes: number;
  archiveCount: number;
  archivedCount: number;
  archivedBytes: number;
  totalRetainedBytes: number;
  archives: KernelLogArchiveSnapshot[];
};

export type KernelObservabilityLogSourceSnapshot = {
  path: string;
  archiveIndex: number | null;
  exists: boolean;
  truncated: boolean;
  scannedBytes: number;
  scannedLineCount: number;
  unreadableCount: number;
};

export type KernelObservabilityLogEventSourceSnapshot = {
  path: string;
  archiveIndex: number | null;
  label: string;
  active: boolean;
};

export type KernelObservabilityLogSnapshot = {
  schemaVersion: number;
  logPath: string;
  logExists: boolean;
  truncated: boolean;
  scannedBytes: number;
  scannedLineCount: number;
  returnedCount: number;
  unreadableCount: number;
  recoveryOnly: boolean;
  includeArchives: boolean;
  levels: KernelLogLevel[];
  eventNames: string[];
  sourceFilter: KernelObservabilityLogSourceFilter;
  limit: number;
  retention: KernelLogRetentionSnapshot;
  health: KernelObservabilityHealthSnapshot;
  sources: KernelObservabilityLogSourceSnapshot[];
  events: KernelObservabilityLogEvent[];
  diagnostics: string[];
};

export type WriteAuthorityWalPhase =
  | "preparing"
  | "prepared"
  | "auxiliary_durable"
  | "effect_visible"
  | "target_durable";

export type WriteAuthorityRecoveryClassification =
  | "no_effect"
  | "staged_only"
  | "effect_committed"
  | "rollback_completed"
  | "cleanup_required"
  | "partial_append"
  | "partial_namespace_creation"
  | "partial_tree_removal"
  | "conflict"
  | "unreadable_or_corrupt";

export type WriteAuthorityRecoveryResolutionAction =
  | "restore_original"
  | "accept_restored_state"
  | "accept_current_state"
  | "continue_tree_removal"
  | "restore_remaining_tree";

export type WriteAuthorityRecoveryResolutionInput = {
  operationId: string;
  expectedPhase: WriteAuthorityWalPhase;
  evidenceHash: string;
  action: WriteAuthorityRecoveryResolutionAction;
};

export type WriteAuthorityRecoveryItem = {
  fileName: string;
  operationId: string | null;
  phase: WriteAuthorityWalPhase | null;
  classification: WriteAuthorityRecoveryClassification;
  automaticRecoveryAvailable: boolean;
  evidenceHash: string | null;
  availableResolutionActions: WriteAuthorityRecoveryResolutionAction[];
  diagnostic: string;
};

export type WriteAuthorityRecoveryScan = {
  schemaVersion: number;
  scannedAtMs: number;
  blocked: boolean;
  recordCount: number;
  totalBytes: number;
  items: WriteAuthorityRecoveryItem[];
};

export type WriteAuthorityRecoveryResolutionReceipt = {
  schemaVersion: number;
  operationId: string;
  action: WriteAuthorityRecoveryResolutionAction;
  diagnostic: string;
  recoveryScan: WriteAuthorityRecoveryScan;
};

export type KernelProjectStateStatus = "idle" | "clean" | "info" | "dirty" | "warning" | "blocked";

export type KernelProjectStateReason =
  | "no_project"
  | "project_session_missing"
  | "project_workspace_missing"
  | "disk_conflict_snapshot_missing"
  | "disk_unverifiable"
  | "disk_conflict"
  | "workspace_dirty"
  | "metadata_changed"
  | "clean";

export type KernelProjectStateSnapshot = {
  schemaVersion: number;
  status: KernelProjectStateStatus;
  reason: KernelProjectStateReason;
  verdictReason: string;
  projectOpen: boolean;
  sessionId: string | null;
  projectRoot: string | null;
  isClean: boolean;
  writeBlocked: boolean;
  projectWorkspaceAvailable: boolean;
  diskConflictSnapshotAvailable: boolean;
  workspaceDirty: boolean;
  workspaceRevision: number | null;
  workspaceDiskGeneration: number | null;
  workspaceDirtyResourceCount: number;
  workspaceDirtyDocumentCount: number;
  workspaceCreatedDocumentCount: number;
  workspaceDeletedDocumentCount: number;
  workspaceDirtyPageJsCount: number;
  workspaceUndoCount: number;
  workspaceRedoCount: number;
  dirtyOnlyCount: number;
  metadataChangedCount: number;
  diskConflictCount: number;
  diskBlockingCount: number;
  unreadableFileCount: number;
};

export type KernelProjectTransitionAction = "open_project" | "reload_project" | "close_project";

export type KernelProjectTransitionDecision = "allow" | "confirm" | "block";

export type KernelProjectTransitionReason =
  | "no_open_project"
  | "clean"
  | "metadata_changed"
  | "workspace_dirty"
  | "disk_conflict"
  | "blocked_project_state"
  | "unknown_warning";

export type KernelProjectTransitionPolicy = {
  schemaVersion: number;
  action: KernelProjectTransitionAction;
  decision: KernelProjectTransitionDecision;
  reason: KernelProjectTransitionReason;
  projectStateStatus: KernelProjectStateStatus;
  projectStateReason: KernelProjectStateReason;
  projectRoot: string | null;
  sessionId: string | null;
  requiresOperatorConfirmation: boolean;
  blocksTransition: boolean;
  title: string;
  message: string;
  evidence: string;
  recommendedAction: string;
  workspaceDirtyResourceCount: number;
  workspaceRevision: number | null;
  workspaceUndoCount: number;
  workspaceRedoCount: number;
  diskConflictCount: number;
  diskBlockingCount: number;
  metadataChangedCount: number;
};

export type KernelProjectTransitionPolicyMatrixSnapshot = {
  schemaVersion: number;
  projectState: KernelProjectStateSnapshot;
  policies: KernelProjectTransitionPolicy[];
};

export type KernelProjectTransitionBlockedCause =
  | "disk_conflict"
  | "workspace_dirty"
  | "blocked_project_state"
  | "unknown";

export type KernelProjectTransitionResolutionSurface =
  | "disk_conflict"
  | "project_workspace"
  | "overview"
  | "observability";

export type KernelProjectTransitionBlockedHealthStatus =
  | "clean"
  | "recently_blocked"
  | "repeatedly_blocked"
  | "degraded";

export type KernelProjectTransitionBlockedHealthSnapshot = {
  schemaVersion: number;
  status: KernelProjectTransitionBlockedHealthStatus;
  recordCount: number;
  actionCount: number;
  repeatedActionCount: number;
  causeCount: number;
  repeatedCauseCount: number;
  latestRecordId: string | null;
  latestAction: KernelProjectTransitionAction | null;
  latestBlockedAtMs: number | null;
  summary: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionBlockedCauseSummary = {
  schemaVersion: number;
  cause: KernelProjectTransitionBlockedCause;
  surface: KernelProjectTransitionResolutionSurface;
  count: number;
  latestBlockedAtMs: number;
  latestRecordId: string | null;
  recordIds: string[];
  title: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionBlockedActionSummary = {
  schemaVersion: number;
  action: KernelProjectTransitionAction;
  count: number;
  latestRecordId: string;
  latestBlockedAtMs: number;
  cause: KernelProjectTransitionBlockedCause;
  surface: KernelProjectTransitionResolutionSurface;
  decision: KernelProjectTransitionDecision | null;
  reason: KernelProjectTransitionReason | null;
  projectStateStatus: KernelProjectStateStatus | null;
  projectStateReason: KernelProjectStateReason | null;
  currentProjectRoot: string | null;
  targetProjectRoot: string | null;
  sessionId: string | null;
  title: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionBlockedRecord = {
  schemaVersion: number;
  id: string;
  blockedAtMs: number;
  sourceLabel: string;
  action: KernelProjectTransitionAction | null;
  decision: KernelProjectTransitionDecision | null;
  reason: KernelProjectTransitionReason | null;
  projectStateStatus: KernelProjectStateStatus | null;
  projectStateReason: KernelProjectStateReason | null;
  currentProjectRoot: string | null;
  targetProjectRoot: string | null;
  sessionId: string | null;
  operation: string;
  target: string | null;
  message: string;
  diagnostic: string | null;
  workspaceDirtyResourceCount: number;
  workspaceRevision: number | null;
  workspaceUndoCount: number;
  workspaceRedoCount: number;
  diskConflictCount: number;
  diskBlockingCount: number;
};

export type KernelProjectTransitionBlockedAuditSnapshot = {
  schemaVersion: number;
  logPath: string;
  logExists: boolean;
  truncated: boolean;
  scannedLineCount: number;
  unreadableCount: number;
  matchingEventCount: number;
  returnedCount: number;
  includeArchives: boolean;
  sourceFilter: KernelObservabilityLogSourceFilter;
  health: KernelProjectTransitionBlockedHealthSnapshot;
  latestByAction: KernelProjectTransitionBlockedActionSummary[];
  causes: KernelProjectTransitionBlockedCauseSummary[];
  records: KernelProjectTransitionBlockedRecord[];
  diagnostics: string[];
};

export type KernelProjectTransitionDecisionKind =
  | "discard_local_drafts_for_transition"
  | "acknowledge_dirty_history_for_transition"
  | "discard_session_for_external_reload";

export type KernelProjectTransitionDecisionJournalHealthStatus =
  | "clean"
  | "has_decisions"
  | "integrity_warning"
  | "degraded";

export type KernelProjectTransitionDecisionReuseStatus =
  | "no_decisions"
  | "exact_evidence_only"
  | "repeated_context"
  | "blocked_by_integrity";

export type KernelProjectTransitionDecisionRecoveryPlanStatus =
  | "clean_noop"
  | "verified_audit"
  | "retention_review"
  | "integrity_blocked";

export type KernelProjectTransitionDecisionRecoveryAckKind =
  | "acknowledge_integrity_blocked"
  | "acknowledge_retention_review";

export type KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus =
  | "clean"
  | "has_acknowledgements"
  | "integrity_warning"
  | "degraded";

export type KernelProjectTransitionDecisionRecoveryIssueKind =
  | "read_diagnostic"
  | "invalid_evidence_hash"
  | "duplicate_decision_id"
  | "superseded_record";

export type KernelProjectTransitionDecisionRecoveryIssueSeverity = "info" | "warning" | "error";

export type KernelProjectTransitionDirtyFileEvidence = {
  relativePath: string;
  baselineHash: string;
  currentHash: string;
  currentBytes: number;
  revision: number;
};

export type KernelProjectTransitionDiskFileEvidence = {
  relativePath: string;
  kind: string;
  baselineHash: string;
  diskHash: string | null;
  revision: number;
};

export type KernelProjectTransitionWorkspaceEvidence = {
  revision: number;
  diskGeneration: number;
  dirty: boolean;
  dirtyDocumentCount: number;
  createdDocumentCount: number;
  deletedDocumentCount: number;
  dirtyPageJsCount: number;
  undoCount: number;
  redoCount: number;
  fingerprint: string;
};

export type KernelProjectTransitionDecisionEvidence = {
  schemaVersion: number;
  action: KernelProjectTransitionAction;
  targetProjectRoot: string;
  sessionId: string;
  projectRoot: string;
  projectStateStatus: KernelProjectStateStatus;
  projectStateReason: KernelProjectStateReason;
  transitionDecision: KernelProjectTransitionDecision;
  transitionReason: KernelProjectTransitionReason;
  workspaceDirtyResourceCount: number;
  dirtyFiles: KernelProjectTransitionDirtyFileEvidence[];
  diskFiles: KernelProjectTransitionDiskFileEvidence[];
  workspace: KernelProjectTransitionWorkspaceEvidence;
};

export type KernelProjectTransitionDecisionRecord = {
  schemaVersion: number;
  id: string;
  decidedAtMs: number;
  decisionKind: KernelProjectTransitionDecisionKind;
  diagnostic: string;
  evidenceHash: string;
  evidence: KernelProjectTransitionDecisionEvidence;
};

export type KernelProjectTransitionDecisionReceipt = {
  schemaVersion: number;
  decision: KernelProjectTransitionDecisionRecord;
};

export type KernelProjectTransitionDecisionRecoveryAckEvidence = {
  schemaVersion: number;
  sessionId: string;
  projectRoot: string;
  decisionJournalPath: string;
  recoveryPlanEvidenceHash: string;
  recoveryPlanStatus: KernelProjectTransitionDecisionRecoveryPlanStatus;
  integrityTrusted: boolean;
  recordCount: number;
  readDiagnosticCount: number;
  invalidEvidenceHashCount: number;
  duplicateIdCount: number;
  supersededRecordCount: number;
  retentionCandidateCount: number;
  issueCount: number;
};

export type KernelProjectTransitionDecisionRecoveryAckRecord = {
  schemaVersion: number;
  id: string;
  acknowledgedAtMs: number;
  ackKind: KernelProjectTransitionDecisionRecoveryAckKind;
  diagnostic: string;
  evidenceHash: string;
  evidence: KernelProjectTransitionDecisionRecoveryAckEvidence;
};

export type KernelProjectTransitionDecisionRecoveryAckReceipt = {
  schemaVersion: number;
  acknowledgement: KernelProjectTransitionDecisionRecoveryAckRecord;
};

export type KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot = {
  schemaVersion: number;
  status: KernelProjectTransitionDecisionRecoveryAckJournalHealthStatus;
  recordCount: number;
  returnedCount: number;
  diagnosticCount: number;
  invalidEvidenceHashCount: number;
  duplicateIdCount: number;
  latestRecordId: string | null;
  latestAcknowledgedAtMs: number | null;
  latestAckKind: KernelProjectTransitionDecisionRecoveryAckKind | null;
  latestRecoveryPlanEvidenceHash: string | null;
  summary: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionDecisionRecoveryAckJournalSnapshot = {
  schemaVersion: number;
  path: string;
  health: KernelProjectTransitionDecisionRecoveryAckJournalHealthSnapshot;
  recordCount: number;
  returnedCount: number;
  records: KernelProjectTransitionDecisionRecoveryAckRecord[];
  diagnostics: string[];
};

export type KernelProjectTransitionDecisionRetentionStatus =
  | "clean_noop"
  | "committed"
  | "recovery_attention";

export type KernelProjectTransitionDecisionRetentionHotJournalDiskState =
  | "no_effect"
  | "completed_retention"
  | "partial_retention"
  | "conflict_state";

export type KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction =
  | "clear_no_effect_journal"
  | "clear_completed_journal"
  | "restore_before_journal"
  | "manual_review_conflict";

export type KernelProjectTransitionDecisionRetentionReceipt = {
  schemaVersion: number;
  retentionId: string;
  sessionId: string;
  decisionJournalPath: string;
  archivePath: string | null;
  hotJournalPath: string | null;
  status: KernelProjectTransitionDecisionRetentionStatus;
  startedAtMs: number;
  completedAtMs: number;
  acknowledgementId: string;
  recoveryPlanEvidenceHash: string;
  diagnostic: string;
  candidateRecordIds: string[];
  beforeJournalHash: string;
  afterJournalHash: string;
  archiveHash: string;
  hotJournalWritten: boolean;
  archiveWritten: boolean;
  activeJournalWritten: boolean;
  hotJournalCleared: boolean;
  retentionCandidateCount: number;
  archivedRecordCount: number;
  keptRecordCount: number;
  writeReceipts: WriteReceipt[];
  recoveryDiagnostic: string | null;
};

export type KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan = {
  action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction;
  title: string;
  summary: string;
  requiredChecks: string[];
  canClearJournal: boolean;
  canRestoreBeforeJournal: boolean;
};

export type KernelProjectTransitionDecisionRetentionHotJournal = {
  schemaVersion: number;
  retentionId: string;
  path: string;
  sessionId: string;
  projectRoot: string;
  decisionJournalPath: string;
  archivePath: string;
  createdAtMs: number;
  acknowledgementId: string;
  recoveryPlanEvidenceHash: string;
  candidateRecordIds: string[];
  candidateCount: number;
  archivedRecordCount: number;
  keptRecordCount: number;
  beforeJournalHash: string;
  afterJournalHash: string;
  archiveHash: string;
  currentJournalHash: string | null;
  archiveDiskHash: string | null;
  diskState: KernelProjectTransitionDecisionRetentionHotJournalDiskState;
  recoveryPlan: KernelProjectTransitionDecisionRetentionHotJournalRecoveryPlan;
  diagnostics: string[];
};

export type KernelProjectTransitionDecisionRetentionRecoveryReceipt = {
  schemaVersion: number;
  retentionId: string;
  action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction;
  journalPath: string;
  decisionJournalPath: string;
  archivePath: string;
  diskStateBefore: KernelProjectTransitionDecisionRetentionHotJournalDiskState;
  journalCleared: boolean;
  restoredBeforeJournal: boolean;
  candidateCount: number;
  archivedRecordCount: number;
  keptRecordCount: number;
  operatorDiagnostic: string;
  writeReceipts: WriteReceipt[];
};

export type KernelProjectTransitionDecisionJournalHealthSnapshot = {
  schemaVersion: number;
  status: KernelProjectTransitionDecisionJournalHealthStatus;
  recordCount: number;
  returnedCount: number;
  diagnosticCount: number;
  invalidEvidenceHashCount: number;
  duplicateIdCount: number;
  latestRecordId: string | null;
  latestDecidedAtMs: number | null;
  latestDecisionKind: KernelProjectTransitionDecisionKind | null;
  summary: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionDecisionActionSummary = {
  schemaVersion: number;
  action: KernelProjectTransitionAction;
  count: number;
  latestRecordId: string;
  latestDecidedAtMs: number;
  latestDecisionKind: KernelProjectTransitionDecisionKind;
  latestTransitionReason: KernelProjectTransitionReason;
  latestProjectStateStatus: KernelProjectStateStatus;
  latestProjectStateReason: KernelProjectStateReason;
  latestTargetProjectRoot: string;
  latestSessionId: string;
  title: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionDecisionKindSummary = {
  schemaVersion: number;
  decisionKind: KernelProjectTransitionDecisionKind;
  count: number;
  latestRecordId: string;
  latestDecidedAtMs: number;
  latestAction: KernelProjectTransitionAction;
  latestTransitionReason: KernelProjectTransitionReason;
  latestTargetProjectRoot: string;
  title: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionDecisionReuseContextSummary = {
  schemaVersion: number;
  action: KernelProjectTransitionAction;
  decisionKind: KernelProjectTransitionDecisionKind;
  targetProjectRoot: string;
  count: number;
  latestRecordId: string;
  latestDecidedAtMs: number;
  latestTransitionReason: KernelProjectTransitionReason;
  supersededRecordIds: string[];
  title: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionDecisionReuseGuidanceSnapshot = {
  schemaVersion: number;
  status: KernelProjectTransitionDecisionReuseStatus;
  exactEvidenceOnly: boolean;
  blockedByIntegrity: boolean;
  recordCount: number;
  contextCount: number;
  repeatedContextCount: number;
  supersededRecordCount: number;
  latestContextRecordId: string | null;
  latestDecidedAtMs: number | null;
  summary: string;
  detail: string;
  recommendedAction: string;
  contexts: KernelProjectTransitionDecisionReuseContextSummary[];
};

export type KernelProjectTransitionDecisionRecoveryIssue = {
  schemaVersion: number;
  kind: KernelProjectTransitionDecisionRecoveryIssueKind;
  severity: KernelProjectTransitionDecisionRecoveryIssueSeverity;
  recordId: string | null;
  count: number;
  title: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionDecisionRetentionCandidate = {
  schemaVersion: number;
  recordId: string;
  supersededByRecordId: string;
  action: KernelProjectTransitionAction;
  decisionKind: KernelProjectTransitionDecisionKind;
  targetProjectRoot: string;
  decidedAtMs: number;
  transitionReason: KernelProjectTransitionReason;
  title: string;
  detail: string;
  recommendedAction: string;
};

export type KernelProjectTransitionDecisionRecoveryPlanSnapshot = {
  schemaVersion: number;
  evidenceHash: string;
  status: KernelProjectTransitionDecisionRecoveryPlanStatus;
  readOnly: boolean;
  mutationAllowed: boolean;
  integrityTrusted: boolean;
  recordCount: number;
  readDiagnosticCount: number;
  invalidEvidenceHashCount: number;
  duplicateIdCount: number;
  supersededRecordCount: number;
  retentionCandidateCount: number;
  issueCount: number;
  summary: string;
  detail: string;
  recommendedAction: string;
  issues: KernelProjectTransitionDecisionRecoveryIssue[];
  retentionCandidates: KernelProjectTransitionDecisionRetentionCandidate[];
};

export type KernelProjectTransitionDecisionJournalSnapshot = {
  schemaVersion: number;
  path: string;
  health: KernelProjectTransitionDecisionJournalHealthSnapshot;
  latestByAction: KernelProjectTransitionDecisionActionSummary[];
  byDecisionKind: KernelProjectTransitionDecisionKindSummary[];
  reuseGuidance: KernelProjectTransitionDecisionReuseGuidanceSnapshot;
  recoveryPlan: KernelProjectTransitionDecisionRecoveryPlanSnapshot;
  recordCount: number;
  returnedCount: number;
  records: KernelProjectTransitionDecisionRecord[];
  diagnostics: string[];
};

export type SourcePageKind = "page" | "section" | "home";
export type SourceStyleScope = "global" | "page" | "partial" | "other";
export type SourceOrigin = "local" | "theme";
export type SourceNodeKind =
  | "page"
  | "template"
  | "partial"
  | "style"
  | "script"
  | "asset"
  | "dataFile"
  | "html"
  | "extends"
  | "block"
  | "include"
  | "import"
  | "macro"
  | "for"
  | "if"
  | "set"
  | "with"
  | "teraVariable"
  | "teraComment"
  | "raw"
  | "tera";
export type SourceRelationKind =
  | "pageTemplate"
  | "sectionPageTemplate"
  | "getsPage"
  | "getsSection"
  | "internalContentLink"
  | "assetUrl"
  | "assetHash"
  | "dataLoad"
  | "dataFileLoad"
  | "contentDataLoad"
  | "imageMetadata"
  | "imageResize"
  | "extends"
  | "includes"
  | "imports"
  | "definesBlock"
  | "overridesBlock"
  | "usesStyle"
  | "usesScript";
export type SourceDiagnosticSeverity = "warning" | "error";

export type SourceRange = {
  start: number;
  end: number;
  line: number;
  column: number;
  endLine: number;
  endColumn: number;
};

export type SourceCapabilities = {
  canOpenInCode: boolean;
  canEditVisual: boolean;
  canEditText: boolean;
  canEditAttributes: boolean;
  canMove: boolean;
  canExtractPartial: boolean;
  reason: string | null;
};

export type SourceEditLocation = {
  file: string;
  line: number;
  column?: number;
};

export type SourceEditTarget = {
  sourceId: string;
  file: string;
  location: SourceEditLocation;
  range: SourceRange;
  kind: SourceNodeKind;
  label: string;
  capabilities: SourceCapabilities;
};

export type SourceGraphNode = {
  id: string;
  kind: SourceNodeKind;
  file: string;
  origin: SourceOrigin;
  themeName: string | null;
  label: string;
  range: SourceRange | null;
  parent: string | null;
  children: string[];
  capabilities: SourceCapabilities;
};

export type SourceGraphRelation = {
  id: string;
  from: string;
  to: string;
  kind: SourceRelationKind;
  label: string;
};

export type SourceGraphDiagnostic = {
  severity: SourceDiagnosticSeverity;
  message: string;
  file: string | null;
  range: SourceRange | null;
};

export type SourceGraphPage = {
  id: string;
  file: string;
  title: string;
  url: string;
  pageKind: SourcePageKind;
  frontmatterTemplate: string | null;
  frontmatterPageTemplate: string | null;
  resolvedTemplate: string | null;
  contentNodeId: string;
  templateNodeId: string | null;
  pageTemplateNodeId: string | null;
};

export type SourceGraphTemplate = {
  id: string;
  file: string;
  name: string;
  origin: SourceOrigin;
  themeName: string | null;
  isPartial: boolean;
  extends: string | null;
  includes: string[];
  imports: string[];
  getPages: string[];
  getSections: string[];
  internalLinks: string[];
  assetUrls: string[];
  assetHashes: string[];
  dataLoads: string[];
  imageMetadata: string[];
  imageResizes: string[];
  blocks: string[];
  macros: string[];
  nodeId: string;
};

export type SourceGraphAsset = {
  id: string;
  file: string;
  origin: SourceOrigin;
  themeName: string | null;
  logicalPath: string;
  nodeId: string;
};

export type SourceGraphScript = {
  id: string;
  file: string;
  origin: SourceOrigin;
  themeName: string | null;
  logicalPath: string;
  nodeId: string;
};

export type SourceGraphDataFile = {
  id: string;
  file: string;
  origin: SourceOrigin;
  themeName: string | null;
  logicalPath: string;
  nodeId: string;
};

export type SourceGraphStyle = {
  id: string;
  file: string;
  origin: SourceOrigin;
  themeName: string | null;
  scope: SourceStyleScope;
  nodeId: string;
};

export type SourceGraph = {
  projectRoot: string;
  zolaRoot: string;
  activeTheme: string | null;
  pages: SourceGraphPage[];
  templates: SourceGraphTemplate[];
  styles: SourceGraphStyle[];
  scripts: SourceGraphScript[];
  assets: SourceGraphAsset[];
  dataFiles: SourceGraphDataFile[];
  nodes: SourceGraphNode[];
  relations: SourceGraphRelation[];
  diagnostics: SourceGraphDiagnostic[];
};

export type ProjectModelFileKind =
  | "config"
  | "content"
  | "template"
  | "style"
  | "script"
  | "data"
  | "staticText"
  | "otherText";

export type ProjectModelFileSummary = {
  relativePath: string;
  kind: ProjectModelFileKind;
  sizeBytes: number;
  revision: string;
  fromDraft: boolean;
};

export type ProjectModelDiagnosticSeverity = "warning" | "error";

export type ProjectModelDiagnostic = {
  severity: ProjectModelDiagnosticSeverity;
  message: string;
  file: string | null;
  range: SourceRange | null;
};

export type TeraGraphRelationKind =
  | "contains"
  | "extends"
  | "includes"
  | "imports"
  | "definesBlock"
  | "definesMacro";

export type TeraGraphTemplate = {
  file: string;
  name: string;
  origin: SourceOrigin;
  themeName: string | null;
  isPartial: boolean;
  sourceGraphTemplateId: string;
  sourceGraphNodeId: string;
  rootNodeId: string;
  extends: string | null;
  includes: string[];
  imports: string[];
  blocks: string[];
  macros: string[];
};

export type TeraGraphNode = {
  id: string;
  kind: SourceNodeKind;
  file: string;
  label: string;
  target: string | null;
  range: SourceRange | null;
  parent: string | null;
  children: string[];
  capabilities: SourceCapabilities;
};

export type TeraGraphRelation = {
  id: string;
  from: string;
  to: string;
  kind: TeraGraphRelationKind;
  label: string;
};

export type TeraGraph = {
  templates: TeraGraphTemplate[];
  nodes: TeraGraphNode[];
  relations: TeraGraphRelation[];
};

export type ProjectModelSnapshot = {
  projectRoot: string;
  zolaRoot: string;
  revision: string;
  files: ProjectModelFileSummary[];
  sourceGraph: SourceGraph;
  teraGraph: TeraGraph;
  diagnostics: ProjectModelDiagnostic[];
};

export const PROJECT_AUDIT_SCHEMA_VERSION = 1 as const;

export type AuditSeverity = "info" | "warning" | "error";

export type AuditCategory =
  | "build"
  | "references"
  | "accessibility"
  | "seo"
  | "assets"
  | "workspace";

export type AuditDiagnostic = {
  id: string;
  severity: AuditSeverity;
  category: AuditCategory;
  code: string;
  title: string;
  message: string;
  file: string | null;
  range: SourceRange | null;
};

export type AuditSummary = {
  total: number;
  errors: number;
  warnings: number;
  info: number;
  affectedFiles: number;
};

export type ProjectAuditSnapshot = {
  schemaVersion: typeof PROJECT_AUDIT_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  projectModelRevision: string;
  summary: AuditSummary;
  diagnostics: AuditDiagnostic[];
};

export const DESIGN_CLASS_INVENTORY_SCHEMA_VERSION = 1;
export const DESIGN_CLASS_RENAME_SCHEMA_VERSION = 1;

export type DesignClassOccurrenceKind = "markup" | "style";

export type DesignClassOccurrence = {
  file: string;
  kind: DesignClassOccurrenceKind;
  range: SourceRange;
};

export type DesignClassEntry = {
  name: string;
  markupOccurrences: number;
  selectorOccurrences: number;
  files: string[];
  occurrences: DesignClassOccurrence[];
};

export type DesignClassInventorySnapshot = {
  schemaVersion: typeof DESIGN_CLASS_INVENTORY_SCHEMA_VERSION;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  projectModelRevision: string;
  classes: DesignClassEntry[];
};

export type DesignClassRenameReceipt = {
  schemaVersion: typeof DESIGN_CLASS_RENAME_SCHEMA_VERSION;
  oldName: string;
  newName: string;
  changedFiles: string[];
  replacementCount: number;
  workspace: WorkspaceEntryMutationReceipt;
};

export type PublishOperationKind = "build" | "deploy";

export type PublishOperationCancelReceipt = {
  schemaVersion: 1;
  operationId: string;
  kind: PublishOperationKind;
  cancellationRequested: boolean;
};

export type TemplateWorkbenchDependencyKind = "extends" | "includes" | "imports";

export type TemplateWorkbenchTemplate = {
  sourceId: string;
  file: string;
  name: string;
  origin: SourceOrigin;
  themeName: string | null;
  isPartial: boolean;
  definesMacros: boolean;
};

export type TemplateWorkbenchDependencyStep = {
  fromSourceId: string;
  fromFile: string;
  toSourceId: string;
  toFile: string;
  kind: TemplateWorkbenchDependencyKind;
};

export type TemplateWorkbenchConsumer = {
  pageId: string;
  pageFile: string;
  pageTitle: string;
  pageUrl: string;
  rootTemplateSourceId: string;
  rootTemplateFile: string;
  dependencyPath: TemplateWorkbenchDependencyStep[];
};

export type TemplateWorkbenchNavigatorEntry = {
  role: "directParent" | "active";
  template: TemplateWorkbenchTemplate;
  expanded: boolean;
  editable: boolean;
};

export type TemplateWorkbenchRenderMode =
  | "page"
  | "includedTemplate"
  | "macroScenario"
  | "orphanTemplate";

export type TemplateWorkbenchRenderContextKind =
  | "realZolaPage"
  | "realZolaConsumer"
  | "controlledMacroScenario"
  | "controlledTemplateFixture";

export type TemplateWorkbenchRenderContext = {
  kind: TemplateWorkbenchRenderContextKind;
  canonicalTruth: boolean;
  label: string;
  explanation: string;
};

export type TemplateWorkbenchPlan = {
  schemaVersion: 2;
  projectModelRevision: string;
  activeTemplate: TemplateWorkbenchTemplate;
  directParent: TemplateWorkbenchTemplate | null;
  navigator: TemplateWorkbenchNavigatorEntry[];
  consumers: TemplateWorkbenchConsumer[];
  selectedContext: TemplateWorkbenchConsumer | null;
  renderMode: TemplateWorkbenchRenderMode;
  renderContext: TemplateWorkbenchRenderContext;
  diagnostics: Array<{ code: string; message: string }>;
};

export type ProjectMovePosition = "before" | "after" | "inside";

export type ProjectHtmlMoveIntent = {
  sourceSourceId: string | null;
  targetSourceId: string | null;
  sourceLocation?: ProjectSourceEditLocation | null;
  targetLocation?: ProjectSourceEditLocation | null;
  sourceTag?: string | null;
  targetTag?: string | null;
  sourceSelector?: string | null;
  targetSelector?: string | null;
  position: ProjectMovePosition;
};

export type ProjectHtmlInsertElement = {
  kind?: "html" | "component" | null;
  componentId?: string | null;
  tag: string;
  className?: string | null;
  text?: string | null;
  label?: string | null;
};

export type ProjectHtmlInsertIntent = {
  targetSourceId: string | null;
  targetLocation?: ProjectSourceEditLocation | null;
  targetTag?: string | null;
  targetSelector?: string | null;
  targetKind?: string | null;
  position: ProjectMovePosition;
  element: ProjectHtmlInsertElement;
};

export type ProjectHtmlAttributeMutation =
  | { kind: "setAttribute"; name: string; value: string }
  | { kind: "removeAttribute"; name: string };

export type ProjectHtmlAttributeIntent = {
  targetSourceId: string | null;
  targetLocation?: ProjectSourceEditLocation | null;
  targetTag?: string | null;
  targetSelector?: string | null;
  attributes: ProjectHtmlAttributeMutation[];
  zolaImage?: ProjectZolaImageIntent | null;
};

export type ProjectZolaImageIntent = {
  enabled: boolean;
  sourceUrl?: string | null;
  sourcePath?: string | null;
  width?: number | null;
  height?: number | null;
  operation?: ZolaImageOperation | null;
  format?: ZolaImageFormat | null;
  quality?: number | null;
};

export type ProjectHtmlTextIntent = {
  targetSourceId: string | null;
  targetLocation?: ProjectSourceEditLocation | null;
  targetTag?: string | null;
  targetSelector?: string | null;
  text: string;
};

export type ProjectHtmlTagIntent = {
  targetSourceId: string | null;
  targetLocation?: ProjectSourceEditLocation | null;
  targetTag?: string | null;
  targetSelector?: string | null;
  newTag: string;
};

export type ProjectHtmlDeleteIntent = {
  targetSourceId: string | null;
  targetLocation?: ProjectSourceEditLocation | null;
  targetTag?: string | null;
  targetSelector?: string | null;
};

export type ProjectHtmlDuplicateIntent = {
  sourceSourceId: string | null;
  sourceLocation?: ProjectSourceEditLocation | null;
  sourceTag?: string | null;
  sourceSelector?: string | null;
};

export type ProjectTeraDeleteIntent = {
  targetSourceId: string | null;
  targetLocation?: ProjectSourceEditLocation | null;
  targetKind?: string | null;
  targetLabel?: string | null;
};

export type ProjectTeraInsertItem = {
  kind: string;
  label?: string | null;
  target?: string | null;
  name?: string | null;
  expression?: string | null;
};

export type ProjectTeraInsertIntent = {
  targetSourceId: string | null;
  targetLocation?: ProjectSourceEditLocation | null;
  targetKind?: string | null;
  targetTag?: string | null;
  targetSelector?: string | null;
  position: ProjectMovePosition;
  item: ProjectTeraInsertItem;
};

export type ProjectTeraMoveIntent = {
  sourceSourceId: string | null;
  targetSourceId: string | null;
  sourceLocation?: ProjectSourceEditLocation | null;
  targetLocation?: ProjectSourceEditLocation | null;
  sourceKind?: string | null;
  targetKind?: string | null;
  sourceLabel?: string | null;
  targetTag?: string | null;
  targetSelector?: string | null;
  position: ProjectMovePosition;
};

export type ProjectTemplateEditPermissionIntent = {
  targetSourceId: string | null;
  targetSelector?: string | null;
};

export type TemplateEditPermissionScope = "template" | "partial" | "tera_scope";

export type ProjectTemplateEditPermissionGrant = {
  file: string;
  resolvedTargetId: string;
  targetKind: string;
  targetLabel: string;
  targetLocation: ProjectSourceEditLocation | null;
  selector: string;
  scope: TemplateEditPermissionScope;
};

export type ProjectSourceEditLocation = {
  file: string;
  line: number;
  column: number;
};

export type PreviewProjectionIntentKind =
  | "layer_drop"
  | "html_insert_drop"
  | "html_attributes"
  | "html_text"
  | "html_tag"
  | "html_duplicate"
  | "tera_insert_drop"
  | "tera_move_drop"
  | "html_delete"
  | "template_delete"
  | "template_edit"
  | "unsupported";

export type PreviewProjectionIntentStatus = "accepted" | "blocked" | "unsupported";

export type PreviewProjectionEffect =
  | "kernel_mutation_preflight"
  | "template_permission_preflight"
  | "unsupported";

export type PreviewProjectionDiagnosticSeverity = "info" | "warning" | "error";

export type PreviewProjectionDiagnostic = {
  code: string;
  severity: PreviewProjectionDiagnosticSeverity;
  message: string;
  blocking: boolean;
};

export type PreviewProjectionIntentInput = {
  messageType: string;
  previewRevision?: number | null;
  sourceSelector?: string | null;
  targetSelector?: string | null;
  selector?: string | null;
  sourceId?: string | null;
  targetSourceId?: string | null;
  sourceTemplateSourceId?: string | null;
  targetTemplateSourceId?: string | null;
  sourceSessionId?: string | null;
  targetSessionId?: string | null;
  sourceTag?: string | null;
  targetTag?: string | null;
  targetKind?: string | null;
  position?: string | null;
  itemKind?: string | null;
  elementTag?: string | null;
};

export type PreviewProjectionIntentReceipt = {
  schemaVersion: number;
  intentId: string;
  kind: PreviewProjectionIntentKind;
  status: PreviewProjectionIntentStatus;
  effect: PreviewProjectionEffect;
  accepted: boolean;
  requiresProjectSession: boolean;
  projectSessionId: string | null;
  projectRoot: string | null;
  runtimeSessionId: string | null;
  previewRevision: number | null;
  message: string;
  diagnostics: PreviewProjectionDiagnostic[];
};

export type PreviewStructuralCommandIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

export type ProjectHtmlInsertPatch = {
  file: string;
  resolvedTargetId: string;
  insertedLabel: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  insertedLocation: ProjectSourceEditLocation;
  insertedStartLine: number;
  lineShiftStart: number;
  lineShift: number;
  tag: string;
  className: string;
  text: string;
  html: string;
  componentId: string | null;
  dataAnim: string | null;
  componentInstanceId: string | null;
};

export type PageComponentContractTextPlan = {
  changed: boolean;
  contents: string;
};

export type PageComponentRegistryItem = {
  id: string;
  kind: "css" | "js";
  label: string;
  description: string;
  tag: string;
  text: string;
  className: string;
  html: string;
};

export type PageComponentRegistryGroup = {
  label: string;
  elements: PageComponentRegistryItem[];
};

export type PageComponentRegistrySnapshot = {
  schemaVersion: number;
  components: PageComponentRegistryItem[];
  groups: PageComponentRegistryGroup[];
};

export type PageComponentContractInput = {
  templatePath: string;
  templateSource: string;
  stylesheetSource?: string | null;
  pageJsConfig?: PageJsConfig | null;
  ensureComponentId?: string | null;
  cachebustAssets?: boolean | null;
};

export type PageComponentContractApplyInput = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  templatePath: string;
  ensureComponentId?: string | null;
  cachebustAssets?: boolean | null;
};

export type PageComponentContractPlan = {
  templatePath: string;
  stylesheetPath: string;
  stylesheetHref: string;
  activeComponentIds: string[];
  template: PageComponentContractTextPlan;
  stylesheet: PageComponentContractTextPlan;
  pageJsConfig: PageJsConfig;
  pageJsChanged: boolean;
  previewCss: string;
  diagnostics: string[];
};

export type PageComponentContractApplyReceipt = {
  plan: PageComponentContractPlan;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  pageJs: PageJsDraftStageReceipt | null;
  authority: PageContractAuthorityReceipt;
};

export type PageAssetContractTextPlan = {
  changed: boolean;
  contents: string;
};

export type PageAssetContractInput = {
  templatePath: string;
  templateSource: string;
  stylesheetSource?: string | null;
  stylesheetKnown?: boolean | null;
  pageJsConfig?: PageJsConfig | null;
};

export type PageAssetContractApplyInput = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  templatePath: string;
};

export type PageAssetContractPlan = {
  templatePath: string;
  stylesheetPath: string;
  stylesheetHref: string;
  activeDataAnimIds: string[];
  activeGeneratedClasses: string[];
  template: PageAssetContractTextPlan;
  stylesheet: PageAssetContractTextPlan;
  pageJsConfig: PageJsConfig;
  pageJsChanged: boolean;
  diagnostics: string[];
};

export type PageAssetContractApplyReceipt = {
  plan: PageAssetContractPlan;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  pageJs: PageJsDraftStageReceipt | null;
  authority: PageContractAuthorityReceipt;
};

export type PageContractApplyStatus = "noop" | "staged";

export type PageContractConsumedSourceRevision = {
  relativePath: string;
  beforeRevision: number | null;
  beforeHash: string | null;
  afterRevision: number | null;
  afterHash: string | null;
};

export type PageContractAuthorityReceipt = {
  schemaVersion: 2;
  operationId: string;
  status: PageContractApplyStatus;
  projectRoot: string;
  sessionId: string;
  revisionBefore: number;
  revisionAfter: number;
  dirty: boolean;
  consumedSources: PageContractConsumedSourceRevision[];
  touchedFiles: string[];
};

export type CanvasPatchAnchor = {
  sourceId: string;
  renderInstanceId: string | null;
  selectorFallback: string | null;
  expectedTag: string | null;
};

export type CanvasPatchOperation =
  | { kind: "setAttributes"; target: CanvasPatchAnchor; attributes: Record<string, string | null> }
  | { kind: "setText"; target: CanvasPatchAnchor; text: string }
  | { kind: "replaceTag"; target: CanvasPatchAnchor; newTag: string }
  | { kind: "insert"; target: CanvasPatchAnchor; position: ProjectMovePosition; html: string }
  | { kind: "move"; source: CanvasPatchAnchor; target: CanvasPatchAnchor; position: ProjectMovePosition }
  | { kind: "duplicate"; source: CanvasPatchAnchor; html: string }
  | { kind: "delete"; target: CanvasPatchAnchor };

export type CanvasPatch = {
  schemaVersion: 1;
  patchId: string;
  issuedAtMs: number;
  projectRoot: string;
  runtimeSessionId: string;
  baseWorkspaceRevision: number;
  workspaceRevision: number;
  workspaceTransactionId: string;
  beforeModelRevision: string;
  afterModelRevision: string;
  operation: CanvasPatchOperation;
};

export type PreviewHtmlInsertDropExecutionStatus = "committed" | "blocked";

export type PreviewHtmlInsertDropExecutionInput = {
  intent: PreviewProjectionIntentInput;
  insertIntent: ProjectHtmlInsertIntent;
};

export type PreviewHtmlInsertDropExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlInsertDropExecutionStatus;
  message: string;
  modelRevision: string | null;
  patch: ProjectHtmlInsertPatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectHtmlAttributePatch = {
  file: string;
  resolvedTargetId: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  tag: string;
  attributes: Record<string, string | null>;
  zolaImageContract: boolean;
  zolaImage: ZolaImagePresentation | null;
};

export type PreviewHtmlAttributesExecutionStatus = "committed" | "blocked";

export type PreviewHtmlAttributesExecutionInput = {
  intent: PreviewProjectionIntentInput;
  attributeIntent: ProjectHtmlAttributeIntent;
};

export type PreviewHtmlAttributesExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlAttributesExecutionStatus;
  message: string;
  modelRevision: string | null;
  patch: ProjectHtmlAttributePatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectHtmlTextPatch = {
  file: string;
  resolvedTargetId: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  lineShiftStart: number;
  lineShift: number;
  tag: string;
  text: string;
};

export type PreviewHtmlTextExecutionStatus = "committed" | "blocked";

export type PreviewHtmlTextExecutionInput = {
  intent: PreviewProjectionIntentInput;
  textIntent: ProjectHtmlTextIntent;
  deferCanonicalProjection?: boolean;
  editSessionId?: string | null;
};

export type PreviewHtmlTextExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlTextExecutionStatus;
  message: string;
  modelRevision: string | null;
  patch: ProjectHtmlTextPatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectHtmlTagPatch = {
  file: string;
  resolvedTargetId: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  lineShiftStart: number;
  lineShift: number;
  oldTag: string;
  newTag: string;
};

export type PreviewHtmlTagExecutionStatus = "committed" | "blocked";

export type PreviewHtmlTagExecutionInput = {
  intent: PreviewProjectionIntentInput;
  tagIntent: ProjectHtmlTagIntent;
};

export type PreviewHtmlTagExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlTagExecutionStatus;
  message: string;
  modelRevision: string | null;
  patch: ProjectHtmlTagPatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectHtmlDuplicatePatch = {
  file: string;
  resolvedSourceId: string;
  duplicatedLabel: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  sourceLocation: ProjectSourceEditLocation;
  insertedLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  sourceEndLine: number;
  insertedStartLine: number;
  lineShiftStart: number;
  lineShift: number;
  tag: string;
  html: string;
  componentIds: string[];
  dataAnimCount: number;
  duplicateIdCount: number;
  zolaImageContract: boolean;
};

export type PreviewHtmlDuplicateExecutionStatus = "committed" | "blocked";

export type PreviewHtmlDuplicateExecutionInput = {
  intent: PreviewProjectionIntentInput;
  duplicateIntent: ProjectHtmlDuplicateIntent;
};

export type PreviewHtmlDuplicateExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlDuplicateExecutionStatus;
  message: string;
  modelRevision: string | null;
  patch: ProjectHtmlDuplicatePatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectHtmlDeletePatch = {
  file: string;
  resolvedTargetId: string;
  deletedLabel: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  sourceEndLine: number;
  lineShiftStart: number;
  lineShift: number;
};

export type PreviewHtmlDeleteExecutionStatus = "committed" | "blocked";

export type PreviewHtmlDeleteExecutionInput = {
  intent: PreviewProjectionIntentInput;
  deleteIntent: ProjectHtmlDeleteIntent;
};

export type PreviewHtmlDeleteExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewHtmlDeleteExecutionStatus;
  message: string;
  modelRevision: string | null;
  patch: ProjectHtmlDeletePatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectTeraDeletePatch = {
  file: string;
  resolvedTargetId: string;
  deletedLabel: string;
  deletedKind: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  sourceEndLine: number;
  lineShiftStart: number;
  lineShift: number;
};

export type PreviewTeraDeleteExecutionStatus = "committed" | "blocked";

export type PreviewTeraDeleteExecutionInput = {
  intent: PreviewProjectionIntentInput;
  deleteIntent: ProjectTeraDeleteIntent;
};

export type PreviewTeraDeleteExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewTeraDeleteExecutionStatus;
  message: string;
  modelRevision: string | null;
  patch: ProjectTeraDeletePatch | null;
  canvasPatch: null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectTeraInsertPatch = {
  file: string;
  resolvedTargetId: string;
  insertedLabel: string;
  insertedKind: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  targetLocation: ProjectSourceEditLocation;
  insertedLocation: ProjectSourceEditLocation;
  insertedStartLine: number;
  lineShiftStart: number;
  lineShift: number;
  snippet: string;
};

export type PreviewTeraInsertDropExecutionStatus = "committed" | "blocked";

export type PreviewTeraInsertDropExecutionInput = {
  intent: PreviewProjectionIntentInput;
  insertIntent: ProjectTeraInsertIntent;
};

export type PreviewTeraInsertDropExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewTeraInsertDropExecutionStatus;
  message: string;
  modelRevision: string | null;
  patch: ProjectTeraInsertPatch | null;
  canvasPatch: null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectTeraMovePatch = {
  file: string;
  resolvedSourceId: string;
  resolvedTargetId: string;
  movedLabel: string;
  movedKind: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  sourceLocation: ProjectSourceEditLocation;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  sourceEndLine: number;
  newStartLine: number;
};

export type PreviewTeraMoveDropExecutionStatus = "committed" | "blocked";

export type PreviewTeraMoveDropExecutionInput = {
  intent: PreviewProjectionIntentInput;
  moveIntent: ProjectTeraMoveIntent;
};

export type PreviewTeraMoveDropExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewTeraMoveDropExecutionStatus;
  message: string;
  modelRevision: string | null;
  patch: ProjectTeraMovePatch | null;
  canvasPatch: null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type PreviewTemplateEditPermissionStatus = "granted" | "blocked";

export type PreviewTemplateEditPermissionInput = {
  intent: PreviewProjectionIntentInput;
  editIntent: ProjectTemplateEditPermissionIntent;
};

export type PreviewTemplateEditPermissionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewTemplateEditPermissionStatus;
  message: string;
  modelRevision: string | null;
  grant: ProjectTemplateEditPermissionGrant | null;
  diagnostics: PreviewProjectionDiagnostic[];
};

export type PreviewLayerDropExecutionInput = {
  intent: PreviewProjectionIntentInput;
  moveIntent: ProjectHtmlMoveIntent;
};

export type PreviewLayerDropExecutionStatus = "committed" | "blocked";

export type PreviewLayerDropExecutionReceipt = {
  schemaVersion: number;
  intent: PreviewProjectionIntentReceipt;
  status: PreviewLayerDropExecutionStatus;
  message: string;
  modelRevision: string | null;
  projectedSourceId: string | null;
  patch: ProjectHtmlMovePatch | null;
  canvasPatch: CanvasPatch | null;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  touchedFiles: string[];
  diagnostics: PreviewProjectionDiagnostic[];
};

export type ProjectHtmlMovePatch = {
  file: string;
  resolvedSourceId: string;
  resolvedTargetId: string;
  sourceLabel: string;
  beforeRevision: string;
  afterRevision: string;
  contents: string;
  sourceLocation: ProjectSourceEditLocation;
  targetLocation: ProjectSourceEditLocation;
  sourceStartLine: number;
  sourceEndLine: number;
  newStartLine: number;
};

export type ProjectHtmlMovePlan = {
  allowed: boolean;
  diagnostic: string | null;
  modelRevision: string;
  patch: ProjectHtmlMovePatch | null;
};

export type ProjectDiskManifestEntry = {
  relativePath: string;
  modifiedMs: number;
  size: number;
  versionToken?: string;
};

export type ProjectDiskManifest = {
  root: string;
  files: ProjectDiskManifestEntry[];
  truncated: boolean;
  maxFiles: number;
};

export type ExternalDiskState = {
  baseline: ProjectDiskManifest | null;
  reconciling: boolean;
  changed: boolean;
  changedFiles: string[];
  activeFileChanged: boolean;
  previewRelevantChanged: boolean;
  blockedByDirtySession: boolean;
  lastDetectedAt: number | null;
  lastDetectedFiles: string[];
  lastDetectedActiveFileChanged: boolean;
  lastDetectedPreviewRelevantChanged: boolean;
  lastAppliedAt: number | null;
  lastAppliedFiles: string[];
  lastCheckedAt: number | null;
  checking: boolean;
  workspaceProjectionRecoveryRequired: boolean;
  truncated: boolean;
};

export type AiContextStatus = {
  contextPath: string;
  discoveryPath: string;
  endpoint: string;
  contextExists: boolean;
  discoveryExists: boolean;
  updatedAt: string | null;
  mode: string;
  serverRunning: boolean;
};

export type AiPresenceStatus = "active" | "idle" | "expired";

export type AiClientSessionSnapshot = {
  sessionId: string;
  clientName: string;
  clientVersion: string | null;
  initializedAtMs: number;
  lastSeenAtMs: number;
  contextRevisionSeen: number | null;
  presence: AiPresenceStatus;
  ownsEditLease: boolean;
};

export type EditLeaseRequest = {
  clientSessionId: string;
  expectedProjectSessionId: string;
  expectedProjectRevision: number;
  requestId: string;
  intent: string;
};

export type EditLease = {
  id: string;
  requestId: string;
  clientSessionId: string;
  projectSessionId: string;
  basisProjectRevision: number;
  intent: string;
  grantedAtMs: number;
  expiresAtMs: number;
};

export type EditAuthority =
  | { state: "user_active" }
  | {
      state: "ai_requested";
      detail: { request: EditLeaseRequest; requestedAtMs: number };
    }
  | { state: "ai_active"; detail: { lease: EditLease } }
  | {
      state: "ai_orphaned";
      detail: {
        leaseId: string;
        clientSessionId: string;
        projectSessionId: string;
        basisProjectRevision: number;
        expiredAtMs: number;
        reason: string;
      };
    }
  | {
      state: "reconciling";
      detail: {
        leaseId: string;
        clientSessionId: string;
        projectSessionId: string;
        basisProjectRevision: number;
        releasedAtMs: number;
        expectedChangedFiles: string[];
        observedChangedFiles: string[];
        declarationReviewedByUser: boolean;
        recoveryReloadAuthorized: boolean;
        recoveryReloadReplacementSessionId: string | null;
        summary: string | null;
        reason: string;
      };
    }
  | {
      state: "conflict";
      detail: {
        projectSessionId: string;
        detectedAtMs: number;
        files: string[];
        reason: string;
      };
    };

export type EditLeaseStatus =
  | "pending_ui_quiescence"
  | "granted"
  | "blocked"
  | "busy"
  | "stale"
  | "orphaned"
  | "reconciling"
  | "released_to_user"
  | "conflict";

export type RequiredUserAction =
  | "save_or_discard"
  | "wait_for_ai"
  | "recover_interrupted_ai"
  | "resolve_conflict"
  | "reopen_project";

export type EditTransitionReceipt = {
  status: EditLeaseStatus;
  coordinationRevision: number;
  authority: EditAuthority;
  lease: EditLease | null;
  reason: string | null;
  requiredUserAction: RequiredUserAction | null;
  dirtyFiles: string[];
};

export type AiCoordinationSnapshot = {
  schemaVersion: 2;
  coordinationRevision: number;
  projectSessionId: string | null;
  authority: EditAuthority;
  clients: AiClientSessionSnapshot[];
};

export type UiQuiescenceAcknowledgement = {
  requestId: string;
  projectSessionId: string;
  projectRevision: number;
  uiRevision: number;
  uiQuiescent: boolean;
  blockerReason: string | null;
  dirtyFiles: string[];
};

export type CodexMcpStatus = {
  configPath: string;
  configExists: boolean;
  configured: boolean;
  authenticated: boolean;
  securePermissions: boolean;
  configuredUrl: string | null;
  expectedUrl: string;
};

export type UiContextProjection = {
  schemaVersion: 2;
  uiRevision: number;
  expectedProjectSessionId: string | null;
  expectedProjectRevision: number | null;
  project: {
    isZola: boolean;
    isEmpty: boolean;
    previewBaseUrl: string | null;
    previewWarning: string | null;
  };
  workspace: {
    centerView: CenterView;
    previewDevice: "desktop" | "tablet" | "mobile";
    activeFile: string | null;
    activePreviewPath: string | null;
    sourceLanguage: SourceLanguage;
  };
  selection: {
    hasSelection: boolean;
    selector: string | null;
    cssSelector: string | null;
    tag: string | null;
    id: string | null;
    classes: string[];
    text: string | null;
    imageSrc: string | null;
    sourceLocation: SourceEditLocation | null;
    sourceId: string | null;
    templateSourceId: string | null;
    sessionId: string | null;
    rect: SelectionInfo["rect"] | null;
  };
  css: {
    activeSelector: string | null;
    targetFile: string | null;
    variablesCount: number;
  };
  uiDirtyState: {
    dirty: boolean;
    canSave: boolean;
    areas: string[];
    blockedReason: string;
  };
  externalDisk: {
    changed: boolean;
    changedFiles: string[];
    activeFileChanged: boolean;
    previewRelevantChanged: boolean;
    blockedByDirtySession: boolean;
    lastDetectedAt: number | null;
    lastDetectedFiles: string[];
    lastDetectedActiveFileChanged: boolean;
    lastDetectedPreviewRelevantChanged: boolean;
    lastAppliedAt: number | null;
    lastAppliedFiles: string[];
    lastCheckedAt: number | null;
    checking: boolean;
    reconciling: boolean;
    workspaceProjectionRecoveryRequired: boolean;
    truncated: boolean;
  };
};

export type SourceNodeRange = {
  selector: string;
  cssSelector: string;
  tag: string;
  openStart: number;
  openEnd: number;
  end: number;
};

// ── JS / Motion tab types ─────────────────────────────────────────────────────

export type PanaComponent = {
  id: string;
};

export type PanaMotionFamily =
  | "animation"
  | "timeline"
  | "timer"
  | "animatable"
  | "draggable"
  | "layout"
  | "scope"
  | "scroll"
  | "svg"
  | "text"
  | "utilities"
  | "easing"
  | "waapi"
  | "engine"
  | "interaction"
  | "custom";

export type PanaMotionTargetMode =
  | "selected"
  | "dataAnim"
  | "selector"
  | "dom"
  | "array"
  | "object"
  | "scope"
  | "expression";

export type PanaMotionValueMode =
  | "literal"
  | "fromTo"
  | "relative"
  | "cssVariable"
  | "color"
  | "function"
  | "random"
  | "expression";

export type PanaMotionExpression = {
  enabled: boolean;
  label: string;
  code: string;
};

export type PanaMotionTween = {
  delay: number;
  duration: number;
  ease: string;
};

export type PanaMotionTarget = {
  mode: PanaMotionTargetMode;
  selector: string;
  dataAnim: string;
  expression: string;
};

export type PanaMotionValue = {
  mode: PanaMotionValueMode;
  value: string;
  from: string;
  to: string;
  unit: string;
  expression: string;
};

export type PanaMotionProperty = {
  id: string;
  property: string;
  category: "css" | "transform" | "cssVariable" | "object" | "htmlAttribute" | "svgAttribute" | "utility";
  value: PanaMotionValue;
  modifier: PanaMotionExpression;
  composition: string;
  tween: PanaMotionTween;
};

export type PanaMotionKeyframe = {
  id: string;
  label: string;
  at: string;
  duration: number;
  ease: string;
  properties: PanaMotionProperty[];
  advanced: PanaMotionExpression[];
};

export type PanaMotionPlayback = {
  autoplay: boolean;
  delay: number;
  duration: number;
  loop: number;
  loopDelay: number;
  alternate: boolean;
  reversed: boolean;
  frameRate: number;
  playbackRate: number;
  playbackEase: string;
  persist: boolean;
};

export type PanaMotionStagger = {
  enabled: boolean;
  each: number;
  start: number;
  from: string;
  reversed: boolean;
  ease: string;
  grid: string;
  axis: string;
  use: string;
  total: number;
  modifier: PanaMotionExpression;
};

export type PanaMotionBaseItem = {
  id: string;
  type: PanaMotionFamily;
  name: string;
  enabled: boolean;
  target: PanaMotionTarget;
  scopeId: string;
  advanced: PanaMotionExpression[];
};

export type PanaMotionAnimationItem = PanaMotionBaseItem & {
  type: "animation";
  properties: PanaMotionProperty[];
  keyframes: PanaMotionKeyframe[];
  playback: PanaMotionPlayback;
  stagger: PanaMotionStagger;
  callbacks: Record<string, PanaMotionExpression>;
  trigger?: "load" | "scroll" | "click" | "hover";
  scrollRepeat?: boolean;
  scrollScrub?: boolean;
  textEffect?: string;
  targetSelector?: string;
};

export type PanaMotionTimelineStepType = "animation" | "timer" | "callback" | "set" | "sync" | "label";

export type PanaMotionTimelineStep = {
  id: string;
  type: PanaMotionTimelineStepType;
  label: string;
  position: string;
  duration: number;
  lane: string;
  targetItemId: string;
  callback: PanaMotionExpression;
};

export type PanaMotionTimelineTrack = {
  id: string;
  name: string;
  collapsed: boolean;
  height: number;
  color: string;
};

export type PanaMotionTimelineItem = PanaMotionBaseItem & {
  type: "timeline";
  duration: number;
  tracks: PanaMotionTimelineTrack[];
  labels: Array<{ id: string; name: string; position: string }>;
  steps: PanaMotionTimelineStep[];
  playback: PanaMotionPlayback;
};

export type PanaMotionTimerItem = PanaMotionBaseItem & {
  type: "timer";
  playback: PanaMotionPlayback;
  callbacks: Record<string, PanaMotionExpression>;
};

export type PanaMotionAnimatableItem = PanaMotionBaseItem & {
  type: "animatable";
  properties: PanaMotionProperty[];
  mode: "setters" | "getters" | "both";
  duration: number;
  ease: string;
  unit: string;
  liveSource: "none" | "pointer" | "scroll" | "expression";
  setterExpression: PanaMotionExpression;
};

export type PanaMotionDraggableItem = PanaMotionBaseItem & {
  type: "draggable";
  axes: "x" | "y" | "both";
  container: string;
  trigger: string;
  snap: string;
  snapX: string;
  snapY: string;
  mapTo: string;
  modifier: PanaMotionExpression;
  containerPadding: number;
  friction: number;
  releaseContainerFriction: number;
  velocity: number;
  minVelocity: number;
  maxVelocity: number;
  releaseEase: string;
  dragSpeed: number;
  dragThreshold: number;
  scrollThreshold: number;
  scrollSpeed: number;
  cursor: boolean;
  release: {
    spring: boolean;
    mass: number;
    stiffness: number;
    damping: number;
  };
  callbacks: Record<string, PanaMotionExpression>;
};

export type PanaMotionLayoutItem = PanaMotionBaseItem & {
  type: "layout";
  mode: "record" | "animate" | "update" | "revert";
  children: string;
  properties: string;
  enterFrom: string;
  leaveTo: string;
  swapAt: string;
  includeDisplay: boolean;
  includeGrid: boolean;
  includeFlex: boolean;
  includeOrder: boolean;
  enterExit: boolean;
  swapParent: boolean;
  playback: PanaMotionPlayback;
  callbacks: Record<string, PanaMotionExpression>;
};

export type PanaMotionScopeItem = PanaMotionBaseItem & {
  type: "scope";
  root: string;
  defaults: Record<string, string>;
  mediaQueries: Array<{ id: string; query: string; enabled: boolean }>;
  reducedMotion: "respect" | "ignore" | "disable";
  keepTime: boolean;
};

export type PanaMotionScrollItem = PanaMotionBaseItem & {
  type: "scroll";
  container: string;
  axis: "x" | "y";
  repeat: boolean;
  debug: boolean;
  enter: string;
  leave: string;
  threshold: string;
  sync: "play" | "pause" | "restart" | "reverse" | "progress" | "smooth" | "eased";
  syncMode: "methods" | "progress" | "smooth" | "eased";
  syncMethods: string;
  syncEase: string;
  smooth: number;
  callbacks: Record<string, PanaMotionExpression>;
};

export type PanaMotionSvgItem = PanaMotionBaseItem & {
  type: "svg";
  mode: "morphTo" | "createDrawable" | "createMotionPath";
  attribute: "d" | "points";
  source: string;
  path: string;
  precision: number;
  offset: number;
  draw: string;
  playback: PanaMotionPlayback;
  callbacks: Record<string, PanaMotionExpression>;
};

export type PanaMotionTextItem = PanaMotionBaseItem & {
  type: "text";
  mode: "splitText" | "scrambleText";
  split: {
    lines: boolean;
    words: boolean;
    chars: boolean;
    debug: boolean;
    includeSpaces: boolean;
    accessible: boolean;
    className: string;
    wrap: string;
    clone: boolean;
  };
  scramble: {
    text: string;
    chars: string;
    override: boolean;
    ease: string;
    cursor: string;
    revealRate: number;
    revealDelay: number;
    settleRate: number;
    settleDuration: number;
    delay: number;
    duration: number;
    from: "auto" | "left" | "center" | "right" | "random" | string;
    reversed: boolean;
    perturbation: number;
    seed: number;
  };
  callbacks: Record<string, PanaMotionExpression>;
};

export type PanaMotionUtilitiesItem = PanaMotionBaseItem & {
  type: "utilities";
  utility: string;
  args: string;
  stagger: PanaMotionStagger;
  expression: PanaMotionExpression;
};

export type PanaMotionEasingItem = PanaMotionBaseItem & {
  type: "easing";
  mode: "builtIn" | "cubicBezier" | "linear" | "steps" | "irregular" | "spring" | "custom";
  value: string;
  previewDuration: number;
};

export type PanaMotionWaapiItem = PanaMotionBaseItem & {
  type: "waapi";
  properties: PanaMotionProperty[];
  playback: PanaMotionPlayback;
  iterations: number;
  direction: string;
  easing: string;
  autoplay: boolean;
  hardwareAcceleration: boolean;
  convertEase: boolean;
  finished: PanaMotionExpression;
};

export type PanaMotionEngineItem = PanaMotionBaseItem & {
  type: "engine";
  timeUnit: "ms" | "s";
  speed: number;
  fps: number;
  precision: number;
  pauseOnDocumentHidden: boolean;
  priority: number;
};

export type PanaMotionInteractionItem = PanaMotionBaseItem & {
  type: "interaction";
  event: string;
  action: string;
  targetSelector: string;
  value: string;
};

export type PanaMotionCustomItem = PanaMotionBaseItem & {
  type: "custom";
  code: string;
};

export type PanaMotionItem =
  | PanaMotionAnimationItem
  | PanaMotionTimelineItem
  | PanaMotionTimerItem
  | PanaMotionAnimatableItem
  | PanaMotionDraggableItem
  | PanaMotionLayoutItem
  | PanaMotionScopeItem
  | PanaMotionScrollItem
  | PanaMotionSvgItem
  | PanaMotionTextItem
  | PanaMotionUtilitiesItem
  | PanaMotionEasingItem
  | PanaMotionWaapiItem
  | PanaMotionEngineItem
  | PanaMotionInteractionItem
  | PanaMotionCustomItem;

export type PanaMotionConfig = {
  schemaVersion: 1;
  animeVersion: string;
  activeItemId: string | null;
  items: PanaMotionItem[];
};

export type PageJsConfig = {
  version?: 1;
  components: PanaComponent[];
  motion?: PanaMotionConfig;
};

export type MotionTimelineStepTimingPatch = {
  position?: string;
  duration?: number;
};

export type MotionTimelineStepTimingInput = {
  config: PageJsConfig;
  timelineId?: string | null;
  stepId?: string | null;
  stepIndex?: number | null;
  patch: MotionTimelineStepTimingPatch;
};

export type MotionGraphTransaction = {
  schemaVersion: 1;
  id: string;
  command: "motion.timeline.stepTiming";
  target: string;
  timelineId: string;
  stepId: string;
  forwardPatch: MotionTimelineStepTimingPatch;
  reversePatch: MotionTimelineStepTimingPatch;
  beforeConfigHash: string;
  afterConfigHash: string;
};

export type MotionTimelineStepTimingReceipt = {
  schemaVersion: 1;
  command: "motion.timeline.stepTiming";
  changed: boolean;
  timelineId: string;
  stepId: string;
  stepIndex: number;
  beforeStep: Record<string, unknown>;
  afterStep: Record<string, unknown>;
  afterConfig: PageJsConfig;
  transaction: MotionGraphTransaction | null;
  diagnostics: string[];
};

export type PageJsDraftEntry = {
  templatePath: string;
  base: PageJsConfig;
  current: PageJsConfig;
  cachebustAssets: boolean;
  source: string;
  coalesceKey: string | null;
  transactionId: string | null;
  updatedAtMs: number;
  revision: number;
  baseConfigBytes: number;
  currentConfigBytes: number;
  retainedConfigBytes: number;
};

export type PageJsDraftStoreLimits = {
  maxDrafts: number;
  maxConfigBytes: number;
  maxTotalConfigBytes: number;
};

export type PageJsDraftStoreSnapshot = {
  schemaVersion: 2;
  sessionId: string;
  runtimeSessionId: string;
  projectRoot: string;
  revision: number;
  dirtyCount: number;
  retainedConfigBytes: number;
  limits: PageJsDraftStoreLimits;
  drafts: PageJsDraftEntry[];
};

export type PageJsDraftStageInput = {
  templatePath: string;
  baseConfig: PageJsConfig;
  currentConfig: PageJsConfig;
  cachebustAssets: boolean;
  source?: string | null;
  coalesceKey?: string | null;
  transactionId?: string | null;
};

export type PageJsRequestIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

export type PageJsDraftSessionIdentity = PageJsRequestIdentity;

export type PageJsCommandReceipt<T> = {
  projectRoot: string;
  runtimeSessionId: string;
  payload: T;
};

export type PageJsWorkspaceState = {
  templatePath: string;
  accepted: PageJsConfig;
  current: PageJsConfig;
  dirty: boolean;
  entryRevision: number | null;
};

export type PageJsDraftStageReceipt = {
  schemaVersion: 2;
  status: "staged" | "cleared" | "unchanged";
  changed: boolean;
  dirty: boolean;
  templatePath: string;
  revision: number;
  entryRevision: number | null;
  dirtyCount: number;
  retainedConfigBytes: number;
  projectRoot: string;
  runtimeSessionId: string;
};
