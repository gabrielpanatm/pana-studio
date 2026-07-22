import {
  applyWorkbenchIntent,
  readWorkbenchState,
  workbenchIdentity,
} from "$lib/workbench/io";
import type {
  CenterView,
  ProjectFile,
  WorkbenchCommandReceipt,
  WorkbenchIntent,
  WorkbenchSnapshot,
  WorkbenchSurface,
} from "$lib/types";

export type WorkbenchProjectionHost = {
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  workbenchSnapshot: WorkbenchSnapshot | null;
};

export class WorkbenchProjectionController {
  private commandTail: Promise<void> = Promise.resolve();
  private refreshSerial = 0;

  constructor(private readonly host: () => WorkbenchProjectionHost) {}

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
        throw new Error("Workbench nu are o ProjectSession activă.");
      }

      const receipt = await applyWorkbenchIntent(workbenchIdentity(snapshot), intent);
      const current = this.host();
      if (
        current.sessionProjectRoot !== receipt.projectRoot
        || current.kernelProjectSessionId !== receipt.runtimeSessionId
      ) {
        throw new Error("Workbench a ignorat un receipt pentru o ProjectSession închisă.");
      }
      current.workbenchSnapshot = receipt.snapshot;
      return receipt;
    });
    this.commandTail = operation.then(() => undefined, () => undefined);
    return operation;
  }

  async openDocument(file: ProjectFile, centerView: CenterView): Promise<WorkbenchCommandReceipt> {
    const snapshot = this.host().workbenchSnapshot ?? await this.refresh();
    if (snapshot && snapshot.split !== "none") {
      return this.apply({
        kind: "configure_synchronized_split",
        split: snapshot.split,
        relativePath: file.relativePath,
        secondarySurface: /\.md$/i.test(file.relativePath) ? "markdown" : "code",
      });
    }
    return this.apply({
      kind: "open_document",
      relativePath: file.relativePath,
      groupId: this.host().workbenchSnapshot?.activeGroupId ?? "primary",
      surface: workbenchSurface(centerView),
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
    const surface = workbenchSurface(centerView);
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
  if (centerView === "markdown") return "markdown";
  if (centerView === "code") return "code";
  return "visual";
}
