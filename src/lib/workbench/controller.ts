import {
  applyWorkbenchIntent,
  readWorkbenchState,
  workbenchIdentity,
} from "$lib/workbench/io";
import type { CenterView } from "$lib/application/contracts";
import type { ProjectFile } from "$lib/project/lifecycle-contract";
import type {
  WorkbenchCommandReceipt,
  WorkbenchIntent,
  WorkbenchSnapshot,
  WorkbenchSurface,
} from "$lib/workbench/contracts";
import {
  patchExactWorkbenchActivityChange,
  patchExactWorkbenchDocumentActivation,
} from "$lib/workbench/activation-projection";
import { t } from "$lib/i18n/runtime.svelte";
import {
  activeWorkbenchDocument,
  workbenchPresentationForProjectFile,
} from "$lib/workbench/document-presentation";

export type WorkbenchProjectionHost = {
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  workbenchSnapshot: WorkbenchSnapshot | null;
  projectActiveDocument: (previous: ReturnType<typeof activeWorkbenchDocument>) => void;
};

export class WorkbenchProjectionController {
  private commandTail: Promise<void> = Promise.resolve();
  private refreshSerial = 0;
  private readonly host: () => WorkbenchProjectionHost;

  constructor(host: () => WorkbenchProjectionHost) {
    this.host = host;
  }

  reset() {
    this.refreshSerial += 1;
    this.host().workbenchSnapshot = null;
  }

  async refresh(): Promise<WorkbenchSnapshot | null> {
    const host = this.host();
    const projectRoot = host.sessionProjectRoot;
    const runtimeSessionId = host.kernelProjectSessionId;
    const serial = ++this.refreshSerial;
    if (!projectRoot || !runtimeSessionId) {
      host.workbenchSnapshot = null;
      return null;
    }

    const snapshot = await readWorkbenchState();
    const current = this.host();
    if (
      serial !== this.refreshSerial
      || current.sessionProjectRoot !== projectRoot
      || current.kernelProjectSessionId !== runtimeSessionId
    ) return current.workbenchSnapshot;

    current.workbenchSnapshot = snapshot?.projectRoot === projectRoot
      && snapshot.runtimeSessionId === runtimeSessionId
      ? snapshot
      : null;
    return current.workbenchSnapshot;
  }

  apply(intent: WorkbenchIntent): Promise<WorkbenchCommandReceipt> {
    const operation = this.commandTail.then(async () => {
      const host = this.host();
      const projectRoot = host.sessionProjectRoot;
      const runtimeSessionId = host.kernelProjectSessionId;
      let snapshot = host.workbenchSnapshot;
      if (
        !snapshot
        || snapshot.projectRoot !== projectRoot
        || snapshot.runtimeSessionId !== runtimeSessionId
      ) {
        snapshot = await this.refresh();
      }
      if (!snapshot || !projectRoot || !runtimeSessionId) {
        throw new Error(t("workbench-session-missing"));
      }

      const receipt = await applyWorkbenchIntent(workbenchIdentity(snapshot), intent);
      const current = this.host();
      if (
        current.sessionProjectRoot !== receipt.projectRoot
        || current.kernelProjectSessionId !== receipt.runtimeSessionId
      ) {
        throw new Error(t("workbench-closed-session-receipt"));
      }
      const previousActiveDocument = activeWorkbenchDocument(current.workbenchSnapshot);
      const patched = intent.kind === "activate_document"
        ? patchExactWorkbenchDocumentActivation(
          current.workbenchSnapshot,
          receipt.snapshot,
          intent,
        )
        : intent.kind === "set_activity"
          ? patchExactWorkbenchActivityChange(
            current.workbenchSnapshot,
            receipt.snapshot,
            intent,
          )
          : false;
      if (!patched) {
        current.workbenchSnapshot = receipt.snapshot;
      } else {
        current.projectActiveDocument(previousActiveDocument);
      }
      return receipt;
    });
    this.commandTail = operation.then(() => undefined, () => undefined);
    return operation;
  }

  async openDocument(file: ProjectFile, centerView: CenterView): Promise<WorkbenchCommandReceipt> {
    const snapshot = this.host().workbenchSnapshot ?? await this.refresh();
    const presentation = workbenchPresentationForProjectFile(file);
    if (presentation === "html" && snapshot && snapshot.split !== "none") {
      return this.apply({
        kind: "configure_synchronized_split",
        split: snapshot.split,
        relativePath: file.relativePath,
        secondarySurface: "code",
        presentation,
      });
    }
    return this.apply({
      kind: "open_document",
      relativePath: file.relativePath,
      groupId: this.host().workbenchSnapshot?.activeGroupId ?? "primary",
      surface: presentation === "html" ? workbenchSurface(centerView) : "code",
      presentation,
    });
  }

  async setActiveDocumentSurface(
    relativePath: string,
    centerView: CenterView,
  ): Promise<WorkbenchCommandReceipt | null> {
    const snapshot = this.host().workbenchSnapshot ?? await this.refresh();
    if (!snapshot) return null;
    if (snapshot.split !== "none") return null;
    const group = snapshot.groups.find((candidate) => candidate.groupId === snapshot.activeGroupId);
    if (!group) return null;
    const document = group?.documents.find((candidate) => candidate.relativePath === relativePath);
    if (!document) return null;
    const surface = document.presentation === "html" ? workbenchSurface(centerView) : "code";
    if (document.surface === surface) return null;
    return this.apply({
      kind: "set_document_surface",
      documentId: document.documentId,
      groupId: group.groupId,
      surface,
    });
  }
}

function workbenchSurface(centerView: CenterView): WorkbenchSurface {
  if (centerView === "code") return "code";
  return "visual";
}
