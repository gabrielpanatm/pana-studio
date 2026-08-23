import { t } from "$lib/i18n/runtime.svelte";
import type {
  VersionDiffKind,
  VersionDiffReceipt,
  VersionFileStatus,
  VersionHistoryEntry,
  VersioningMutationIdentity,
  VersioningSessionIdentity,
  VersioningSnapshot,
} from "$lib/versioning/contracts";
import {
  commitVersioning,
  configureVersioningIdentity,
  initializeVersioning,
  previewVersion,
  readVersionDiff,
  readVersionHistory,
  readVersioningSnapshot,
  stageAllVersioning,
  stageVersioningPaths,
  unstageAllVersioning,
  unstageVersioningPaths,
} from "$lib/versioning/io";
import type { VersioningOperationState } from "$lib/versioning/panel-context.svelte";
import { VersioningSessionEpoch } from "$lib/versioning/session-epoch";

export type VersioningSnapshotParticipant = Readonly<{
  reset: () => void;
  beforeRefresh?: (keepDiff: boolean) => void;
  onSnapshot?: (snapshot: VersioningSnapshot) => void;
  refresh?: (serial: number) => Promise<void>;
  refreshAfterRepositoryMutation?: (serial: number) => Promise<void>;
}>;

/** Owns session identity, snapshot/history/diff reads and local Git mutations. */
export class VersioningSnapshotController {
  snapshot = $state<VersioningSnapshot | null>(null);
  history = $state<VersionHistoryEntry[]>([]);
  historyHasMore = $state(false);
  diff = $state<VersionDiffReceipt | null>(null);
  loading = $state(false);
  commitMessage = $state("");
  identityName = $state("");
  identityEmail = $state("");

  private readonly epoch = new VersioningSessionEpoch();
  private readonly participants: VersioningSnapshotParticipant[] = [];
  private hydratedIdentityToken = "";

  constructor(readonly operations: VersioningOperationState) {}

  registerParticipant(participant: VersioningSnapshotParticipant) {
    this.participants.push(participant);
  }

  synchronize() {
    const { changed } = this.epoch.synchronize(
      this.operations.host.projectRoot(),
      this.operations.host.sessionId(),
    );
    if (!changed) return;
    this.reset();
    for (const participant of this.participants) participant.reset();
    if (this.readIdentity()) void this.refresh();
  }

  readIdentity(): VersioningSessionIdentity | null {
    const expectedProjectRoot = this.operations.host.projectRoot();
    const expectedSessionId = this.operations.host.sessionId();
    if (!expectedProjectRoot || !expectedSessionId) return null;
    return { expectedProjectRoot, expectedSessionId };
  }

  mutationIdentity(): VersioningMutationIdentity {
    if (!this.snapshot) throw new Error(t("versions-git-unavailable"));
    const identity = this.readIdentity();
    if (!identity) throw new Error(t("versions-session-unavailable"));
    return {
      ...identity,
      expectedStatusToken: this.snapshot.statusToken,
      expectedHeadOid: this.snapshot.headOid,
    };
  }

  isCurrent(serial: number) {
    return this.epoch.isCurrent(serial);
  }

  currentSerial() {
    return this.epoch.current();
  }

  publishSnapshot(snapshot: VersioningSnapshot) {
    this.snapshot = snapshot;
    this.hydrateIdentity(snapshot);
    for (const participant of this.participants) participant.onSnapshot?.(snapshot);
  }

  clearDiff() {
    this.diff = null;
  }

  async refresh(options: { keepDiff?: boolean } = {}) {
    const identity = this.readIdentity();
    if (!identity) {
      this.reset();
      for (const participant of this.participants) participant.reset();
      return;
    }
    const serial = this.epoch.nextRequest();
    this.loading = true;
    this.operations.error = "";
    try {
      const next = await readVersioningSnapshot(identity);
      if (!this.epoch.isCurrent(serial)) return;
      this.publishSnapshot(next);
      if (!options.keepDiff) this.diff = null;
      for (const participant of this.participants) {
        participant.beforeRefresh?.(Boolean(options.keepDiff));
      }
      await Promise.all([
        this.refreshHistory(true, serial),
        ...this.participants
          .filter((participant) => participant.refresh)
          .map((participant) => participant.refresh!(serial)),
      ]);
    } catch (reason) {
      if (this.epoch.isCurrent(serial)) this.operations.fail(reason);
    } finally {
      if (this.epoch.isCurrent(serial)) this.loading = false;
    }
  }

  async refreshHistory(reset = true, parentSerial = this.epoch.current()) {
    const identity = this.readIdentity();
    if (!identity || this.snapshot?.repositoryState !== "ready" || !this.snapshot.headOid) {
      this.history = [];
      this.historyHasMore = false;
      return;
    }
    const offset = reset ? 0 : this.history.length;
    const page = await readVersionHistory(identity, offset, 30);
    if (!this.epoch.isCurrent(parentSerial)) return;
    this.history = reset ? page.entries : [...this.history, ...page.entries];
    this.historyHasMore = page.hasMore;
  }

  async refreshAfterRepositoryMutation() {
    const serial = this.epoch.current();
    await Promise.all([
      this.refreshHistory(true, serial),
      ...this.participants
        .filter((participant) => participant.refreshAfterRepositoryMutation)
        .map((participant) => participant.refreshAfterRepositoryMutation!(serial)),
    ]);
  }

