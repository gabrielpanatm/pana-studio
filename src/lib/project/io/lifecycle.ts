import type { ProjectLifecycleSnapshot } from "$lib/project/lifecycle-contract";
import type {
  ProjectOpenBootstrapReceipt,
  ProjectOpenInspectionReceipt,
  ProjectOpenRecoveryDecisionInput,
} from "$lib/project/lifecycle-contract";
import { invoke } from "@tauri-apps/api/core";
import { validateProjectOpenBootstrapReceipt } from "$lib/project/io/configuration";

export async function openProject(
  path: string,
  operationId: string,
  candidateToken: string,
  operatorDecisionId?: string,
  recoveryDecision?: ProjectOpenRecoveryDecisionInput,
): Promise<ProjectOpenBootstrapReceipt> {
  const receipt = await invoke<ProjectOpenBootstrapReceipt>("open_project", {
    path,
    operationId,
    candidateToken,
    operatorDecisionId,
    recoveryDecision,
  });
  validateProjectOpenBootstrapReceipt(receipt);
  return receipt;
}

export function inspectProjectOpen(
  path: string,
  expectedSnapshotToken: string,
): Promise<ProjectOpenInspectionReceipt> {
  return invoke<ProjectOpenInspectionReceipt>("inspect_project_open", {
    path,
    expectedSnapshotToken,
  });
}

export function readProjectLifecycle(): Promise<ProjectLifecycleSnapshot> {
  return invoke<ProjectLifecycleSnapshot>("read_project_lifecycle");
}

export function acknowledgeProjectFrontendHydrated(
  projectRoot: string,
  runtimeSessionId: string,
): Promise<ProjectLifecycleSnapshot> {
  return invoke<ProjectLifecycleSnapshot>("acknowledge_project_frontend_hydrated", {
    projectRoot,
    runtimeSessionId,
  });
}

export function reportProjectCapabilityDegraded(
  projectRoot: string,
  runtimeSessionId: string,
  capability: "frontend" | "preview" | "canvas" | "source_graph",
  diagnostic: string,
): Promise<ProjectLifecycleSnapshot> {
  return invoke<ProjectLifecycleSnapshot>("report_project_capability_degraded", {
    projectRoot,
    runtimeSessionId,
    capability,
    diagnostic,
  });
}

export function cancelProjectOpen(
  operationId: string,
  diagnostic: string,
): Promise<ProjectLifecycleSnapshot> {
  return invoke<ProjectLifecycleSnapshot>("cancel_project_open", {
    operationId,
    diagnostic,
  });
}

export function closeProject(operatorDecisionId?: string): Promise<void> {
  return invoke<void>("close_project", { operatorDecisionId });
}

export async function reattachProjectSession(): Promise<ProjectOpenBootstrapReceipt | null> {
  const receipt = await invoke<ProjectOpenBootstrapReceipt | null>("reattach_project_session");
  if (receipt) validateProjectOpenBootstrapReceipt(receipt);
  return receipt;
}
