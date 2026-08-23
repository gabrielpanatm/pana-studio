import type { ProjectDiskChangeNotice } from "$lib/kernel/project-disk-events";
import type { ProjectDiskManifest } from "$lib/project/external-disk-contract";
import type { ProjectDiskWatchStopIdentity } from "$lib/project/io/external-disk";
import type {
  ExternalDiskContext,
  ExternalDiskEnvironment,
  ExternalDiskRuntime,
} from "$lib/session/external-disk/contracts";
import {
  resumeExternalDiskMonitoringAfterSave,
  resumeExternalDiskMonitoringAfterTransition,
  startExternalDiskMonitoring,
  stopExternalDiskMonitoring,
  suspendAndDrainExternalDiskMonitoring,
} from "$lib/session/external-disk/monitor";
import {
  acceptExternalDiskSaveBaseline,
  createExternalDiskSnapshot,
  establishExternalDiskBaseline,
  invalidateExternalDiskForTransition,
  markExternalDiskProjectionRecovery,
  resetExternalDiskSnapshot,
  rollbackExternalDiskTransition,
} from "$lib/session/external-disk/state";

/** Owns disk-watch, audit and reconcile lifetime for one frontend runtime. */
export class ExternalDiskState implements ExternalDiskRuntime {
  snapshot = $state(createExternalDiskSnapshot());
  auditTimer: number | null = null;
  watchUnlisten: (() => void) | null = null;
  watchGeneration: number | null = null;
  watchStopIdentity: ProjectDiskWatchStopIdentity | null = null;
  watchRevision = 0;
  watchSubscriptionGeneration = 0;
  pendingWatchNotice: ProjectDiskChangeNotice | null = null;
  watchEventPending = false;
  watchEventDrainInFlight = false;
  suspended = $state(false);
  checkInFlight: ExternalDiskRuntime["checkInFlight"] = null;
  checkGeneration = 0;
  reconcileGeneration = 0;

  constructor(
    private readonly resolveEnvironment: () => ExternalDiskEnvironment,
  ) {}

  start() {
    startExternalDiskMonitoring(this.context());
  }

  stop() {
    stopExternalDiskMonitoring(this.context());
  }

  async suspendAndDrain() {
    await suspendAndDrainExternalDiskMonitoring(this.context());
  }

  resumeAfterSave() {
    resumeExternalDiskMonitoringAfterSave(this.context());
  }

  resumeAfterTransition() {
    resumeExternalDiskMonitoringAfterTransition(this.context());
  }

  reset() {
    stopExternalDiskMonitoring(this.context());
    resetExternalDiskSnapshot(this.context());
  }

  invalidateForProjectTransition() {
    stopExternalDiskMonitoring(this.context());
    invalidateExternalDiskForTransition(this.context());
  }

  markProjectionRecoveryRequired(message: string) {
    stopExternalDiskMonitoring(this.context());
    markExternalDiskProjectionRecovery(this.context(), message);
  }

  rollbackFailedProjectTransition() {
    rollbackExternalDiskTransition(this.context());
  }

  async establishBaseline() {
    establishExternalDiskBaseline(this.context());
  }

  acceptSaveBaseline(manifest: ProjectDiskManifest, diskGeneration: number) {
    stopExternalDiskMonitoring(this.context());
    acceptExternalDiskSaveBaseline(this.context(), manifest, diskGeneration);
  }

  private context(): ExternalDiskContext {
    return {
      runtime: this,
      environment: this.resolveEnvironment(),
    };
  }
}
