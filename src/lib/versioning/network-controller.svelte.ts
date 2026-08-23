import { listen } from "@tauri-apps/api/event";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  VersionNetworkProgressEvent,
  VersioningSnapshot,
} from "$lib/versioning/contracts";
import {
  cancelVersionNetworkOperation,
  configureVersionRemote,
  fetchVersionRemote,
  pushVersionBranch,
  removeVersionRemote,
} from "$lib/versioning/io";
import type { VersioningOperationState } from "$lib/versioning/panel-context.svelte";
import { VersionNetworkProgressLifetime } from "$lib/versioning/network-progress-lifetime";
import type { VersioningSnapshotController } from "$lib/versioning/snapshot-controller.svelte";

const VERSIONING_NETWORK_PROGRESS_EVENT = "pana-versioning-network-progress";

export type VersioningNetworkHooks = Readonly<{
  clearIntegration: () => void;
  selectionChanged: (remote: string, remoteBranch: string) => void;
}>;

/** Owns remote configuration, fetch/push and the network progress lifecycle. */
export class VersioningNetworkController {
  remoteName = $state("origin");
  remoteFetchUrl = $state("");
  remotePushUrl = $state("");
  selectedRemote = $state("");
  selectedRemoteBranch = $state("");
  pendingRemoteRemoval = $state("");
  remoteRemovalConfirmation = $state("");
  activeNetwork = $state<VersionNetworkProgressEvent | null>(null);

  private unlisten: () => void = () => {};
  private listenerGeneration = 0;
  private readonly progressLifetime = new VersionNetworkProgressLifetime<VersionNetworkProgressEvent>({
    schedule: (callback, delayMs) => globalThis.setTimeout(callback, delayMs),
    cancel: (handle) => globalThis.clearTimeout(
      handle as ReturnType<typeof globalThis.setTimeout>,
    ),
  });

  constructor(
    readonly snapshot: VersioningSnapshotController,
    readonly operations: VersioningOperationState,
    private readonly hooks: VersioningNetworkHooks,
  ) {}

  start() {
    const generation = ++this.listenerGeneration;
    void listen<VersionNetworkProgressEvent>(
      VERSIONING_NETWORK_PROGRESS_EVENT,
      (event) => this.receiveProgress(event.payload),
    ).then((cleanup) => {
      if (generation !== this.listenerGeneration) cleanup();
      else this.unlisten = cleanup;
    });
    return () => this.dispose();
  }

  dispose() {
    this.listenerGeneration += 1;
    this.unlisten();
    this.unlisten = () => {};
    this.progressLifetime.clear((value) => { this.activeNetwork = value; });
  }

  reset() {
    this.remoteName = "origin";
    this.remoteFetchUrl = "";
    this.remotePushUrl = "";
    this.selectedRemote = "";
    this.selectedRemoteBranch = "";
    this.pendingRemoteRemoval = "";
    this.remoteRemovalConfirmation = "";
    this.progressLifetime.clear((value) => { this.activeNetwork = value; });
  }

  onSnapshot(next: VersioningSnapshot) {
    const remote = next.remotes.find(
      (item) => item.name === this.selectedRemote && item.usable,
    ) ?? next.remotes.find(
      (item) => item.name === next.upstream?.remote && item.usable,
    ) ?? next.remotes.find((item) => item.usable);
    this.selectedRemote = remote?.name ?? "";
    const remoteBranch = next.remoteBranches.find(
      (branch) => branch.remote === this.selectedRemote
        && branch.name === this.selectedRemoteBranch,
    ) ?? next.remoteBranches.find(
      (branch) => branch.remote === this.selectedRemote
        && branch.name === next.upstream?.remoteBranch,
    ) ?? next.remoteBranches.find((branch) => branch.remote === this.selectedRemote);
    this.selectedRemoteBranch = remoteBranch?.name ?? "";
    this.hooks.selectionChanged(this.selectedRemote, this.selectedRemoteBranch);
  }

  editRemote(name: string) {
    const remote = this.snapshot.snapshot?.remotes.find((item) => item.name === name);
    if (!remote) return;
    this.remoteName = remote.name;
    this.remoteFetchUrl = remote.usable ? remote.fetchUrl : "";
    this.remotePushUrl = remote.usable && remote.pushUrl !== remote.fetchUrl
      ? remote.pushUrl
      : "";
    this.pendingRemoteRemoval = "";
    this.remoteRemovalConfirmation = "";
  }

  async saveRemote() {
    if (!this.remoteName.trim() || !this.remoteFetchUrl.trim()) {
      this.operations.error = t("versions-remote-required");
      return;
    }
    await this.snapshot.runSnapshotMutation(
      t("versions-remote-saved"),
      () => configureVersionRemote(this.snapshot.mutationIdentity(), {
        name: this.remoteName.trim(),
        fetchUrl: this.remoteFetchUrl.trim(),
        pushUrl: this.remotePushUrl.trim() || null,
      }),
    );
  }

  requestRemoteRemoval(name: string) {
    this.pendingRemoteRemoval = name;
    this.remoteRemovalConfirmation = "";
  }

  cancelRemoteRemoval() {
    this.pendingRemoteRemoval = "";
    this.remoteRemovalConfirmation = "";
  }

