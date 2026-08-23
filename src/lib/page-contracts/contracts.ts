import type {
  PageJsConfig,
  PageJsDraftStageReceipt,
} from "$lib/js/contracts";
import type { ProjectWorkspaceMutationReceipt } from "$lib/project/workspace-contract";

type PageContractApplyStatus = "noop" | "staged";

type PageContractConsumedSourceRevision = {
  relativePath: string;
  beforeRevision: number | null;
  beforeHash: string | null;
  afterRevision: number | null;
  afterHash: string | null;
};

export type PageContractAuthorityReceipt = {
  schemaVersion: 2;
  operationId: string;
  status: PageContractApplyStatus;
  projectRoot: string;
  sessionId: string;
  revisionBefore: number;
  revisionAfter: number;
  dirty: boolean;
  consumedSources: PageContractConsumedSourceRevision[];
  touchedFiles: string[];
};

type PageAssetContractTextPlan = {
  changed: boolean;
  contents: string;
};

export type PageAssetContractApplyInput = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  templatePath: string;
};

type PageAssetContractPlan = {
  templatePath: string;
  stylesheetPath: string;
  stylesheetHref: string;
  activeDataAnimIds: string[];
  activeGeneratedClasses: string[];
  template: PageAssetContractTextPlan;
  stylesheet: PageAssetContractTextPlan;
  pageJsConfig: PageJsConfig;
  pageJsChanged: boolean;
  diagnostics: string[];
};

export type PageAssetContractApplyReceipt = {
  plan: PageAssetContractPlan;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  pageJs: PageJsDraftStageReceipt | null;
  authority: PageContractAuthorityReceipt;
};
