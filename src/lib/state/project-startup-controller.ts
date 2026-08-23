import { tick } from "svelte";
import {
  applyStartupCreation,
  chooseProjectFolder,
  inspectStartupFolder,
  planStartupCreation,
  readStartupCreationCatalog,
} from "$lib/project/io/startup";
import type { OpenProjectRootOptions } from "$lib/project/controller-contracts";
import type {
  StartupCreationCatalog,
  StartupCreationPlan,
  StartupFlowSnapshot,
} from "$lib/project/lifecycle-contract";
import type { GlobalStatusEscalationRequest } from "$lib/status/global-status";
import { errorMessage } from "$lib/util";

export type ProjectStartupHost = {
  startupFlow: StartupFlowSnapshot;
  startupCreationCatalog: StartupCreationCatalog | null;
  startupCreationPlan: StartupCreationPlan | null;
  startupSelectedOptionId: string | null;
  startupPending: boolean;
  startupError: string;
  escalateGlobalStatus: (notification: GlobalStatusEscalationRequest) => void;
  clearNotification: (id: string) => void;
};

export type ProjectStartupOpen = (
  root: string,
  options?: OpenProjectRootOptions,
) => Promise<void>;

export type ProjectStartupDependencies = {
  chooseFolder: typeof chooseProjectFolder;
  inspectFolder: typeof inspectStartupFolder;
  readCreationCatalog: typeof readStartupCreationCatalog;
  planCreation: typeof planStartupCreation;
  applyCreation: typeof applyStartupCreation;
  nextRender: () => Promise<void>;
};

const projectStartupDependencies: ProjectStartupDependencies = {
  chooseFolder: chooseProjectFolder,
  inspectFolder: inspectStartupFolder,
  readCreationCatalog: readStartupCreationCatalog,
  planCreation: planStartupCreation,
  applyCreation: applyStartupCreation,
  nextRender: tick,
};

export async function openProjectFolder(
  host: ProjectStartupHost,
  openProjectRoot: ProjectStartupOpen,
  dependencies: ProjectStartupDependencies = projectStartupDependencies,
) {
  host.startupError = "";
  host.startupCreationPlan = null;
  host.startupCreationCatalog = null;
  host.startupSelectedOptionId = null;
  await dependencies.nextRender();
  try {
    const selected = await dependencies.chooseFolder();
    if (!selected || Array.isArray(selected)) return;
    host.startupPending = true;
    await dependencies.nextRender();
    const startup = await dependencies.inspectFolder(selected);
    host.startupFlow = startup;
    const candidate = startup.candidate;
    if (!candidate) return;
    if (candidate.kind === "valid_project") {
      await openProjectRoot(candidate.root, { startupCandidate: candidate });
      return;
    }
    if (candidate.kind === "empty_directory") {
      host.startupCreationCatalog = await dependencies.readCreationCatalog(
        candidate.snapshotToken,
      );
    }
  } catch (error) {
    const message = errorMessage(error);
    host.startupError = message;
    host.escalateGlobalStatus({
      id: "startup.folder.error",
      level: "error",
      title: "Dosarul nu a putut fi inspectat",
      message,
    });
  } finally {
    host.startupPending = false;
  }
}

export async function retryStartupProjectOpen(
  host: ProjectStartupHost,
  openProjectRoot: ProjectStartupOpen,
  dependencies: Pick<ProjectStartupDependencies, "nextRender"> = projectStartupDependencies,
) {
  const candidate = host.startupFlow.candidate;
  if (candidate?.kind !== "valid_project") {
    host.startupError = "Proiectul valid selectat nu mai este disponibil pentru redeschidere.";
    return;
  }
  host.startupPending = true;
  host.startupError = "";
  await dependencies.nextRender();
  try {
    await openProjectRoot(candidate.root, { startupCandidate: candidate });
    host.clearNotification("startup.folder.error");
  } catch (error) {
    const message = errorMessage(error);
    host.startupError = message;
    host.escalateGlobalStatus({
      id: "startup.folder.error",
      level: "error",
      title: "Proiectul nu a putut fi deschis",
      message,
    });
  } finally {
    host.startupPending = false;
  }
}

export function selectStartupCreationOption(
  host: ProjectStartupHost,
  optionId: string,
) {
  if (!host.startupCreationCatalog?.options.some((option) => option.id === optionId)) return;
  host.startupSelectedOptionId = optionId;
  host.startupCreationPlan = null;
  host.startupError = "";
}

export async function planStartupProject(
  host: ProjectStartupHost,
  dependencies: Pick<ProjectStartupDependencies, "planCreation"> = projectStartupDependencies,
) {
  const candidate = host.startupFlow.candidate;
  const optionId = host.startupSelectedOptionId;
  if (candidate?.kind !== "empty_directory" || !optionId) return;
  host.startupPending = true;
  host.startupError = "";
  try {
    host.startupCreationPlan = await dependencies.planCreation({
      expectedSnapshotToken: candidate.snapshotToken,
      optionId,
    });
  } catch (error) {
    host.startupError = errorMessage(error);
  } finally {
    host.startupPending = false;
  }
}

export function cancelStartupCreationPlan(host: ProjectStartupHost) {
  host.startupCreationPlan = null;
  host.startupError = "";
}

export async function applyStartupProject(
  host: ProjectStartupHost,
  openProjectRoot: ProjectStartupOpen,
  dependencies: Pick<ProjectStartupDependencies, "applyCreation"> = projectStartupDependencies,
) {
  const plan = host.startupCreationPlan;
  if (!plan) return;
  host.startupPending = true;
  host.startupError = "";
  try {
    const receipt = await dependencies.applyCreation({
      expectedSnapshotToken: plan.expectedSnapshotToken,
      expectedPlanToken: plan.planToken,
    });
    host.startupFlow = receipt.startup;
    host.startupCreationPlan = null;
    host.startupCreationCatalog = null;
    host.startupSelectedOptionId = null;
    await openProjectRoot(receipt.projectRoot, {
      startupCandidate: receipt.startup.candidate,
    });
  } catch (error) {
    const message = errorMessage(error);
    host.startupError = message;
    host.escalateGlobalStatus({
      id: "startup.creation.error",
      level: "error",
      title: "Proiectul nu a putut fi creat",
      message,
    });
  } finally {
    host.startupPending = false;
  }
}