  selectRemote() {
    this.selectedRemoteBranch = this.snapshot.snapshot?.remoteBranches.find(
      (branch) => branch.remote === this.selectedRemote,
    )?.name ?? "";
    this.hooks.clearIntegration();
    this.hooks.selectionChanged(this.selectedRemote, this.selectedRemoteBranch);
  }

  selectRemoteBranch() {
    this.hooks.clearIntegration();
    this.hooks.selectionChanged(this.selectedRemote, this.selectedRemoteBranch);
  }

  async removeRemoteConfirmed() {
    if (
      !this.pendingRemoteRemoval
      || this.remoteRemovalConfirmation !== this.pendingRemoteRemoval
    ) {
      this.operations.error = t("versions-remote-confirmation-mismatch");
      return;
    }
    const name = this.pendingRemoteRemoval;
    await this.snapshot.runSnapshotMutation(
      t("versions-remote-removed", { name }),
      () => removeVersionRemote(this.snapshot.mutationIdentity(), name),
    );
    this.pendingRemoteRemoval = "";
    this.remoteRemovalConfirmation = "";
  }

  async fetchRemote() {
    if (!this.selectedRemote) {
      this.operations.error = t("versions-choose-remote");
      return;
    }
    if (!this.operations.requireMutationAllowed()) return;
    const operationId = this.networkOperationId("fetch");
    this.activeNetwork = this.startedEvent(operationId, "fetch");
    this.operations.begin(`fetch:${this.selectedRemote}`);
    try {
      const receipt = await fetchVersionRemote(this.snapshot.mutationIdentity(), {
        operationId,
        remote: this.selectedRemote,
        prune: true,
      });
      this.snapshot.publishSnapshot(receipt.snapshot);
      this.hooks.clearIntegration();
      if (!(await this.operations.settlePublishedEffect(
        t("versions-fetch-backend", { remote: this.selectedRemote }),
        () => this.snapshot.refreshAfterRepositoryMutation(),
      ))) return;
      this.operations.host.onStatusUpdate(
        receipt.changed
          ? t("versions-fetch-updated", { remote: this.selectedRemote })
          : t("versions-fetch-no-updates", { remote: this.selectedRemote }),
        "saved",
      );
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(
        t("versions-fetch-blocked", { message: error }),
        "error",
      );
    } finally {
      this.operations.finish();
    }
  }

  async pushBranch() {
    const snapshot = this.snapshot.snapshot;
    if (!snapshot?.branch || !this.selectedRemote) {
      this.operations.error = t("versions-push-required");
      return;
    }
    if (!this.operations.requireMutationAllowed()) return;
    const branch = snapshot.branch;
    const remoteBranch = this.selectedRemoteBranch || branch;
    const operationId = this.networkOperationId("push");
    this.activeNetwork = this.startedEvent(operationId, "push");
    this.operations.begin(`push:${this.selectedRemote}/${remoteBranch}`);
    try {
      const receipt = await pushVersionBranch(this.snapshot.mutationIdentity(), {
        operationId,
        remote: this.selectedRemote,
        remoteBranch,
        setUpstream: !snapshot.upstream
          || snapshot.upstream.remote !== this.selectedRemote
          || snapshot.upstream.remoteBranch !== remoteBranch,
      });
      this.snapshot.publishSnapshot(receipt.snapshot);
      this.hooks.clearIntegration();
      this.operations.host.onStatusUpdate(
        t("versions-push-published", {
          branch,
          remote: this.selectedRemote,
          remoteBranch,
        }),
        "saved",
      );
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(
        t("versions-push-blocked", { message: error }),
        "error",
      );
    } finally {
      this.operations.finish();
    }
  }

  async cancelNetwork() {
    const identity = this.snapshot.readIdentity();
    if (!identity || !this.activeNetwork) return;
    try {
      const receipt = await cancelVersionNetworkOperation(
        identity,
        this.activeNetwork.operationId,
      );
      if (!receipt.cancellationRequested) this.activeNetwork = null;
    } catch (reason) {
      this.operations.fail(reason);
    }
  }

  private receiveProgress(payload: VersionNetworkProgressEvent) {
    if (
      payload.projectRoot !== this.operations.host.projectRoot()
      || payload.sessionId !== this.operations.host.sessionId()
    ) return;
    this.progressLifetime.receive(payload, (value) => { this.activeNetwork = value; });
  }

  private startedEvent(
    operationId: string,
    kind: VersionNetworkProgressEvent["kind"],
  ): VersionNetworkProgressEvent {
    return {
      schemaVersion: 2,
      projectRoot: this.operations.host.projectRoot(),
      sessionId: this.operations.host.sessionId(),
      operationId,
      kind,
      status: "started",
      messageDiagnostic: {
        schemaVersion: 1,
        code: kind === "fetch" ? "versions-fetch-started" : "versions-push-started",
        arguments: {},
      },
    };
  }

  private networkOperationId(kind: "fetch" | "push") {
    const random = globalThis.crypto?.randomUUID?.().replaceAll("-", "")
      ?? Math.random().toString(16).slice(2);
    return `${kind}-${Date.now()}-${random}`;
  }
}