  async runSnapshotMutation(
    label: string,
    operation: () => Promise<VersioningSnapshot>,
  ) {
    if (!this.operations.requireMutationAllowed()) return;
    this.operations.begin(label);
    try {
      this.publishSnapshot(await operation());
      this.diff = null;
      for (const participant of this.participants) participant.beforeRefresh?.(false);
      if (!(await this.operations.settlePublishedEffect(
        t("versions-backend-effect", { label }),
        () => this.refreshAfterRepositoryMutation(),
      ))) return;
      this.operations.host.onStatusUpdate(label, "saved");
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(`${label}: ${error}`, "error");
    } finally {
      this.operations.finish();
    }
  }

  async initialize() {
    await this.runSnapshotMutation(
      t("versions-repository-initialized"),
      () => initializeVersioning(this.mutationIdentity()),
    );
  }

  async saveIdentity() {
    await this.runSnapshotMutation(
      t("versions-identity-saved"),
      () => configureVersioningIdentity(this.mutationIdentity(), {
        name: this.identityName,
        email: this.identityEmail,
      }),
    );
  }

  async stagePaths(paths: string[]) {
    await this.runFileMutation(
      paths.length === 1
        ? t("versions-file-staged", { path: paths[0] })
        : t("versions-all-staged"),
      () => stageVersioningPaths(this.mutationIdentity(), paths),
    );
  }

  async stageAll() {
    await this.runFileMutation(
      t("versions-all-staged"),
      () => stageAllVersioning(this.mutationIdentity()),
    );
  }

  async unstagePaths(paths: string[]) {
    await this.runFileMutation(
      paths.length === 1
        ? t("versions-removed-from-staged", { path: paths[0] })
        : t("versions-index-cleared"),
      () => unstageVersioningPaths(this.mutationIdentity(), paths),
    );
  }

  async unstageAll() {
    await this.runFileMutation(
      t("versions-index-cleared"),
      () => unstageAllVersioning(this.mutationIdentity()),
    );
  }

  async commit() {
    if (!this.commitMessage.trim()) {
      this.operations.error = t("versions-commit-message-required");
      return;
    }
    if (!this.operations.requireMutationAllowed()) return;
    this.operations.begin("commit");
    try {
      const receipt = await commitVersioning(this.mutationIdentity(), this.commitMessage);
      this.commitMessage = "";
      if (receipt.snapshot) this.publishSnapshot(receipt.snapshot);
      else await this.refresh();
      if (!(await this.operations.settlePublishedEffect(
        t("versions-commit-published-backend"),
        () => this.refreshHistory(true),
      ))) return;
      this.diff = null;
      for (const participant of this.participants) participant.beforeRefresh?.(false);
      const diagnostic = receipt.diagnostic
        ? ` ${t("versions-technical-details-available")}`
        : "";
      this.operations.host.onStatusUpdate(
        t("versions-created-status", {
          oid: receipt.commitOid.slice(0, 8),
          diagnostic,
        }),
        receipt.publicationStatus === "published" ? "saved" : "error",
      );
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(
        t("versions-commit-blocked", { message: error }),
        "error",
      );
    } finally {
      this.operations.finish();
    }
  }

  async showFileDiff(file: VersionFileStatus, kind: VersionDiffKind) {
    const identity = this.readIdentity();
    if (!identity) return;
    this.operations.begin(`diff:${kind}:${file.path}`);
    try {
      this.diff = await readVersionDiff(identity, { kind, path: file.path });
    } catch (reason) {
      this.operations.fail(reason);
    } finally {
      this.operations.finish();
    }
  }

  async showCommitDiff(entry: VersionHistoryEntry) {
    const identity = this.readIdentity();
    if (!identity) return;
    this.operations.begin(`diff:commit:${entry.oid}`);
    try {
      this.diff = await readVersionDiff(identity, { kind: "commit", commitOid: entry.oid });
    } catch (reason) {
      this.operations.fail(reason);
    } finally {
      this.operations.finish();
    }
  }

  async previewCommit(entry: VersionHistoryEntry) {
    const identity = this.readIdentity();
    if (!identity) return;
    this.operations.begin(`preview:${entry.oid}`);
    try {
      if (this.operations.host.activePreviewCommitOid()) {
        await this.operations.host.returnToLivePreview();
      }
      const receipt = await previewVersion(identity, entry.oid);
      await this.operations.host.showPreview(receipt);
      this.operations.host.onStatusUpdate(
        t("versions-preview-status", { oid: receipt.shortOid }),
        "saved",
      );
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(
        t("versions-preview-blocked", { message: error }),
        "error",
      );
    } finally {
      this.operations.finish();
    }
  }

  private async runFileMutation(
    label: string,
    operation: () => Promise<{ snapshot: VersioningSnapshot }>,
  ) {
    if (!this.operations.requireMutationAllowed()) return;
    this.operations.begin(label);
    try {
      const receipt = await operation();
      this.publishSnapshot(receipt.snapshot);
      this.diff = null;
      for (const participant of this.participants) participant.beforeRefresh?.(false);
      this.operations.host.onStatusUpdate(label, "saved");
    } catch (reason) {
      const error = this.operations.fail(reason);
      this.operations.host.onStatusUpdate(`${label}: ${error}`, "error");
    } finally {
      this.operations.finish();
    }
  }

  private hydrateIdentity(next: VersioningSnapshot) {
    if (this.hydratedIdentityToken === next.projectRoot) return;
    this.identityName = next.userName ?? "";
    this.identityEmail = next.userEmail ?? "";
    this.hydratedIdentityToken = next.projectRoot;
  }

  private reset() {
    this.snapshot = null;
    this.history = [];
    this.historyHasMore = false;
    this.diff = null;
    this.loading = false;
    this.commitMessage = "";
    this.identityName = "";
    this.identityEmail = "";
    this.hydratedIdentityToken = "";
    this.operations.error = "";
    this.operations.finish();
  }
}
