import type { WorkspaceEntryMutationReceipt } from "$lib/project/workspace-contract";

type DataMutationOperation =
  | "create_file"
  | "update_node"
  | "insert_child"
  | "delete_node";

export type DataDraftKind =
  | "string"
  | "integer"
  | "float"
  | "boolean"
  | "datetime"
  | "array"
  | "inline_table"
  | "table"
  | "array_of_tables";

export type DataMutationInput = {
  operation: DataMutationOperation;
  file: string;
  nodeId: string | null;
  key: string | null;
  draftKind: DataDraftKind | null;
  value: string | null;
};

type DataMutationPlan = {
  schemaVersion: 1;
  operation: DataMutationOperation;
  file: string;
  nodeId: string | null;
  touchedFiles: string[];
};

export type DataMutationApplyReceipt = {
  plan: DataMutationPlan;
  workspace: WorkspaceEntryMutationReceipt;
};

export type DataNodeEditorSnapshot = {
  schemaVersion: 1;
  file: string;
  nodeId: string;
  key: string | null;
  draftKind: DataDraftKind | null;
  value: string | null;
  editableKey: boolean;
  editableValue: boolean;
};
