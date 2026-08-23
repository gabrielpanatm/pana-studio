import type {
  StartupCreationCatalog,
  StartupCreationPlan,
  StartupFlowSnapshot,
} from "$lib/project/lifecycle-contract";
import type { ProjectOpenRecoveryDecisionRequest } from "$lib/project/open-recovery";
import type { ProjectTransitionDecisionRequest } from "$lib/project/transition-decision";
import {
  readStartupFlow,
} from "$lib/project/io/startup";
import type { OpenProjectRootOptions } from "$lib/project/controller-contracts";
import type { GlobalStatusEscalationRequest } from "$lib/status/global-status";
import {
  applyStartupProject,
  cancelStartupCreationPlan,
  openProjectFolder,
  planStartupProject,
  retryStartupProjectOpen,
  selectStartupCreationOption,
  type ProjectStartupHost,
} from "$lib/state/project-startup-controller";

export type ProjectStartupStateDependencies = {
  openProjectRoot: (root: string, options?: OpenProjectRootOptions) => Promise<void>;
  escalateStatus: (notification: GlobalStatusEscalationRequest) => void;
  clearStatus: (id: string) => void;
};

export function initialStartupFlow(): StartupFlowSnapshot {
  return {
    schemaVersion: 1,
    revision: 1,
    stage: "idle",
    candidate: null,
    diagnostics: [],
  };
}

/** Owns startup discovery, creation planning and explicit operator decisions. */
export class ProjectStartupState {
  flow = $state<StartupFlowSnapshot>(initialStartupFlow());
  creationCatalog = $state<StartupCreationCatalog | null>(null);
  creationPlan = $state<StartupCreationPlan | null>(null);
  selectedOptionId = $state<string | null>(null);
  pending = $state(false);
  error = $state("");
  openRecoveryDecision = $state<ProjectOpenRecoveryDecisionRequest | null>(null);
  transitionDecision = $state<ProjectTransitionDecisionRequest | null>(null);

  private readonly dependencies: ProjectStartupStateDependencies;

  constructor(dependencies: ProjectStartupStateDependencies) {
    this.dependencies = dependencies;
  }

  escalateGlobalStatus(notification: GlobalStatusEscalationRequest) {
    this.dependencies.escalateStatus(notification);
  }

  clearNotification(id: string) {
    this.dependencies.clearStatus(id);
  }

  async refreshFlow() {
    this.flow = await readStartupFlow();
    return this.flow;
  }

  async openFolder() {
    await openProjectFolder(this.controllerHost(), this.dependencies.openProjectRoot);
  }

  async retryOpen() {
    await retryStartupProjectOpen(this.controllerHost(), this.dependencies.openProjectRoot);
  }

  selectCreationOption(optionId: string) {
    selectStartupCreationOption(this.controllerHost(), optionId);
  }

  async planProject() {
    await planStartupProject(this.controllerHost());
  }

  cancelCreationPlan() {
    cancelStartupCreationPlan(this.controllerHost());
  }

  async applyProject() {
    await applyStartupProject(this.controllerHost(), this.dependencies.openProjectRoot);
  }

  private controllerHost(): ProjectStartupHost {
    const owner = this;
    return {
      get startupFlow() { return owner.flow; },
      set startupFlow(flow) { owner.flow = flow; },
      get startupCreationCatalog() { return owner.creationCatalog; },
      set startupCreationCatalog(catalog) { owner.creationCatalog = catalog; },
      get startupCreationPlan() { return owner.creationPlan; },
      set startupCreationPlan(plan) { owner.creationPlan = plan; },
      get startupSelectedOptionId() { return owner.selectedOptionId; },
      set startupSelectedOptionId(optionId) { owner.selectedOptionId = optionId; },
      get startupPending() { return owner.pending; },
      set startupPending(pending) { owner.pending = pending; },
      get startupError() { return owner.error; },
      set startupError(error) { owner.error = error; },
      escalateGlobalStatus: (notification) => owner.escalateGlobalStatus(notification),
      clearNotification: (id) => owner.clearNotification(id),
    };
  }

  reset() {
    this.flow = initialStartupFlow();
    this.clearCreation();
    this.openRecoveryDecision = null;
    this.transitionDecision = null;
  }

  clearCreation() {
    this.creationCatalog = null;
    this.creationPlan = null;
    this.selectedOptionId = null;
    this.pending = false;
    this.error = "";
  }
}
