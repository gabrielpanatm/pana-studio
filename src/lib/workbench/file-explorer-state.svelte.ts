import { t } from "$lib/i18n/runtime.svelte";
import {
  commitFileExplorerOperation as commitFileExplorerOperationInRust,
  planFileExplorerOperation as planFileExplorerOperationInRust,
  readFileExplorerSnapshot,
  selectFileExplorerEntry as selectFileExplorerEntryInRust,
} from "$lib/project/file-explorer";
import type {
  FileExplorerCommitReceipt,
  FileExplorerEntry,
  FileExplorerOperationPlan,
  FileExplorerOperationRequest,
  FileExplorerSnapshot,
} from "$lib/project/file-explorer-contract";
import type { ProjectFile } from "$lib/project/lifecycle-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import type { WorkbenchSnapshot } from "$lib/workbench/contracts";
import type {
  WorkspaceMutationAuthorityReceipt,
  WorkspaceMutationSettlementOptions,
} from "$lib/session/workspace-mutation-coordinator";
import { errorMessage } from "$lib/util";

export type FileExplorerAuthority = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  workspace: ProjectWorkspaceSnapshot | null;
  workbenchRevision: number | null;
  activeRelativePath: string | null;
}>;

export type FileExplorerWorkspaceCommands = {
  authority: () => FileExplorerAuthority;
  refreshWorkbench: () => Promise<WorkbenchSnapshot | null>;
  acceptWorkbench: (snapshot: WorkbenchSnapshot) => void;
  loadProjectFile: (file: ProjectFile, options: { syncWorkbench: false }) => Promise<unknown>;
  settleMutation: (
    mutation: WorkspaceMutationAuthorityReceipt,
    options: WorkspaceMutationSettlementOptions,
  ) => Promise<unknown>;
  setStatus: (text: string, kind: "error" | "unsaved") => void;
};

function projectFileFromExplorerEntry(entry: FileExplorerEntry): ProjectFile {
  return {
    name: entry.name,
    relativePath: entry.relativePath,
    absolutePath: entry.absolutePath,
    kind: entry.fileKind,
    role: entry.role,
    previewPath: entry.previewPath,
  };
}

/** Owns the Rust-authoritative file tree, selection ordering and mutations. */
export class FileExplorerWorkspaceState {
  snapshot = $state<FileExplorerSnapshot | null>(null);
  loading = $state(false);
  error = $state("");

  private requestSerial = 0;
  private selectionSerial = 0;
  private selectionTail: Promise<void> = Promise.resolve();

  constructor(private readonly commands: FileExplorerWorkspaceCommands) {}

  reset() {
    this.requestSerial += 1;
    this.selectionSerial += 1;
    this.snapshot = null;
    this.loading = false;
    this.error = "";
  }

  async refresh() {
    const authority = this.commands.authority();
    const workspace = authority.workspace;
    if (
      !workspace
      || workspace.projectRoot !== authority.projectRoot
      || workspace.runtimeSessionId !== authority.runtimeSessionId
    ) {
      this.reset();
      return null;
    }
    const serial = ++this.requestSerial;
    const identity = {
      expectedProjectRoot: workspace.projectRoot,
      expectedSessionId: workspace.runtimeSessionId,
      expectedRevision: workspace.revision,
    };
    this.loading = true;
    try {
      const snapshot = await readFileExplorerSnapshot(identity);
      let current = this.commands.authority();
      if (
        serial !== this.requestSerial
        || current.projectRoot !== identity.expectedProjectRoot
        || current.runtimeSessionId !== identity.expectedSessionId
        || current.workspace?.revision !== identity.expectedRevision
      ) return this.snapshot;
      this.snapshot = snapshot;
      this.error = "";
      if (current.workbenchRevision !== snapshot.workbenchRevision) {
        await this.commands.refreshWorkbench();
        current = this.commands.authority();
        if (
          serial !== this.requestSerial
          || current.projectRoot !== identity.expectedProjectRoot
          || current.runtimeSessionId !== identity.expectedSessionId
        ) return this.snapshot;
      }
      return snapshot;
    } catch (error) {
      if (serial !== this.requestSerial) return this.snapshot;
      this.error = errorMessage(error);
      return null;
    } finally {
      if (serial === this.requestSerial) this.loading = false;
    }
  }

  async resolveProjectFile(relativePath: string): Promise<ProjectFile | null> {
    const workspace = this.commands.authority().workspace;
    if (!workspace) return null;
    let explorer = this.snapshot;
    if (
      !explorer
      || explorer.projectRoot !== workspace.projectRoot
      || explorer.runtimeSessionId !== workspace.runtimeSessionId
      || explorer.workspaceRevision !== workspace.revision
    ) explorer = await this.refresh();
    const entry = explorer?.entries.find(
      (candidate) => candidate.relativePath === relativePath && candidate.kind === "text",
    );
    return entry ? projectFileFromExplorerEntry(entry) : null;
  }

  select(entryId: string) {
    const serial = ++this.selectionSerial;
    const selection = this.selectionTail.then(async () => {
      if (serial !== this.selectionSerial) return;
      await this.commitSelection(entryId, serial);
    });
    this.selectionTail = selection.catch(() => {});
    return selection;
  }

