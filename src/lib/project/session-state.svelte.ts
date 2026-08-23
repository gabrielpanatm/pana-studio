import type {
  ProjectLifecycleSnapshot,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";

export function initialProjectLifecycle(): ProjectLifecycleSnapshot {
  return {
    schemaVersion: 1,
    revision: 1,
    activeSession: null,
    transition: "idle",
    operationId: null,
    transitionStartedAtMs: null,
    reason: "frontend_initialized",
  };
}

/** Owns the Rust-authoritative project identity and workspace projection. */
export class ProjectSessionState {
  root = $state("");
  runtimeSessionId = $state("");
  epoch = $state(0);
  workspace = $state<ProjectWorkspaceSnapshot | null>(null);
  project = $state<ProjectScan | null>(null);
  lifecycle = $state<ProjectLifecycleSnapshot>(initialProjectLifecycle());
  status = $state("");
  workspaceMutationEpoch = $state(0);
  editorMutationEpoch = $state(0);
  saveRequest = $state(0);
  refreshToken = $state(0);
  jsRefreshToken = $state(0);
  reattachPromise: Promise<boolean> | null = null;

  invalidateLeases() {
    this.epoch += 1;
  }

  resetIdentity() {
    this.root = "";
    this.runtimeSessionId = "";
    this.invalidateLeases();
    this.workspace = null;
    this.project = null;
    this.status = "";
  }
}
