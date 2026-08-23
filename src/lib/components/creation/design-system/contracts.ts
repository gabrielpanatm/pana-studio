import type { CssMutationAuthorityReceipt } from "$lib/css/mutation-contract";
import type { ScssVariable } from "$lib/css/contracts";
import type {
  FileExplorerCommitReceipt,
  FileExplorerOperationPlan,
  FileExplorerOperationRequest,
  FileExplorerSnapshot,
} from "$lib/project/file-explorer-contract";

export type DesignView = "global-styles" | "tokens" | "classes" | "styles" | "fonts";
export type DetailMode = "info" | "create" | "edit";

export type DesignSystemCommands = {
  refreshClassInventory: () => Promise<unknown>;
  createVariable: (path: string, name: string, value: string) => Promise<boolean>;
  createClass: (name: string, path: string) => Promise<boolean>;
  updateVariable: (variable: ScssVariable, value: string) => Promise<boolean>;
  renameClass: (oldName: string, newName: string) => Promise<boolean>;
  refreshFileExplorer: () => Promise<FileExplorerSnapshot | null>;
  planFileExplorer: (request: FileExplorerOperationRequest) => Promise<FileExplorerOperationPlan>;
  commitFileExplorer: (plan: FileExplorerOperationPlan) => Promise<FileExplorerCommitReceipt>;
  injectRawCss: (id: string, css: string) => void;
  projectCommittedCssMutation: (
    authority: CssMutationAuthorityReceipt,
    liveEpoch: number | null,
  ) => Promise<unknown>;
};