  private async commitSelection(entryId: string, serial: number) {
    const explorer = this.snapshot;
    let authority = this.commands.authority();
    const workspace = authority.workspace;
    if (
      !explorer
      || !workspace
      || explorer.projectRoot !== workspace.projectRoot
      || explorer.runtimeSessionId !== workspace.runtimeSessionId
      || explorer.workspaceRevision !== workspace.revision
    ) {
      await this.refresh();
      return;
    }
    try {
      if (authority.workbenchRevision === null) {
        await this.commands.refreshWorkbench();
        authority = this.commands.authority();
      }
      if (
        serial !== this.selectionSerial
        || authority.projectRoot !== explorer.projectRoot
        || authority.runtimeSessionId !== explorer.runtimeSessionId
        || authority.workspace?.revision !== explorer.workspaceRevision
        || authority.workbenchRevision === null
      ) return;
      const receipt = await selectFileExplorerEntryInRust({
        identity: {
          expectedProjectRoot: explorer.projectRoot,
          expectedSessionId: explorer.runtimeSessionId,
          expectedRevision: explorer.workspaceRevision,
        },
        expectedWorkbenchRevision: authority.workbenchRevision,
        entryId,
      });
      authority = this.commands.authority();
      if (
        authority.projectRoot !== receipt.projectRoot
        || authority.runtimeSessionId !== receipt.runtimeSessionId
        || authority.workspace?.revision !== receipt.workspaceRevision
      ) return;
      this.snapshot = receipt.snapshot;
      this.commands.acceptWorkbench(receipt.workbench.snapshot);
      this.error = "";
      if (serial !== this.selectionSerial) return;
      const selection = receipt.snapshot.selectedEntry;
      if (!selection || selection.kind !== "text") return;
      const entry = receipt.snapshot.entries.find(
        (candidate) => candidate.relativePath === selection.relativePath
          && candidate.kind === "text",
      );
      if (!entry) throw new Error(t("workbench-document-missing", { path: selection.relativePath }));
      await this.commands.loadProjectFile(projectFileFromExplorerEntry(entry), { syncWorkbench: false });
    } catch (error) {
      if (serial !== this.selectionSerial) return;
      this.error = errorMessage(error);
      this.commands.setStatus(this.error, "error");
    }
  }

  async plan(operation: FileExplorerOperationRequest): Promise<FileExplorerOperationPlan> {
    const explorer = this.snapshot;
    let authority = this.commands.authority();
    const workspace = authority.workspace;
    if (
      !explorer
      || !workspace
      || explorer.projectRoot !== workspace.projectRoot
      || explorer.runtimeSessionId !== workspace.runtimeSessionId
      || explorer.workspaceRevision !== workspace.revision
    ) throw new Error(t("project-files-projection-unavailable"));
    if (authority.workbenchRevision === null) {
      await this.commands.refreshWorkbench();
      authority = this.commands.authority();
    }
    if (
      authority.projectRoot !== explorer.projectRoot
      || authority.runtimeSessionId !== explorer.runtimeSessionId
      || authority.workspace?.revision !== explorer.workspaceRevision
      || authority.workbenchRevision === null
    ) throw new Error(t("project-files-projection-unavailable"));
    return planFileExplorerOperationInRust({
      identity: {
        expectedProjectRoot: explorer.projectRoot,
        expectedSessionId: explorer.runtimeSessionId,
        expectedRevision: explorer.workspaceRevision,
      },
      expectedWorkbenchRevision: authority.workbenchRevision,
      operation,
    });
  }

  async commit(plan: FileExplorerOperationPlan): Promise<FileExplorerCommitReceipt> {
    if (!plan.allowed || !plan.commitToken) {
      throw new Error(plan.diagnostic ?? t("project-files-plan-blocked"));
    }
    let authority = this.commands.authority();
    const workspace = authority.workspace;
    if (
      !workspace
      || workspace.projectRoot !== plan.projectRoot
      || workspace.runtimeSessionId !== plan.runtimeSessionId
      || workspace.revision !== plan.workspaceRevision
      || workspace.diskGeneration !== plan.acceptedDiskGeneration
    ) throw new Error(t("project-files-plan-stale"));
    const receipt = await commitFileExplorerOperationInRust({
      identity: {
        expectedProjectRoot: plan.projectRoot,
        expectedSessionId: plan.runtimeSessionId,
        expectedRevision: plan.workspaceRevision,
      },
      expectedAcceptedDiskGeneration: plan.acceptedDiskGeneration,
      commitToken: plan.commitToken,
    });
    authority = this.commands.authority();
    if (
      authority.projectRoot !== receipt.projectRoot
      || authority.runtimeSessionId !== receipt.runtimeSessionId
    ) return receipt;
    const selected = receipt.snapshot.selectedEntry;
    await this.commands.settleMutation(receipt.mutation, {
      preferredRelativePath: selected?.kind === "text"
        ? selected.relativePath
        : authority.activeRelativePath,
      warningLabel: t("project-files-operation"),
    });
    authority = this.commands.authority();
    if (
      authority.projectRoot === receipt.projectRoot
      && authority.runtimeSessionId === receipt.runtimeSessionId
      && authority.workspace?.revision === receipt.mutation.workspace.revision
    ) {
      this.commands.acceptWorkbench(receipt.workbench.snapshot);
      this.snapshot = receipt.snapshot;
      this.error = "";
    }
    this.commands.setStatus(t("project-files-mutation-staged"), "unsaved");
    return receipt;
  }
}
