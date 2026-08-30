import type { LocalizedDiagnostic } from "$lib/contracts/localized-diagnostic";

export const WORKBENCH_SCHEMA_VERSION = 3;

export const WORKBENCH_COMMAND_SCHEMA_VERSION = 1;

export type WorkbenchActivity =
  | "editor"
  | "templates"
  | "components"
  | "design_system"
  | "assets"
  | "content"
  | "content_models"
  | "taxonomies"
  | "data"
  | "versioning"
  | "audit"
  | "publish"
  | "project_settings";

export type WorkbenchSurface = "visual" | "code";

export type WorkbenchDocumentPresentation = "html" | "code_only";

type ContentWorkspaceMode = "list" | "edit";

type ContentWorkspaceSnapshot = {
  mode: ContentWorkspaceMode;
  pagePath: string | null;
};

type WorkbenchProjectEntryKind = "directory" | "text" | "binary";

type WorkbenchProjectEntrySelection = {
  relativePath: string;
  kind: WorkbenchProjectEntryKind;
};

type WorkbenchProjectEntryRemap = {
  sourcePrefix: string;
  destinationPrefix: string;
};

type WorkbenchDocumentPresentationEntry = {
  relativePath: string;
  presentation: WorkbenchDocumentPresentation;
};

export type WorkspaceSourceOpenOptions = {
  surface?: WorkbenchSurface;
  templateContextPagePath?: string | null;
  templateContextUrl?: string | null;
  componentName?: string | null;
};

export type WorkbenchSplit = "none" | "vertical" | "horizontal";

export type WorkbenchGroupId = "primary" | "secondary";

export type WorkbenchBottomPanelView = "problems" | "output" | "terminal";

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
  presentation: WorkbenchDocumentPresentation;
  surface: WorkbenchSurface;
  pinned: boolean;
};

export type WorkbenchDocumentActivationPhase =
  | "idle"
  | "applying"
  | "loading"
  | "ready"
  | "failed";

export type WorkbenchDocumentActivationCacheOutcome =
  | "reused"
  | "materialized"
  | "not_applicable"
  | "unknown";

type WorkbenchDocumentActivationMetrics = {
  intentMs: number | null;
  resolveMs: number | null;
  loadMs: number | null;
  surfaceMs: number | null;
  totalMs: number | null;
};

export type WorkbenchDocumentActivationSnapshot = {
  serial: number;
  phase: WorkbenchDocumentActivationPhase;
  documentId: string | null;
  relativePath: string | null;
  surface: WorkbenchSurface | null;
  cacheOutcome: WorkbenchDocumentActivationCacheOutcome;
  diagnostic: string | null;
  metrics: WorkbenchDocumentActivationMetrics;
};

type WorkbenchGroupSnapshot = {
  groupId: WorkbenchGroupId;
  documents: WorkbenchDocumentSnapshot[];
  activeDocumentId: string | null;
};

type WorkbenchBottomPanelSnapshot = {
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
  contentWorkspace: ContentWorkspaceSnapshot;
  selectedProjectEntry: WorkbenchProjectEntrySelection | null;
};

export type WorkbenchIntent =
  | {
      kind: "open_document";
      relativePath: string;
      groupId?: WorkbenchGroupId;
      surface?: WorkbenchSurface;
      presentation?: WorkbenchDocumentPresentation;
      pinned?: boolean;
    }
  | {
      kind: "select_project_entry";
      relativePath: string;
      entryKind: WorkbenchProjectEntryKind;
      openSurface?: WorkbenchSurface | null;
      openPresentation?: WorkbenchDocumentPresentation | null;
    }
  | {
      kind: "reconcile_project_entries";
      remaps?: WorkbenchProjectEntryRemap[];
      deletedPrefixes?: string[];
      selectionOverride?: WorkbenchProjectEntrySelection | null;
      documentPresentations?: WorkbenchDocumentPresentationEntry[];
    }
  | {
      kind: "reconcile_document_presentations";
      documents: WorkbenchDocumentPresentationEntry[];
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
      presentation: WorkbenchDocumentPresentation;
    }
  | { kind: "set_split_ratio"; ratioBasisPoints: number }
  | { kind: "set_canvas_viewport"; viewport: WorkbenchCanvasViewportSnapshot }
  | { kind: "set_activity"; activity: WorkbenchActivity }
  | { kind: "open_content_page"; relativePath: string }
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

export const COMMAND_CENTER_SCHEMA_VERSION = 3 as const;

export type CommandCenterScope = "all" | "commands" | "files" | "symbols";

type CommandCenterItemKind =
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
  | "close_application"
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
  | "open_about"
  | "show_visual"
  | "show_code";

export type CommandCenterAction =
  | { kind: "set_activity"; activity: WorkbenchActivity }
  | { kind: "open_document"; relativePath: string; surface: WorkbenchSurface }
  | { kind: "app_command"; command: CommandCenterAppCommand };

export type CommandCenterItem = {
  id: string;
  kind: CommandCenterItemKind;
  title: string | null;
  titleDiagnostic: LocalizedDiagnostic | null;
  subtitle: string | null;
  subtitleDiagnostic: LocalizedDiagnostic | null;
  shortcut: string | null;
  enabled: boolean;
  disabledDiagnostic: LocalizedDiagnostic | null;
  score: number;
  action: CommandCenterAction;
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
