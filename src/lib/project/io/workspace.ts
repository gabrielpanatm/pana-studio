import type {
  FileBufferChangeSetInput,
  FileBufferChangeSetResult,
  FileBufferCommandReceipt,
  FileBufferFileSnapshot,
  FileBufferMutationExpectation,
  FileBufferRequestIdentity,
  FileBufferTextSnapshot,
  ProjectWorkspaceHistoryIdentity,
  ProjectWorkspaceIdentity,
  ProjectWorkspaceSaveReceipt,
  ProjectWorkspaceSnapshot,
  ProjectWorkspaceUndoRedoCommandReceipt,
} from "$lib/project/workspace-contract";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";

export function readProjectWorkspaceState(): Promise<ProjectWorkspaceSnapshot | null> {
  return invoke<ProjectWorkspaceSnapshot | null>("read_project_workspace_state");
}

export function saveProjectWorkspace(
  identity: ProjectWorkspaceIdentity,
): Promise<ProjectWorkspaceSaveReceipt> {
  return invoke<ProjectWorkspaceSaveReceipt>("save_project_workspace", { identity });
}

export function undoProjectWorkspace(
  identity: ProjectWorkspaceHistoryIdentity,
): Promise<ProjectWorkspaceUndoRedoCommandReceipt> {
  return invoke<ProjectWorkspaceUndoRedoCommandReceipt>("undo_project_workspace", { identity });
}

export function redoProjectWorkspace(
  identity: ProjectWorkspaceHistoryIdentity,
): Promise<ProjectWorkspaceUndoRedoCommandReceipt> {
  return invoke<ProjectWorkspaceUndoRedoCommandReceipt>("redo_project_workspace", { identity });
}

export function readFileBufferText(
  relativePath: string,
  identity: FileBufferRequestIdentity,
): Promise<FileBufferTextSnapshot> {
  return invokeBoundFileBuffer<FileBufferTextSnapshot>(
    "read_file_buffer_text",
    { relativePath, identity },
    identity,
  );
}

export function setFileBufferDraft(
  relativePath: string,
  contents: string,
  expectation: FileBufferMutationExpectation,
  identity: FileBufferRequestIdentity,
): Promise<FileBufferFileSnapshot> {
  return invokeBoundFileBuffer<FileBufferFileSnapshot>(
    "set_file_buffer_draft",
    { relativePath, contents, expectation, identity },
    identity,
  );
}

export function applyFileBufferChangeSet(
  input: FileBufferChangeSetInput,
  identity: FileBufferRequestIdentity,
): Promise<FileBufferChangeSetResult> {
  return invokeBoundFileBuffer<FileBufferChangeSetResult>(
    "apply_file_buffer_changeset",
    { input, identity },
    identity,
  );
}

export function clearFileBufferDraft(
  relativePath: string,
  expectation: FileBufferMutationExpectation,
  identity: FileBufferRequestIdentity,
): Promise<FileBufferFileSnapshot> {
  return invokeBoundFileBuffer<FileBufferFileSnapshot>(
    "clear_file_buffer_draft",
    { relativePath, expectation, identity },
    identity,
  );
}

async function invokeBoundFileBuffer<T>(
  command: string,
  args: Record<string, unknown>,
  identity: FileBufferRequestIdentity,
): Promise<T> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    throw new Error(
      t("io-file-buffer-identity-invalid"),
    );
  }
  const receipt = await invoke<FileBufferCommandReceipt<T>>(command, args);
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || !Number.isSafeInteger(receipt.workspaceRevision)
    || receipt.workspaceRevision < 0
  ) {
    throw new Error(
      t("io-file-buffer-stale-receipt", {
        command,
        expectedRoot: identity.expectedProjectRoot,
        expectedSession: identity.expectedSessionId,
        actualRoot: receipt.projectRoot,
        actualSession: receipt.runtimeSessionId,
      }),
    );
  }
  return receipt.payload;
}

export function readProjectFile(relativePath: string): Promise<string> {
  return invoke<string>("read_project_file", { relativePath });
}
