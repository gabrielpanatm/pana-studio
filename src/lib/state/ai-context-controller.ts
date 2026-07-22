import { saveAiContextSnapshot } from "$lib/project/io";
import type {
  AiContextStatus,
  CenterView,
  ExternalDiskState,
  ProjectScan,
  ProjectWorkspaceSnapshot,
  ScssVariable,
  SelectionInfo,
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
  selectedElement: SelectionInfo | null;
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
  const selected = host.selectedElement;

  return {
    schemaVersion: 2,
    uiRevision,
    expectedProjectSessionId: workspace?.runtimeSessionId ?? null,
    expectedProjectRevision: workspace?.revision ?? null,
    project: {
      isZola: project?.isZola ?? false,
      isEmpty: project?.isEmpty ?? true,
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
      hasSelection: Boolean(selected),
      selector: selected?.selector ?? null,
      cssSelector: selected?.cssSelector ?? null,
      tag: selected?.tag ?? null,
      id: selected?.id ?? null,
      classes: selected?.classes ?? [],
      text: selected?.text ?? null,
      imageSrc: selected?.imageSrc ?? null,
      sourceLocation: selected?.sourceLocation ?? null,
      sourceId: selected?.sourceId ?? null,
      templateSourceId: selected?.templateSourceId ?? null,
      sessionId: selected?.sessionId ?? null,
      rect: selected?.rect ?? null,
    },
    css: {
      activeSelector: host.activeCssSelector || selected?.cssSelector || null,
      targetFile: host.targetCssFile || null,
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
