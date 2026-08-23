import type {
  FileBufferCommandReceipt,
  ProjectWorkspaceMutationReceipt,
  WorkspaceDocumentProjection,
} from "$lib/project/workspace-contract";

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

export type ReusableCssWriteResult = {
  file: string;
  stylesheetCreated: boolean;
  consumerFiles: string[];
  consumerTemplates: string[];
  writtenFiles: WrittenProjectFile[];
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
