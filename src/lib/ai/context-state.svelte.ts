import { saveAiContextSnapshot } from "$lib/ai/io";
import type { ScssVariable } from "$lib/css/contracts";
import { primarySelectionEntry } from "$lib/kernel/selection-read-model";
import type { EditorSelectionSessionController } from "$lib/state/editor-selection-session.svelte";
import type {
  AiContextStatus,
  UiContextProjection,
} from "$lib/ai/contracts";
import type {
  CenterView,
  SourceLanguage,
} from "$lib/application/contracts";
import type { CoordinatedElementSelection } from "$lib/canvas/contracts";
import type { ExternalDiskState } from "$lib/project/external-disk-contract";
import type { ProjectScan } from "$lib/project/lifecycle-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";

const AI_CONTEXT_WRITE_DELAY = 450;

export type AiContextProjectionInput = Readonly<{
  project: ProjectScan | null;
  workspace: ProjectWorkspaceSnapshot | null;
  activeScannedPath: string | null;
  activePreviewPath: string;
  centerView: CenterView;
  previewDevice: "desktop" | "tablet" | "mobile";
  sourceLanguage: SourceLanguage;
  coordinatedElementSelection: CoordinatedElementSelection | null;
  editorSelection: Pick<EditorSelectionSessionController, "selectionSnapshot">;
  activeCssSelector: string;
  targetCssFile: string;
  scssVariables: ScssVariable[];
  dirtyState: {
    dirty: boolean;
    canSave: boolean;
    areas: string[];
    immediateDiskOperationBlockedReason: string;
  };
  externalDisk: ExternalDiskState;
}>;

export function buildAiContextProjection(
  input: AiContextProjectionInput,
  uiRevision: number,
): UiContextProjection {
  const selected = input.coordinatedElementSelection;
  const observation = selected?.observation ?? null;
  const coordinated = input.editorSelection.selectionSnapshot;
  const primary = primarySelectionEntry(coordinated);
  const sourceReference = primary?.provenance.definition ?? primary?.provenance.composition ?? null;
  const coordinatedLocation = sourceReference
    ? {
        file: sourceReference.file,
        line: sourceReference.range?.line ?? 1,
        column: sourceReference.range?.column ?? 1,
      }
    : null;
  const cssFocus = coordinated?.focus.kind === "cssRule" || coordinated?.focus.kind === "cssProperty"
    ? coordinated.focus
    : null;

  return {
    schemaVersion: 4,
    uiRevision,
    expectedProjectSessionId: input.workspace?.runtimeSessionId ?? null,
    expectedProjectRevision: input.workspace?.revision ?? null,
    project: {
      isOpen: Boolean(input.project),
      previewBaseUrl: input.project?.previewBaseUrl ?? null,
      previewWarning: input.project?.previewWarning ?? null,
    },
    workspace: {
      centerView: input.centerView,
      previewDevice: input.previewDevice,
      activeFile: input.activeScannedPath,
      activePreviewPath: input.activePreviewPath === "about:blank" ? null : input.activePreviewPath,
      sourceLanguage: input.sourceLanguage,
    },
    selection: {
      hasSelection: Boolean(primary),
      primaryMemberId: coordinated?.primaryMemberId ?? null,
      memberIds: coordinated?.members.map((member) => member.memberId) ?? [],
      memberCount: coordinated?.members.length ?? 0,
      selector: observation?.selector ?? null,
      cssSelector: observation?.cssSelector ?? null,
      tag: primary?.subject.tag ?? observation?.tag ?? null,
      id: observation?.id ?? null,
      classes: observation?.classes ?? [],
      text: observation?.text ?? null,
      imageSrc: observation?.imageSrc ?? null,
      sourceLocation: coordinatedLocation,
      sourceId: primary?.anchor.sourceNodeId ?? null,
      templateSourceId: primary?.subject.kind === "boundary"
        && primary.subject.boundaryKind !== "markdown"
        ? primary.anchor.sourceNodeId ?? null
        : null,
      sessionId: coordinated?.runtimeSessionId ?? null,
      rect: observation?.rect ?? null,
    },
    css: {
      activeSelector: cssFocus?.selector
        ?? (input.activeCssSelector || observation?.cssSelector || null),
      targetFile: cssFocus?.file ?? (input.targetCssFile || null),
      variablesCount: input.scssVariables.length,
    },
    uiDirtyState: {
      dirty: input.dirtyState.dirty,
      canSave: input.dirtyState.canSave,
      areas: input.dirtyState.areas,
      blockedReason: input.dirtyState.immediateDiskOperationBlockedReason,
    },
    externalDisk: {
      changed: input.externalDisk.changed,
      changedFiles: input.externalDisk.changedFiles,
      activeFileChanged: input.externalDisk.activeFileChanged,
      previewRelevantChanged: input.externalDisk.previewRelevantChanged,
      blockedByDirtySession: input.externalDisk.blockedByDirtySession,
      lastDetectedAt: input.externalDisk.lastDetectedAt,
      lastDetectedFiles: input.externalDisk.lastDetectedFiles,
      lastDetectedActiveFileChanged: input.externalDisk.lastDetectedActiveFileChanged,
      lastDetectedPreviewRelevantChanged: input.externalDisk.lastDetectedPreviewRelevantChanged,
      lastAppliedAt: input.externalDisk.lastAppliedAt,
      lastAppliedFiles: input.externalDisk.lastAppliedFiles,
      lastCheckedAt: input.externalDisk.lastCheckedAt,
      checking: input.externalDisk.checking,
      reconciling: input.externalDisk.reconciling,
      workspaceProjectionRecoveryRequired: input.externalDisk.workspaceProjectionRecoveryRequired,
      truncated: input.externalDisk.truncated,
    },
  };
}

/** Owns the debounced MCP/UI projection and its timer cleanup. */
export class AiContextState {
  status = $state<AiContextStatus | null>(null);
  uiRevision = Date.now();
  private timer: number | null = null;
  private readonly readInput: () => AiContextProjectionInput;

  constructor(readInput: () => AiContextProjectionInput) {
    this.readInput = readInput;
  }

  schedule() {
    const input = this.readInput();
    if (typeof window === "undefined") return;
    if (this.timer !== null) window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => {
      this.timer = null;
      this.uiRevision = Math.max(this.uiRevision + 1, Date.now());
      void saveAiContextSnapshot(buildAiContextProjection(input, this.uiRevision))
        .then((status) => { this.status = status; })
        .catch(() => {
          // Contextul AI este ajutător și nu blochează editorul.
        });
    }, AI_CONTEXT_WRITE_DELAY);
  }

  clear() {
    if (this.timer === null || typeof window === "undefined") return;
    window.clearTimeout(this.timer);
    this.timer = null;
  }
}
