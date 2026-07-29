import { saveAiContextSnapshot } from "$lib/project/io";
import type {
  AiContextStatus,
  CenterView,
  ExternalDiskState,
  ProjectScan,
  ProjectWorkspaceSnapshot,
  ScssVariable,
  CoordinatedElementSelection,
  SelectionSnapshot,
  SourceLanguage,
  UiContextProjection,
} from "$lib/types";

const AI_CONTEXT_WRITE_DELAY = 450;

export type AiContextControllerHost = {
  aiContextStatus: AiContextStatus | null;
  aiContextSaveTimer: number | null;
  aiContextUiRevision: number;
  scannedProject: ProjectScan | null;
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  activeScannedPath: string | null;
  activePreviewPath: string;
  centerView: CenterView;
  previewDevice: "desktop" | "tablet" | "mobile";
  sourceLanguage: SourceLanguage;
  coordinatedElementSelection: CoordinatedElementSelection | null;
  selectionSnapshot: SelectionSnapshot | null;
  activeCssSelector: string;
  targetCssFile: string;
  scssVariables: ScssVariable[];
  globalDirtyState: {
    dirty: boolean;
    canSave: boolean;
    areas: string[];
    immediateDiskOperationBlockedReason: string;
  };
  externalDiskState: ExternalDiskState;
};

export function buildAiContextProjection(
  host: AiContextControllerHost,
  uiRevision: number,
): UiContextProjection {
  const project = host.scannedProject;
  const workspace = host.projectWorkspaceSnapshot;
  const selected = host.coordinatedElementSelection;
  const observation = selected?.observation ?? null;
  const coordinated = host.selectionSnapshot;
  const sourceReference =
    coordinated?.provenance?.definition
    ?? coordinated?.provenance?.composition
    ?? null;
  const coordinatedLocation = sourceReference
    ? {
        file: sourceReference.file,
        line: sourceReference.range?.line ?? 1,
        column: sourceReference.range?.column ?? 1,
      }
    : null;
  const cssFocus =
    coordinated?.focus.kind === "cssRule"
    || coordinated?.focus.kind === "cssProperty"
      ? coordinated.focus
      : null;

  return {
    schemaVersion: 3,
    uiRevision,
    expectedProjectSessionId: workspace?.runtimeSessionId ?? null,
    expectedProjectRevision: workspace?.revision ?? null,
    project: {
      isOpen: Boolean(project),
      previewBaseUrl: project?.previewBaseUrl ?? null,
      previewWarning: project?.previewWarning ?? null,
    },
    workspace: {
      centerView: host.centerView,
      previewDevice: host.previewDevice,
      activeFile: host.activeScannedPath,
      activePreviewPath: host.activePreviewPath === "about:blank" ? null : host.activePreviewPath,
      sourceLanguage: host.sourceLanguage,
    },
    selection: {
      hasSelection: Boolean(coordinated?.subject),
      selector: observation?.selector ?? null,
      cssSelector: observation?.cssSelector ?? null,
      tag: coordinated?.subject?.tag ?? observation?.tag ?? null,
      id: observation?.id ?? null,
      classes: observation?.classes ?? [],
      text: observation?.text ?? null,
      imageSrc: observation?.imageSrc ?? null,
      sourceLocation: coordinatedLocation,
      sourceId: coordinated?.anchor?.sourceNodeId ?? null,
      templateSourceId:
        coordinated?.subject?.kind === "teraBoundary"
          ? coordinated.anchor?.sourceNodeId ?? null
          : null,
      sessionId: coordinated?.runtimeSessionId ?? null,
      rect: observation?.rect ?? null,
    },
    css: {
      activeSelector:
        cssFocus?.selector
        ?? (host.activeCssSelector || observation?.cssSelector || null),
      targetFile: cssFocus?.file ?? (host.targetCssFile || null),
      variablesCount: host.scssVariables.length,
    },
    uiDirtyState: {
      dirty: host.globalDirtyState.dirty,
      canSave: host.globalDirtyState.canSave,
      areas: host.globalDirtyState.areas,
      blockedReason: host.globalDirtyState.immediateDiskOperationBlockedReason,
    },
    externalDisk: {
      changed: host.externalDiskState.changed,
      changedFiles: host.externalDiskState.changedFiles,
      activeFileChanged: host.externalDiskState.activeFileChanged,
      previewRelevantChanged: host.externalDiskState.previewRelevantChanged,
      blockedByDirtySession: host.externalDiskState.blockedByDirtySession,
      lastDetectedAt: host.externalDiskState.lastDetectedAt,
      lastDetectedFiles: host.externalDiskState.lastDetectedFiles,
      lastDetectedActiveFileChanged: host.externalDiskState.lastDetectedActiveFileChanged,
      lastDetectedPreviewRelevantChanged: host.externalDiskState.lastDetectedPreviewRelevantChanged,
      lastAppliedAt: host.externalDiskState.lastAppliedAt,
      lastAppliedFiles: host.externalDiskState.lastAppliedFiles,
      lastCheckedAt: host.externalDiskState.lastCheckedAt,
      checking: host.externalDiskState.checking,
      reconciling: host.externalDiskState.reconciling,
      workspaceProjectionRecoveryRequired: host.externalDiskState.workspaceProjectionRecoveryRequired,
      truncated: host.externalDiskState.truncated,
    },
  };
}

export function scheduleAiContextSnapshot(host: AiContextControllerHost) {
  if (typeof window === "undefined") return;
  if (host.aiContextSaveTimer !== null) {
    window.clearTimeout(host.aiContextSaveTimer);
  }
  host.aiContextSaveTimer = window.setTimeout(() => {
    host.aiContextSaveTimer = null;
    host.aiContextUiRevision = Math.max(host.aiContextUiRevision + 1, Date.now());
    void saveAiContextSnapshot(buildAiContextProjection(host, host.aiContextUiRevision))
      .then((status) => {
        host.aiContextStatus = status;
      })
      .catch(() => {
        // Contextul AI este ajutător, nu trebuie să blocheze editorul.
      });
  }, AI_CONTEXT_WRITE_DELAY);
}

export function clearAiContextTimer(host: AiContextControllerHost) {
  if (host.aiContextSaveTimer === null || typeof window === "undefined") return;
  window.clearTimeout(host.aiContextSaveTimer);
  host.aiContextSaveTimer = null;
}
