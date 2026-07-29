<script lang="ts">
  import {
    IconAlertTriangle,
    IconCheck,
    IconChevronDown,
    IconGitBranch,
    IconGitCommit,
    IconEye,
    IconMinus,
    IconPlus,
    IconRefresh,
    IconRestore,
    IconSettings,
    IconX,
  } from "@tabler/icons-svelte";
  import {
    cancelVersionNetworkOperation,
    clearVersionUpstream,
    commitVersioning,
    configureVersionRemote,
    configureVersionUpstream,
    configureVersioningIdentity,
    createVersionBranch,
    deleteVersionBranch,
    fetchVersionRemote,
    initializeVersioning,
    integrateVersionTarget,
    previewVersion,
    readVersionDiff,
    readVersionHistory,
    readVersionIntegrationPlan,
    readVersionIntegrationRecovery,
    readVersionRestoreRecovery,
    readVersionSyncComparison,
    readVersioningSnapshot,
    removeVersionRemote,
    resolveVersionIntegrationRecovery,
    resolveVersionRestoreRecovery,
    restoreVersioning,
    pushVersionBranch,
    stageAllVersioning,
    stageVersioningPaths,
    switchVersionBranch,
    unstageAllVersioning,
    unstageVersioningPaths,
  } from "$lib/project/io";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import { UI_TERM_IDS } from "$lib/i18n/ui-terms";
  import type { GlobalStatusKind } from "$lib/status/global-status";
  import type {
    ProjectWorkspaceSnapshot,
    VersionDiffKind,
    VersionDiffReceipt,
    VersionFileStatus,
    VersionHistoryEntry,
    VersionIntegrationMode,
    VersionIntegrationPlan,
    VersionIntegrationReceipt,
    VersionIntegrationRecoveryAction,
    VersionIntegrationRecoveryItem,
    VersionIntegrationRecoveryResolutionReceipt,
    VersionIntegrationRecoveryScan,
    VersionNetworkProgressEvent,
    VersionSyncComparison,
    VersioningMutationIdentity,
    VersioningSessionIdentity,
    VersioningSnapshot,
    VersionPreviewReceipt,
    VersionRestoreReceipt,
    VersionRestoreRecoveryAction,
    VersionRestoreRecoveryItem,
    VersionRestoreRecoveryResolutionReceipt,
    VersionRestoreRecoveryScan,
  } from "$lib/types";

  let {
    projectRoot = "",
    sessionId = "",
    workspace = null,
    activePreviewCommitOid = null,
    onStatusUpdate,
    showPreview,
    returnToLivePreview,
    afterRestore,
    afterRecovery,
    afterIntegration,
    afterIntegrationRecovery,
  }: {
    projectRoot?: string;
    sessionId?: string;
    workspace?: ProjectWorkspaceSnapshot | null;
    activePreviewCommitOid?: string | null;
    onStatusUpdate: (text: string, kind: GlobalStatusKind) => void;
    showPreview: (receipt: VersionPreviewReceipt) => void | Promise<void>;
    returnToLivePreview: () => void | Promise<void>;
    afterRestore: (receipt: VersionRestoreReceipt) => void | Promise<void>;
    afterRecovery: (receipt: VersionRestoreRecoveryResolutionReceipt) => void | Promise<void>;
    afterIntegration: (receipt: VersionIntegrationReceipt) => void | Promise<void>;
    afterIntegrationRecovery: (receipt: VersionIntegrationRecoveryResolutionReceipt) => void | Promise<void>;
  } = $props();

  let snapshot = $state<VersioningSnapshot | null>(null);
  let history = $state<VersionHistoryEntry[]>([]);
  let historyHasMore = $state(false);
  let diff = $state<VersionDiffReceipt | null>(null);
  let loading = $state(false);
  let busyAction = $state("");
  let error = $state("");
  let commitMessage = $state("");
  let identityName = $state("");
  let identityEmail = $state("");
  let restoreEntry = $state<VersionHistoryEntry | null>(null);
  let restoreMessage = $state("");
  let restoreConfirmation = $state("");
  let recovery = $state<VersionRestoreRecoveryScan | null>(null);
  let integrationRecovery = $state<VersionIntegrationRecoveryScan | null>(null);
  let syncComparison = $state<VersionSyncComparison | null>(null);
  let integrationPlan = $state<VersionIntegrationPlan | null>(null);
  let integrationDiff = $state<VersionDiffReceipt | null>(null);
  let integrationMessage = $state("");
  let remoteName = $state("origin");
  let remoteFetchUrl = $state("");
  let remotePushUrl = $state("");
  let selectedRemote = $state("");
  let selectedRemoteBranch = $state("");
  let newBranchName = $state("");
  let pendingBranchRemoval = $state("");
  let branchRemovalConfirmation = $state("");
  let pendingRemoteRemoval = $state("");
  let remoteRemovalConfirmation = $state("");
  let activeNetwork = $state<VersionNetworkProgressEvent | null>(null);
  let hydratedIdentityToken = "";
  let requestSerial = 0;

  const stagedFiles = $derived(snapshot?.files.filter((file) => file.staged) ?? []);
  const unstagedFiles = $derived(snapshot?.files.filter((file) => file.unstaged) ?? []);
  const workspaceDirty = $derived(workspace?.dirty ?? false);
  const mutationBlockedReason = $derived(
    workspaceDirty
      ? t("versions-blocked-editor-dirty")
      : recovery?.items.length
        ? t("versions-blocked-restore")
        : integrationRecovery?.items.length
          ? t("versions-blocked-integration")
        : "",
  );
  const usableRemotes = $derived(snapshot?.remotes.filter((remote) => remote.usable) ?? []);
  const selectedRemoteBranches = $derived(
    snapshot?.remoteBranches.filter((branch) => branch.remote === selectedRemote) ?? [],
  );

  function readIdentity(): VersioningSessionIdentity | null {
    if (!projectRoot || !sessionId) return null;
    return {
      expectedProjectRoot: projectRoot,
      expectedSessionId: sessionId,
    };
  }

  function mutationIdentity(): VersioningMutationIdentity {
    if (!snapshot) throw new Error(t("versions-git-unavailable"));
    const identity = readIdentity();
    if (!identity) throw new Error(t("versions-session-unavailable"));
    return {
      ...identity,
      expectedStatusToken: snapshot.statusToken,
      expectedHeadOid: snapshot.headOid,
    };
  }

  function errorMessage(value: unknown) {
    return value instanceof Error ? value.message : String(value);
  }

  async function settlePublishedEffect(
    label: string,
    projection: () => void | Promise<void>,
  ) {
    try {
      await projection();
      return true;
    } catch (reason) {
      error = t("versions-projection-failed", {
        label,
        message: errorMessage(reason),
      });
      onStatusUpdate(error, "error");
      return false;
    }
  }

  function hydrateIdentity(next: VersioningSnapshot) {
    if (hydratedIdentityToken === next.projectRoot) return;
    identityName = next.userName ?? "";
    identityEmail = next.userEmail ?? "";
    hydratedIdentityToken = next.projectRoot;
  }

  function hydrateRemoteSelection(next: VersioningSnapshot) {
    const remote = next.remotes.find((item) => item.name === selectedRemote && item.usable)
      ?? next.remotes.find((item) => item.name === next.upstream?.remote && item.usable)
      ?? next.remotes.find((item) => item.usable);
    selectedRemote = remote?.name ?? "";
    const remoteBranch = next.remoteBranches.find(
      (branch) => branch.remote === selectedRemote && branch.name === selectedRemoteBranch,
    ) ?? next.remoteBranches.find(
      (branch) => branch.remote === selectedRemote && branch.name === next.upstream?.remoteBranch,
    ) ?? next.remoteBranches.find((branch) => branch.remote === selectedRemote);
    selectedRemoteBranch = remoteBranch?.name ?? "";
    if (!integrationMessage.trim() || integrationMessage === t("versions-default-integration-message")) {
      integrationMessage = selectedRemoteBranch
        ? t("versions-integration-message", {
          remote: selectedRemote,
          branch: selectedRemoteBranch,
        })
        : t("versions-default-integration-message");
    }
  }

  async function refresh(options: { keepDiff?: boolean } = {}) {
    const identity = readIdentity();
    if (!identity) {
      snapshot = null;
      history = [];
      diff = null;
      integrationPlan = null;
      integrationDiff = null;
      return;
    }
    const serial = ++requestSerial;
    loading = true;
    error = "";
    try {
      const next = await readVersioningSnapshot(identity);
      if (serial !== requestSerial) return;
      snapshot = next;
      hydrateIdentity(next);
      hydrateRemoteSelection(next);
      if (!options.keepDiff) {
        diff = null;
        integrationPlan = null;
        integrationDiff = null;
      }
      await Promise.all([
        refreshHistory(true, serial),
        refreshRecovery(serial),
        refreshIntegrationRecovery(serial),
        refreshSyncComparison(serial),
      ]);
    } catch (reason) {
      if (serial !== requestSerial) return;
      error = errorMessage(reason);
    } finally {
      if (serial === requestSerial) loading = false;
    }
  }

  async function refreshHistory(reset = true, parentSerial = requestSerial) {
    const identity = readIdentity();
    if (!identity || snapshot?.repositoryState !== "ready" || !snapshot.headOid) {
      history = [];
      historyHasMore = false;
      return;
    }
    const offset = reset ? 0 : history.length;
    const page = await readVersionHistory(identity, offset, 30);
    if (parentSerial !== requestSerial) return;
    history = reset ? page.entries : [...history, ...page.entries];
    historyHasMore = page.hasMore;
  }

  async function refreshRecovery(parentSerial = requestSerial) {
    const identity = readIdentity();
    if (!identity || snapshot?.repositoryState !== "ready") {
      recovery = null;
      return;
    }
    const next = await readVersionRestoreRecovery(identity);
    if (parentSerial === requestSerial) recovery = next;
  }

  async function refreshIntegrationRecovery(parentSerial = requestSerial) {
    const identity = readIdentity();
    if (!identity || snapshot?.repositoryState !== "ready") {
      integrationRecovery = null;
      return;
    }
    const next = await readVersionIntegrationRecovery(identity);
    if (parentSerial === requestSerial) integrationRecovery = next;
  }

  async function refreshSyncComparison(parentSerial = requestSerial) {
    const identity = readIdentity();
    if (!identity || snapshot?.repositoryState !== "ready" || !snapshot.upstream?.oid) {
      syncComparison = null;
      return;
    }
    try {
      const next = await readVersionSyncComparison(identity);
      if (parentSerial === requestSerial) syncComparison = next;
    } catch {
      if (parentSerial === requestSerial) syncComparison = null;
    }
  }

  async function runSnapshotMutation(
    label: string,
    operation: () => Promise<VersioningSnapshot>,
  ) {
    if (mutationBlockedReason) {
      error = mutationBlockedReason;
      return;
    }
    busyAction = label;
    error = "";
    try {
      snapshot = await operation();
      if (snapshot) hydrateIdentity(snapshot);
      if (snapshot) hydrateRemoteSelection(snapshot);
      diff = null;
      integrationPlan = null;
      integrationDiff = null;
      if (!(await settlePublishedEffect(
        t("versions-backend-effect", { label }),
        async () => {
          await refreshHistory(true);
          await refreshIntegrationRecovery();
          await refreshSyncComparison();
        },
      ))) return;
      onStatusUpdate(label, "saved");
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`${label}: ${error}`, "error");
    } finally {
      busyAction = "";
    }
  }

  async function runFileMutation(
    label: string,
    operation: () => Promise<{ snapshot: VersioningSnapshot }>,
  ) {
    if (mutationBlockedReason) {
      error = mutationBlockedReason;
      return;
    }
    busyAction = label;
    error = "";
    try {
      const receipt = await operation();
      snapshot = receipt.snapshot;
      diff = null;
      integrationPlan = null;
      integrationDiff = null;
      onStatusUpdate(label, "saved");
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`${label}: ${error}`, "error");
    } finally {
      busyAction = "";
    }
  }

  async function commit() {
    if (!commitMessage.trim()) {
      error = t("versions-commit-message-required");
      return;
    }
    if (mutationBlockedReason) {
      error = mutationBlockedReason;
      return;
    }
    busyAction = "commit";
    error = "";
    try {
      const receipt = await commitVersioning(mutationIdentity(), commitMessage);
      commitMessage = "";
      if (receipt.snapshot) snapshot = receipt.snapshot;
      else await refresh();
      if (!(await settlePublishedEffect(
        t("versions-commit-published-backend"),
        () => refreshHistory(true),
      ))) return;
      diff = null;
      integrationPlan = null;
      integrationDiff = null;
      const diagnostic = receipt.diagnostic
        ? ` ${t("versions-technical-details-available")}`
        : "";
      onStatusUpdate(
        t("versions-created-status", {
          oid: receipt.commitOid.slice(0, 8),
          diagnostic,
        }),
        receipt.publicationStatus === "published" ? "saved" : "error",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(t("versions-commit-blocked", { message: error }), "error");
    } finally {
      busyAction = "";
    }
  }

  async function showFileDiff(file: VersionFileStatus, kind: VersionDiffKind) {
    const identity = readIdentity();
    if (!identity) return;
    busyAction = `diff:${kind}:${file.path}`;
    error = "";
    try {
      diff = await readVersionDiff(identity, { kind, path: file.path });
    } catch (reason) {
      error = errorMessage(reason);
    } finally {
      busyAction = "";
    }
  }

  async function showCommitDiff(entry: VersionHistoryEntry) {
    const identity = readIdentity();
    if (!identity) return;
    busyAction = `diff:commit:${entry.oid}`;
    error = "";
    try {
      diff = await readVersionDiff(identity, { kind: "commit", commitOid: entry.oid });
    } catch (reason) {
      error = errorMessage(reason);
    } finally {
      busyAction = "";
    }
  }

  async function previewCommit(entry: VersionHistoryEntry) {
    const identity = readIdentity();
    if (!identity) return;
    busyAction = `preview:${entry.oid}`;
    error = "";
    try {
      if (activePreviewCommitOid) await returnToLivePreview();
      const receipt = await previewVersion(identity, entry.oid);
      await showPreview(receipt);
      onStatusUpdate(
        t("versions-preview-status", { oid: receipt.shortOid }),
        "saved",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(t("versions-preview-blocked", { message: error }), "error");
    } finally {
      busyAction = "";
    }
  }

  function requestRestore(entry: VersionHistoryEntry) {
    if (entry.oid === snapshot?.headOid) {
      error = t("versions-already-current");
      return;
    }
    restoreEntry = entry;
    restoreMessage = t("versions-restore-message", {
      oid: entry.shortOid,
      subject: entry.subject,
    });
    restoreConfirmation = "";
    error = "";
  }

  function cancelRestore() {
    restoreEntry = null;
    restoreMessage = "";
    restoreConfirmation = "";
  }

  async function restoreCommit() {
    const entry = restoreEntry;
    if (!entry) return;
    if (!snapshot?.clean) {
      error = t("versions-restore-clean-required");
      return;
    }
    if (workspaceDirty) {
      error = mutationBlockedReason;
      return;
    }
    if (!restoreMessage.trim()) {
      error = t("versions-restore-message-required");
      return;
    }
    if (restoreConfirmation.trim() !== entry.shortOid) {
      error = t("versions-confirmation-exact", { value: entry.shortOid });
      return;
    }
    busyAction = `restore:${entry.oid}`;
    error = "";
    try {
      if (activePreviewCommitOid) await returnToLivePreview();
      const receipt = await restoreVersioning(
        mutationIdentity(),
        entry.oid,
        restoreMessage,
      );
      if (receipt.snapshot) snapshot = receipt.snapshot;
      if (!(await settlePublishedEffect(
        t("versions-restore-terminal-backend"),
        () => afterRestore(receipt),
      ))) return;
      if (receipt.status === "recovery_required") {
        await refresh();
        error = t("versions-restore-recovery-required");
        onStatusUpdate(error, "error");
        return;
      }
      await refresh();
      cancelRestore();
      const diagnostic = receipt.diagnostic
        ? ` ${t("versions-technical-details-available")}`
        : "";
      onStatusUpdate(
        receipt.status === "noop"
          ? t("versions-restore-noop-status", { oid: entry.shortOid, diagnostic })
          : t("versions-restored-status", { oid: entry.shortOid, diagnostic }),
        "restored",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(t("versions-restore-blocked", { message: error }), "error");
    } finally {
      busyAction = "";
    }
  }

  function recoveryActionLabel(action: VersionRestoreRecoveryAction) {
    if (action === "finalize") return t("versions-recovery-finalize-restore");
    if (action === "rollback") return t("versions-recovery-rollback");
    return t("versions-recovery-clear-marker");
  }

  async function resolveRecovery(
    item: VersionRestoreRecoveryItem,
    action: VersionRestoreRecoveryAction,
  ) {
    busyAction = `recovery:${item.transactionId}:${action}`;
    error = "";
    try {
      const receipt = await resolveVersionRestoreRecovery(
        mutationIdentity(),
        item.recoveryRef,
        action,
      );
      if (receipt.snapshot) snapshot = receipt.snapshot;
      if (!(await settlePublishedEffect(
        t("versions-restore-recovery-backend"),
        () => afterRecovery(receipt),
      ))) return;
      await refresh();
      if (!receipt.resolved) {
        error = t("versions-recovery-not-finished");
        onStatusUpdate(error, "error");
        return;
      }
      onStatusUpdate(
        t("versions-recovery-resolved", {
          action: recoveryActionLabel(action),
        }),
        "restored",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(t("versions-recovery-blocked", { message: error }), "error");
    } finally {
      busyAction = "";
    }
  }

  function networkOperationId(kind: "fetch" | "push") {
    const random = globalThis.crypto?.randomUUID?.().replaceAll("-", "")
      ?? Math.random().toString(16).slice(2);
    return `${kind}-${Date.now()}-${random}`;
  }

  function editRemote(name: string) {
    const remote = snapshot?.remotes.find((item) => item.name === name);
    if (!remote) return;
    remoteName = remote.name;
    remoteFetchUrl = remote.usable ? remote.fetchUrl : "";
    remotePushUrl = remote.usable && remote.pushUrl !== remote.fetchUrl ? remote.pushUrl : "";
    pendingRemoteRemoval = "";
    remoteRemovalConfirmation = "";
  }

  async function saveRemote() {
    if (!remoteName.trim() || !remoteFetchUrl.trim()) {
      error = t("versions-remote-required");
      return;
    }
    await runSnapshotMutation(t("versions-remote-saved"), () => configureVersionRemote(
      mutationIdentity(),
      {
        name: remoteName.trim(),
        fetchUrl: remoteFetchUrl.trim(),
        pushUrl: remotePushUrl.trim() || null,
      },
    ));
  }

  async function removeRemoteConfirmed() {
    if (!pendingRemoteRemoval || remoteRemovalConfirmation !== pendingRemoteRemoval) {
      error = t("versions-remote-confirmation-mismatch");
      return;
    }
    const name = pendingRemoteRemoval;
    await runSnapshotMutation(t("versions-remote-removed", { name }), () => removeVersionRemote(
      mutationIdentity(),
      name,
    ));
    pendingRemoteRemoval = "";
    remoteRemovalConfirmation = "";
  }

  async function fetchRemote() {
    if (!selectedRemote) {
      error = t("versions-choose-remote");
      return;
    }
    const operationId = networkOperationId("fetch");
    activeNetwork = {
      schemaVersion: 2,
      projectRoot,
      sessionId,
      operationId,
      kind: "fetch",
      status: "started",
      messageDiagnostic: {
        schemaVersion: 1,
        code: "versions-fetch-started",
        arguments: {},
      },
    };
    busyAction = `fetch:${selectedRemote}`;
    error = "";
    try {
      const receipt = await fetchVersionRemote(mutationIdentity(), {
        operationId,
        remote: selectedRemote,
        prune: true,
      });
      snapshot = receipt.snapshot;
      hydrateRemoteSelection(receipt.snapshot);
      integrationPlan = null;
      integrationDiff = null;
      if (!(await settlePublishedEffect(
        t("versions-fetch-backend", { remote: selectedRemote }),
        async () => {
          await Promise.all([
            refreshHistory(true),
            refreshIntegrationRecovery(),
            refreshSyncComparison(),
          ]);
        },
      ))) return;
      onStatusUpdate(
        receipt.changed
          ? t("versions-fetch-updated", { remote: selectedRemote })
          : t("versions-fetch-no-updates", { remote: selectedRemote }),
        "saved",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(t("versions-fetch-blocked", { message: error }), "error");
    } finally {
      busyAction = "";
    }
  }

  async function pushBranch() {
    if (!snapshot?.branch || !selectedRemote) {
      error = t("versions-push-required");
      return;
    }
    const branch = snapshot.branch;
    const remoteBranch = selectedRemoteBranch || branch;
    const operationId = networkOperationId("push");
    activeNetwork = {
      schemaVersion: 2,
      projectRoot,
      sessionId,
      operationId,
      kind: "push",
      status: "started",
      messageDiagnostic: {
        schemaVersion: 1,
        code: "versions-push-started",
        arguments: {},
      },
    };
    busyAction = `push:${selectedRemote}/${remoteBranch}`;
    error = "";
    try {
      const receipt = await pushVersionBranch(mutationIdentity(), {
        operationId,
        remote: selectedRemote,
        remoteBranch,
        setUpstream: !snapshot.upstream
          || snapshot.upstream.remote !== selectedRemote
          || snapshot.upstream.remoteBranch !== remoteBranch,
      });
      snapshot = receipt.snapshot;
      hydrateRemoteSelection(receipt.snapshot);
      integrationPlan = null;
      integrationDiff = null;
      await refreshSyncComparison();
      onStatusUpdate(
        t("versions-push-published", {
          branch,
          remote: selectedRemote,
          remoteBranch,
        }),
        "saved",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(t("versions-push-blocked", { message: error }), "error");
    } finally {
      busyAction = "";
    }
  }

  async function cancelNetwork() {
    const identity = readIdentity();
    if (!identity || !activeNetwork) return;
    try {
      const receipt = await cancelVersionNetworkOperation(identity, activeNetwork.operationId);
      if (!receipt.cancellationRequested) {
        activeNetwork = null;
      }
    } catch (reason) {
      error = errorMessage(reason);
    }
  }

  async function saveUpstream() {
    if (!snapshot?.branch || !selectedRemote || !selectedRemoteBranch) {
      error = t("versions-upstream-required");
      return;
    }
    await runSnapshotMutation(t("versions-upstream-saved"), () => configureVersionUpstream(
      mutationIdentity(),
      {
        localBranch: snapshot!.branch!,
        remote: selectedRemote,
        remoteBranch: selectedRemoteBranch,
      },
    ));
  }

  async function removeUpstream() {
    if (!snapshot?.branch) return;
    await runSnapshotMutation(t("versions-upstream-removed"), () => clearVersionUpstream(
      mutationIdentity(),
      snapshot!.branch!,
    ));
  }

  async function createBranch() {
    const name = newBranchName.trim();
    if (!name) {
      error = t("versions-branch-name-required");
      return;
    }
    await runSnapshotMutation(t("versions-branch-created", { name }), () => createVersionBranch(
      mutationIdentity(),
      name,
    ));
    newBranchName = "";
  }

  async function switchBranch(branch: string, oid: string | null) {
    if (!oid || !snapshot?.clean) {
      error = t("versions-switch-clean-required");
      return;
    }
    busyAction = `switch:${branch}`;
    error = "";
    try {
      const receipt = await switchVersionBranch(mutationIdentity(), branch, oid);
      if (receipt.snapshot) snapshot = receipt.snapshot;
      if (!(await settlePublishedEffect(
        t("versions-branch-switched-backend", { branch }),
        () => afterIntegration(receipt),
      ))) return;
      await refresh();
      onStatusUpdate(t("versions-active-branch", { branch }), "restored");
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(t("versions-switch-blocked", { message: error }), "error");
    } finally {
      busyAction = "";
    }
  }

  async function deleteBranch(branch: string) {
    if (branchRemovalConfirmation !== branch) {
      error = t("versions-delete-branch-confirmation", { branch });
      return;
    }
    await runSnapshotMutation(t("versions-branch-removed", { branch }), () => deleteVersionBranch(
      mutationIdentity(),
      branch,
    ));
    pendingBranchRemoval = "";
    branchRemovalConfirmation = "";
  }

  function selectedTarget() {
    return snapshot?.remoteBranches.find(
      (branch) => branch.remote === selectedRemote && branch.name === selectedRemoteBranch,
    ) ?? null;
  }

  async function analyzeIntegration() {
    const identity = readIdentity();
    const target = selectedTarget();
    if (!identity || !target) {
      error = t("versions-integration-target-required");
      return;
    }
    busyAction = `plan:${target.refName}`;
    error = "";
    try {
      const [plan, previewDiff] = await Promise.all([
        readVersionIntegrationPlan(identity, target.refName, target.oid),
        readVersionDiff(identity, {
          kind: "integration",
          targetRef: target.refName,
          expectedTargetOid: target.oid,
        }),
      ]);
      integrationPlan = plan;
      integrationDiff = previewDiff;
      integrationMessage = t("versions-integration-message", {
        remote: target.remote,
        branch: target.name,
      });
    } catch (reason) {
      error = errorMessage(reason);
      integrationPlan = null;
      integrationDiff = null;
    } finally {
      busyAction = "";
    }
  }

  async function applyIntegration(mode: VersionIntegrationMode) {
    const plan = integrationPlan;
    if (!plan || !integrationMessage.trim()) return;
    busyAction = `integrate:${mode}`;
    error = "";
    try {
      if (activePreviewCommitOid) await returnToLivePreview();
      const receipt = await integrateVersionTarget(mutationIdentity(), {
        targetRef: plan.targetRef,
        expectedTargetOid: plan.targetOid,
        mode,
        message: integrationMessage.trim(),
      });
      if (receipt.snapshot) snapshot = receipt.snapshot;
      integrationPlan = null;
      integrationDiff = null;
      if (!(await settlePublishedEffect(
        t("versions-integration-backend"),
        () => afterIntegration(receipt),
      ))) return;
      await refresh();
      if (receipt.status === "conflict_resolution_required") {
        onStatusUpdate(
          t("versions-conflicts-count", { count: receipt.conflictPaths.length }),
          "error",
        );
      } else if (receipt.status === "recovery_required") {
        error = t("versions-integration-recovery-required");
        onStatusUpdate(error, "error");
      } else {
        onStatusUpdate(
          receipt.status === "noop"
            ? t("versions-target-already-integrated")
            : t("versions-integration-published"),
          "restored",
        );
      }
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(t("versions-integration-blocked", { message: error }), "error");
    } finally {
      busyAction = "";
    }
  }

  function integrationRecoveryActionLabel(action: VersionIntegrationRecoveryAction) {
    if (action === "finalize") return t("versions-recovery-finalize-integration");
    if (action === "continue") return t("versions-recovery-continue-merge");
    if (action === "rollback") return t("versions-recovery-cancel-return");
    return t("versions-recovery-clear-marker");
  }

  async function resolveIntegrationRecovery(
    item: VersionIntegrationRecoveryItem,
    action: VersionIntegrationRecoveryAction,
  ) {
    busyAction = `integration-recovery:${item.transactionId}:${action}`;
    error = "";
    try {
      const receipt = await resolveVersionIntegrationRecovery(
        mutationIdentity(),
        item.recoveryRef,
        action,
      );
      if (receipt.snapshot) snapshot = receipt.snapshot;
      if (!(await settlePublishedEffect(
        t("versions-integration-recovery-backend"),
        () => afterIntegrationRecovery(receipt),
      ))) return;
      await refresh();
      if (!receipt.resolved) {
        error = t("versions-integration-still-recovery");
        onStatusUpdate(error, "error");
      } else {
        onStatusUpdate(
          t("versions-integration-recovery-status", {
            action: integrationRecoveryActionLabel(action),
          }),
          "restored",
        );
      }
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(t("versions-integration-recovery-blocked", { message: error }), "error");
    } finally {
      busyAction = "";
    }
  }

  function kindLabel(file: VersionFileStatus) {
    const labels: Record<VersionFileStatus["kind"], string> = {
      added: "A",
      modified: "M",
      deleted: "D",
      renamed: "R",
      copied: "C",
      type_changed: "T",
      untracked: "?",
      conflicted: "!",
      unknown: "·",
    };
    return labels[file.kind];
  }

  function networkKindLabel(kind: VersionNetworkProgressEvent["kind"]) {
    return kind === "fetch"
      ? t("versions-network-kind-fetch")
      : t("versions-network-kind-push");
  }

  function networkStatusLabel(status: VersionNetworkProgressEvent["status"]) {
    switch (status) {
      case "started": return t("versions-network-status-started");
      case "progress": return t("versions-network-status-progress");
      case "completed": return t("versions-network-status-completed");
      case "failed": return t("versions-network-status-failed");
      case "cancelled": return t("versions-network-status-cancelled");
    }
  }

  function versionStateLabel(value: string) {
    switch (value) {
      case "no_upstream": return t("versions-state-no-upstream");
      case "upstream_missing": return t("versions-state-upstream-missing");
      case "unborn": return t("versions-state-unborn");
      case "up_to_date": return t("versions-state-up-to-date");
      case "ahead": return t("versions-state-ahead");
      case "behind": return t("versions-state-behind");
      case "diverged": return t("versions-state-diverged");
      case "same": return t("versions-state-same");
      case "fast_forward": return t("versions-state-fast-forward");
      case "local_ahead": return t("versions-state-local-ahead");
      case "merge_clean": return t("versions-state-merge-clean");
      case "merge_conflict": return t("versions-state-merge-conflict");
      case "merge_resolved": return t("versions-state-merge-resolved");
      case "switch_branch": return t("versions-state-switch-branch");
      case "ready_to_finalize": return t("versions-state-ready-finalize");
      case "conflict_resolution": return t("versions-state-conflict-resolution");
      case "ready_to_rollback": return t("versions-state-ready-rollback");
      case "cleanup_required": return t("versions-state-cleanup-required");
      case "manual_review": return t("versions-state-manual-review");
      default: return t("versions-state-unknown");
    }
  }

  function formatDate(value: string) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return l10n.formatDate(date, {
      dateStyle: "medium",
      timeStyle: "short",
    });
  }

  onMount(() => {
    let disposed = false;
    let unlisten: () => void = () => {};
    void listen<VersionNetworkProgressEvent>(
      "pana-versioning-network-progress",
      (event) => {
        const payload = event.payload;
        if (payload.projectRoot !== projectRoot || payload.sessionId !== sessionId) return;
        activeNetwork = payload;
        if (["completed", "failed", "cancelled"].includes(payload.status)) {
          window.setTimeout(() => {
            if (activeNetwork?.operationId === payload.operationId) activeNetwork = null;
          }, 2500);
        }
      },
    ).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten();
    };
  });

  $effect(() => {
    const root = projectRoot;
    const session = sessionId;
    if (!root || !session) {
      requestSerial += 1;
      snapshot = null;
      history = [];
      diff = null;
      error = "";
      recovery = null;
      integrationRecovery = null;
      syncComparison = null;
      integrationPlan = null;
      integrationDiff = null;
      pendingRemoteRemoval = "";
      remoteRemovalConfirmation = "";
      pendingBranchRemoval = "";
      branchRemovalConfirmation = "";
      activeNetwork = null;
      cancelRestore();
      hydratedIdentityToken = "";
      return;
    }
    void refresh();
  });
</script>

<section class="activity-workspace activity-workspace-scroll versions-panel versioning-workspace" aria-labelledby="version-control-title">
    <header class="workspace-header panel-header">
      <div class="title-block">
        <span class="eyebrow"><IconGitBranch size={15} stroke={1.9} /> {t("versions-eyebrow")}</span>
        <h1 id="version-control-title">{t(UI_TERM_IDS.versionControl)}</h1>
        <p>{t("versions-description")}</p>
      </div>
      <div class="header-summary">
        {#if snapshot}
          <dl class="header-metrics">
            <div><dt>{t("versions-editor")}</dt><dd class:warning={workspaceDirty}>{workspaceDirty ? t("versions-unsaved") : t("versions-saved")}</dd></div>
            <div><dt>{t("versions-git")}</dt><dd>{snapshot.repositoryState === "ready"
              ? (snapshot.clean ? t("versions-clean") : t("versions-modified"))
              : t("versions-uninitialized")}</dd></div>
            <div><dt>{t("versions-staged")}</dt><dd>{l10n.formatNumber(snapshot.stagedCount)}</dd></div>
            <div><dt>{t("versions-changes")}</dt><dd>{l10n.formatNumber(snapshot.unstagedCount)}</dd></div>
          </dl>
        {/if}
        <button type="button" class="ui-button toolbar refresh-button" disabled={loading || !!busyAction} onclick={() => refresh()}>
          <IconRefresh size={16} stroke={1.9} /> {t("versions-refresh")}
        </button>
      </div>
    </header>

    {#if loading && !snapshot}
      <p class="empty-text">{t("versions-loading")}</p>
    {:else if !projectRoot || !sessionId}
      <p class="empty-text">{t("versions-open-project")}</p>
    {:else if snapshot}
      <section class="repository-card" class:problem={snapshot.repositoryState !== "ready" && snapshot.repositoryState !== "uninitialized"}>
        <div class="repository-state">
          <span class="state-dot" class:clean={snapshot.repositoryState === "ready" && snapshot.clean}></span>
          <div>
            <strong>{snapshot.repositoryState === "ready" ? (snapshot.branch ?? "detached HEAD") : snapshot.repositoryState}</strong>
            <small title={snapshot.repositoryRoot}>{snapshot.repositoryRoot}</small>
          </div>
          {#if snapshot.headOid}<span class="head-reference">HEAD <code>{snapshot.headOid.slice(0, 8)}</code></span>{/if}
        </div>
        {#if snapshot.diagnostic}<p class="diagnostic">{t("versions-technical-diagnostic")}</p>{/if}
        {#if mutationBlockedReason}<p class="guard-message"><IconAlertTriangle size={14} /> {mutationBlockedReason}</p>{/if}
      </section>

      {#if activePreviewCommitOid}
        <section class="preview-banner">
          <div><IconEye size={15} /><span>{t("versions-isolated-preview")} <code>{activePreviewCommitOid.slice(0, 8)}</code></span></div>
          <button type="button" onclick={returnToLivePreview}>{t("versions-return-current")}</button>
        </section>
      {/if}

      {#if activeNetwork}
        <section class="network-progress" aria-live="polite">
          <div>
            <strong>{networkKindLabel(activeNetwork.kind)} · {networkStatusLabel(activeNetwork.status)}</strong>
            <small>{errorMessage(activeNetwork.messageDiagnostic)}</small>
          </div>
          {#if activeNetwork.status === "started" || activeNetwork.status === "progress"}
            <button type="button" onclick={cancelNetwork}>{t("versions-cancel")}</button>
          {/if}
        </section>
      {/if}

      {#if recovery?.items.length}
        <section class="recovery-section" aria-label={t("versions-restore-recovery-label")}>
          <div class="recovery-title"><IconAlertTriangle size={16} /><div><strong>{t("versions-restore-recovery-title")}</strong><small>{t("versions-pending-transactions", { count: recovery.items.length })}</small></div></div>
          {#each recovery.items as item (item.recoveryRef)}
            <article class="recovery-item" class:manual={item.state === "manual_review"}>
              <div class="recovery-meta"><code>{item.targetCommitOid.slice(0, 8)}</code><span>{versionStateLabel(item.state)}</span></div>
              <p>{t("versions-technical-diagnostic")}</p>
              {#if item.availableActions.length}
                <div class="recovery-actions">
                  {#each item.availableActions as action}
                    <button type="button" disabled={!!busyAction || workspaceDirty} onclick={() => resolveRecovery(item, action)}>{recoveryActionLabel(action)}</button>
                  {/each}
                </div>
              {/if}
            </article>
          {/each}
        </section>
      {/if}

      {#if integrationRecovery?.items.length}
        <section class="recovery-section integration-recovery" aria-label={t("versions-integration-recovery-label")}>
          <div class="recovery-title"><IconAlertTriangle size={16} /><div><strong>{t("versions-integration-recovery-title")}</strong><small>{t("versions-active-transactions", { count: integrationRecovery.items.length })}</small></div></div>
          {#each integrationRecovery.items as item (item.recoveryRef)}
            <article class="recovery-item" class:manual={item.state === "manual_review"}>
              <div class="recovery-meta"><code>{item.targetOid.slice(0, 8)}</code><span>{versionStateLabel(item.kind)} · {versionStateLabel(item.state)}</span></div>
              <p>{t("versions-technical-diagnostic")}</p>
              {#if item.conflictPaths.length}
                <ul class="conflict-list">
                  {#each item.conflictPaths as path}<li><code>{path}</code></li>{/each}
                </ul>
              {/if}
              {#if item.availableActions.length}
                <div class="recovery-actions">
                  {#each item.availableActions as action}
                    <button type="button" disabled={!!busyAction || workspaceDirty} onclick={() => resolveIntegrationRecovery(item, action)}>{integrationRecoveryActionLabel(action)}</button>
                  {/each}
                </div>
              {/if}
            </article>
          {/each}
        </section>
      {/if}

      {#if snapshot.repositoryState === "uninitialized"}
        <section class="setup-card">
          <span class="setup-icon"><IconGitCommit size={21} stroke={1.8} /></span>
          <div>
            <strong>{t("versions-not-initialized")}</strong>
            <p>{t("versions-init-description")}</p>
          </div>
          <button class="ui-button primary" type="button" disabled={!!busyAction || workspaceDirty} onclick={() => runSnapshotMutation(t("versions-repository-initialized"), () => initializeVersioning(mutationIdentity()))}>
            <IconGitBranch size={15} stroke={1.9} /> {t("versions-initialize")}
          </button>
        </section>
      {:else if snapshot.repositoryState === "ready"}
        <details class="identity-card" open={!snapshot.userName || !snapshot.userEmail}>
          <summary><IconSettings size={15} stroke={1.8} /> {t("versions-local-identity")}</summary>
          <div class="identity-fields">
            <label>{t("versions-name")}<input bind:value={identityName} autocomplete="name" /></label>
            <label>{t("versions-email")}<input type="email" bind:value={identityEmail} autocomplete="email" /></label>
            <button class="ui-button primary" type="button" disabled={!!busyAction || workspaceDirty || !identityName.trim() || !identityEmail.trim()} onclick={() => runSnapshotMutation(t("versions-identity-saved"), () => configureVersioningIdentity(mutationIdentity(), { name: identityName, email: identityEmail }))}>
              {t("versions-save-identity")}
            </button>
          </div>
        </details>

        <details class="remote-card" open={snapshot.remotes.length === 0}>
          <summary><IconSettings size={15} stroke={1.8} /> {t("versions-remotes-auth")}</summary>
          <p class="card-hint">{t("versions-auth-hint")}</p>
          {#if snapshot.remotes.length}
            <div class="remote-list">
              {#each snapshot.remotes as remote (remote.name)}
                <article class="remote-row" class:invalid={!remote.usable}>
                  <button type="button" class="remote-main" onclick={() => editRemote(remote.name)}>
                    <strong>{remote.name}</strong>
                    <small title={remote.fetchUrl}>{remote.fetchUrl}</small>
                  </button>
                  <button type="button" class="mini-button" title={t("versions-remove-remote")} aria-label={t("versions-remove-remote-label", { name: remote.name })} disabled={!!busyAction || !!mutationBlockedReason} onclick={() => { pendingRemoteRemoval = remote.name; remoteRemovalConfirmation = ""; }}>
                    <IconX size={13} stroke={1.9} />
                  </button>
                  {#if remote.diagnostic}<p>{t("versions-remote-configuration-invalid")}</p>{/if}
                </article>
              {/each}
            </div>
          {/if}
          <div class="remote-form">
            <label>{t("versions-name")}<input bind:value={remoteName} placeholder="origin" autocomplete="off" /></label>
            <label class="span-2">{t("versions-fetch-url")}<input bind:value={remoteFetchUrl} placeholder="https://github.com/organization/site.git" autocomplete="off" spellcheck="false" /></label>
            <label class="span-2">{t("versions-push-url-optional")}<input bind:value={remotePushUrl} placeholder="ssh://git@github.com/organization/site.git" autocomplete="off" spellcheck="false" /></label>
            <button type="button" class="span-2" disabled={!!busyAction || !!mutationBlockedReason || !remoteName.trim() || !remoteFetchUrl.trim()} onclick={saveRemote}>{t("versions-save-remote")}</button>
          </div>
          {#if pendingRemoteRemoval}
            <div class="destructive-confirmation">
              <p>{t("versions-remove-remote-description")}</p>
              <label>{t("versions-type-value", { value: pendingRemoteRemoval })}<input bind:value={remoteRemovalConfirmation} autocomplete="off" /></label>
              <div><button type="button" onclick={() => { pendingRemoteRemoval = ""; }}>{t("versions-abandon")}</button><button type="button" class="ui-button danger danger-button" disabled={remoteRemovalConfirmation !== pendingRemoteRemoval} onclick={removeRemoteConfirmed}>{t("versions-remove")}</button></div>
            </div>
          {/if}
        </details>

        {#if snapshot.remotes.length}
          <section class="sync-card">
            <div class="section-heading">
              <div><p class="section-label">{t("versions-remote-sync")}</p><span>{t("versions-sync-flow")}</span></div>
              <span class="sync-badge">{versionStateLabel(snapshot.syncState)}</span>
            </div>
            <div class="sync-selectors">
              <label>{t("versions-remote")}
                <select bind:value={selectedRemote} onchange={() => { selectedRemoteBranch = snapshot?.remoteBranches.find((branch) => branch.remote === selectedRemote)?.name ?? ""; integrationPlan = null; integrationDiff = null; }}>
                  <option value="">{t("versions-choose-remote")}</option>
                  {#each usableRemotes as remote}<option value={remote.name}>{remote.name}</option>{/each}
                </select>
              </label>
              <label>{t("versions-remote-branch")}
                <select bind:value={selectedRemoteBranch} onchange={() => { integrationPlan = null; integrationDiff = null; }}>
                  <option value="">{t("versions-choose-branch")}</option>
                  {#each selectedRemoteBranches as branch}<option value={branch.name}>{branch.name}</option>{/each}
                </select>
              </label>
            </div>
            <div class="sync-counters">
              <span>{t("versions-ahead")} <b>{l10n.formatNumber(snapshot.upstream?.ahead ?? 0)}</b></span>
              <span>{t("versions-behind")} <b>{l10n.formatNumber(snapshot.upstream?.behind ?? 0)}</b></span>
              <span>{t("versions-upstream")} <b>{snapshot.upstream ? `${snapshot.upstream.remote}/${snapshot.upstream.remoteBranch}` : t("versions-upstream-unconfigured")}</b></span>
            </div>
            <div class="button-grid">
              <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !selectedRemote} onclick={fetchRemote}>{t("versions-fetch-prune")}</button>
              <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.branch || !selectedRemote} onclick={pushBranch}>{t("versions-safe-push")}</button>
              <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.branch || !selectedRemoteBranch} onclick={saveUpstream}>{t("versions-set-upstream")}</button>
              <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.upstream} onclick={removeUpstream}>{t("versions-remove-upstream")}</button>
            </div>
            <p class="card-hint">{t("versions-no-pull-hint")}</p>
            <button type="button" class="ui-button wide-button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.clean || !selectedTarget()} onclick={analyzeIntegration}>{t("versions-analyze-integration")}</button>
            {#if integrationPlan}
              <article class="integration-plan">
                <div><strong>{versionStateLabel(integrationPlan.relationship)}</strong><code>{integrationPlan.targetOid.slice(0, 8)}</code></div>
                <p>{t("versions-integration-plan-summary", {
                  ahead: integrationPlan.ahead,
                  behind: integrationPlan.behind,
                })}</p>
                <div class="comparison-grid">
                  <span>{t("versions-local-only")} <b>{l10n.formatNumber(integrationPlan.ahead)}</b></span>
                  <span>{t("versions-to-integrate")} <b>{l10n.formatNumber(integrationPlan.behind)}</b></span>
                </div>
                {#if integrationPlan.localOnly.length || integrationPlan.targetOnly.length}
                  <div class="integration-history">
                    {#if integrationPlan.targetOnly.length}
                      <strong>{t("versions-target-commits")}</strong>
                      {#each integrationPlan.targetOnly as entry (entry.oid)}
                        <div><code>{entry.shortOid}</code><span>{entry.subject}</span></div>
                      {/each}
                    {/if}
                    {#if integrationPlan.localOnly.length}
                      <strong>{t("versions-local-commits")}</strong>
                      {#each integrationPlan.localOnly as entry (entry.oid)}
                        <div><code>{entry.shortOid}</code><span>{entry.subject}</span></div>
                      {/each}
                    {/if}
                  </div>
                {/if}
                {#if integrationDiff}
                  <details class="integration-diff">
                    <summary>{t("versions-target-patch-preview")}{integrationDiff.truncated ? ` (${t("versions-truncated")})` : ""}</summary>
                    {#if integrationDiff.binary}
                      <p>{t("versions-binary-preview")}</p>
                    {:else if integrationDiff.patch}
                      <pre>{integrationDiff.patch}{integrationDiff.truncated ? `\n\n… ${t("versions-diff-truncated")}` : ""}</pre>
                    {:else}
                      <p>{t("versions-no-target-diff")}</p>
                    {/if}
                  </details>
                {/if}
                <label>{t("versions-merge-message")}<textarea rows="2" bind:value={integrationMessage}></textarea></label>
                <div class="button-grid">
                  <button type="button" class="ui-button primary primary-button" disabled={!integrationPlan.fastForwardAllowed || !!busyAction} onclick={() => applyIntegration("fast_forward")}>{t("versions-fast-forward")}</button>
                  <button type="button" class="ui-button primary primary-button" disabled={!integrationPlan.mergeAllowed || !!busyAction || !integrationMessage.trim()} onclick={() => applyIntegration("merge")}>{t("versions-explicit-merge")}</button>
                </div>
              </article>
            {/if}
          </section>
        {/if}

        <details class="branches-card">
          <summary><IconGitCommit size={15} stroke={1.8} /> {t("versions-local-branches")}</summary>
          <div class="branch-create">
            <input bind:value={newBranchName} placeholder="feature/new-page" autocomplete="off" spellcheck="false" />
            <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.headOid || !newBranchName.trim()} onclick={createBranch}>{t("versions-create")}</button>
          </div>
          <div class="branch-list">
            {#each snapshot.branches as branch (branch.name)}
              <article class="branch-row" class:current={branch.current}>
                <div><strong>{branch.name}</strong><small>{branch.current ? t("versions-active") : versionStateLabel(branch.syncState)}</small></div>
                {#if !branch.current}
                  <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.clean || !branch.oid} onclick={() => switchBranch(branch.name, branch.oid)}>{t("versions-open")}</button>
                  <button type="button" class="mini-button" title={t("versions-delete-integrated-title")} aria-label={t("versions-delete-branch-label", { branch: branch.name })} disabled={!!busyAction || !!mutationBlockedReason} onclick={() => { pendingBranchRemoval = branch.name; branchRemovalConfirmation = ""; }}>
                    <IconX size={13} stroke={1.9} />
                  </button>
                {/if}
              </article>
            {/each}
          </div>
          {#if pendingBranchRemoval}
            <div class="destructive-confirmation">
              <p>{t("versions-delete-branch-description", { branch: pendingBranchRemoval })}</p>
              <label>{t("versions-confirmation")}<input bind:value={branchRemovalConfirmation} autocomplete="off" spellcheck="false" /></label>
              <div>
                <button type="button" onclick={() => { pendingBranchRemoval = ""; branchRemovalConfirmation = ""; }}>{t("versions-abandon")}</button>
                <button type="button" class="ui-button danger danger-button" disabled={!!busyAction || branchRemovalConfirmation !== pendingBranchRemoval} onclick={() => deleteBranch(pendingBranchRemoval)}>{t("versions-delete-branch")}</button>
              </div>
            </div>
          {/if}
        </details>

        <section class="changes-section">
          <div class="section-heading">
            <div><p class="section-label">{t("versions-staged")}</p><span>{t("versions-files-count", { count: stagedFiles.length })}</span></div>
            <button type="button" disabled={!!busyAction || workspaceDirty || stagedFiles.length === 0} onclick={() => runFileMutation(t("versions-index-cleared"), () => unstageAllVersioning(mutationIdentity()))}>{t("versions-unstage-all")}</button>
          </div>
          {#if stagedFiles.length === 0}
            <p class="empty-row">{t("versions-no-staged")}</p>
          {:else}
            <div class="file-list">
              {#each stagedFiles as file (`staged:${file.path}`)}
                <article class:conflict={file.conflicted} class="file-row">
                  <button type="button" class="file-main" title={t("versions-show-staged-diff")} onclick={() => showFileDiff(file, "staged")}>
                    <b>{kindLabel(file)}</b><span>{file.path}</span>
                  </button>
                  <button type="button" class="mini-button" title={t("versions-unstage-all")} aria-label={t("versions-remove-from-staged", { path: file.path })} disabled={!!busyAction || workspaceDirty} onclick={() => runFileMutation(t("versions-removed-from-staged", { path: file.path }), () => unstageVersioningPaths(mutationIdentity(), [file.path]))}>
                    <IconMinus size={13} stroke={1.9} />
                  </button>
                </article>
              {/each}
            </div>
          {/if}
        </section>

        <section class="commit-card">
          <label for="version-message">{t("versions-version-message")}</label>
          <textarea id="version-message" rows="3" bind:value={commitMessage} placeholder={t("versions-version-placeholder")}></textarea>
          <button type="button" class="ui-button primary primary-button" disabled={!!busyAction || workspaceDirty || stagedFiles.length === 0 || snapshot.conflictedCount > 0 || !snapshot.userName || !snapshot.userEmail || !commitMessage.trim()} onclick={commit}>
            <IconGitCommit size={16} stroke={1.9} /> {t("versions-create-version")}
          </button>
        </section>

        <section class="changes-section">
          <div class="section-heading">
            <div><p class="section-label">{t("versions-changes")}</p><span>{t("versions-files-count", { count: unstagedFiles.length })}</span></div>
            <button type="button" disabled={!!busyAction || workspaceDirty || unstagedFiles.length === 0} onclick={() => runFileMutation(t("versions-all-staged"), () => stageAllVersioning(mutationIdentity()))}>{t("versions-stage-all")}</button>
          </div>
          {#if unstagedFiles.length === 0}
            <p class="empty-row"><IconCheck size={14} /> {t("versions-no-working-changes")}</p>
          {:else}
            <div class="file-list">
              {#each unstagedFiles as file (`unstaged:${file.path}`)}
                <article class:conflict={file.conflicted} class="file-row">
                  <button type="button" class="file-main" title={t("versions-show-diff")} onclick={() => showFileDiff(file, "unstaged")}>
                    <b>{kindLabel(file)}</b><span>{file.path}</span>
                  </button>
                  <button type="button" class="mini-button" title={t("versions-staged")} disabled={!!busyAction || workspaceDirty} onclick={() => runFileMutation(t("versions-file-staged", { path: file.path }), () => stageVersioningPaths(mutationIdentity(), [file.path]))}><IconPlus size={13} /></button>
                </article>
              {/each}
            </div>
          {/if}
        </section>

        {#if diff}
          <section class="diff-card">
            <div class="section-heading">
              <div><p class="section-label">{t("versions-diff-title", { kind: diff.kind })}</p><span>{diff.path ?? diff.commitOid?.slice(0, 8) ?? t("versions-version")}</span></div>
              <button type="button" class="ui-icon-button ui-close-button mini-button" title={t("versions-close-diff")} onclick={() => { diff = null; }}><IconX size={13} /></button>
            </div>
            {#if diff.binary}
              <p class="empty-row">{t("versions-binary-file")}</p>
            {:else if !diff.patch}
              <p class="empty-row">{t("versions-no-text-diff")}</p>
            {:else}
              <pre>{diff.patch}{diff.truncated ? `\n\n… ${t("versions-diff-truncated")}` : ""}</pre>
            {/if}
          </section>
        {/if}

        <section class="history-section">
          <div class="section-heading">
            <div><p class="section-label">{t("versions-git-history")}</p><span>{t("versions-loaded-versions", { count: history.length })}</span></div>
          </div>
          {#if history.length === 0}
            <p class="empty-row">{t("versions-first-commit")}</p>
          {:else}
            <div class="commit-list">
              {#each history as entry (entry.oid)}
                <article class="commit-row" class:active-preview={activePreviewCommitOid === entry.oid}>
                  <span class="commit-graph"></span>
                  <button type="button" class="commit-main" onclick={() => showCommitDiff(entry)}>
                    <span class="commit-content">
                      <strong>{entry.subject}</strong>
                      <small>{entry.authorName} · {formatDate(entry.authoredAt)}</small>
                    </span>
                    <code>{entry.shortOid}</code>
                  </button>
                  <button type="button" class="mini-button" title={t("versions-preview-version")} disabled={!!busyAction} onclick={() => previewCommit(entry)}><IconEye size={14} /></button>
                  <button type="button" class="mini-button restore-button" title={t("versions-restore-version")} disabled={!!busyAction || workspaceDirty || !snapshot.clean || entry.oid === snapshot.headOid} onclick={() => requestRestore(entry)}><IconRestore size={14} /></button>
                </article>
              {/each}
            </div>
            {#if historyHasMore}
              <button type="button" class="ui-button load-more" disabled={!!busyAction} onclick={() => refreshHistory(false)}><IconChevronDown size={15} /> {t("versions-load-older")}</button>
            {/if}
          {/if}
        </section>

        {#if restoreEntry}
          <section class="restore-card" aria-label={t("versions-restore-confirmation-label")}>
            <div class="restore-heading">
              <div>
                <p class="section-label">{t("versions-safe-restore")}</p>
                <strong>{restoreEntry.subject}</strong>
              </div>
              <code>{restoreEntry.shortOid}</code>
            </div>
            <p>{t("versions-restore-description")}</p>
            <label>{t("versions-commit-message")}<textarea rows="3" bind:value={restoreMessage}></textarea></label>
            <label>{t("versions-type-to-confirm", { value: restoreEntry.shortOid })}<input bind:value={restoreConfirmation} autocomplete="off" spellcheck="false" /></label>
            <div class="restore-actions">
              <button type="button" disabled={!!busyAction} onclick={cancelRestore}>{t("versions-abandon")}</button>
              <button type="button" class="ui-button danger danger-button" disabled={!!busyAction || restoreConfirmation.trim() !== restoreEntry.shortOid || !restoreMessage.trim()} onclick={restoreCommit}><IconRestore size={15} /> {t("versions-restore-new-commit")}</button>
            </div>
          </section>
        {/if}
      {/if}
    {/if}

    {#if error}<p class="error-message" role="alert">{error}</p>{/if}
</section>

<style>
  .versions-panel { position: relative; width: 100%; }
  .versions-panel .panel-header { position: sticky; top: 0; z-index: 3; }
  .panel-header, .header-summary, .repository-state, .section-heading, .file-row, .file-main, .commit-row, .guard-message, summary, .primary-button, .load-more, .empty-row { display: flex; align-items: center; }
  .panel-header, .section-heading { justify-content: space-between; gap: 10px; }
  .title-block { min-width: 0; }
  .panel-header h1, .eyebrow, .section-label, p { margin: 0; }
  .panel-header h1 { margin-top: 6px; color: var(--text-strong); font-size: 20px; font-weight: 650; letter-spacing: -.015em; line-height: 1.15; }
  .title-block p { margin-top: 5px; color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; }
  .eyebrow { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  .section-label { color: var(--wb-accent-strong); font-size: 11px; font-weight: 750; letter-spacing: .04em; text-transform: uppercase; }
  .header-summary { flex: 0 0 auto; gap: 10px; }
  .header-metrics { display: flex; gap: 7px; margin: 0; }
  .header-metrics div { min-width: 82px; padding: 7px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .header-metrics dt { color: var(--wb-text-muted); font-size: 11px; font-weight: 650; text-transform: uppercase; }
  .header-metrics dd { overflow: hidden; max-width: 104px; margin: 3px 0 0; color: var(--text-strong); font-size: 13px; font-weight: 650; text-overflow: ellipsis; white-space: nowrap; }
  .header-metrics dd.warning { color: var(--warning-strong, #a66a00); }
  button, input, textarea, select { font: inherit; }
  button { cursor: pointer; }
  button:disabled { cursor: default; opacity: .45; }
  button:focus-visible, input:focus-visible, textarea:focus-visible, select:focus-visible, summary:focus-visible { outline: 2px solid var(--wb-focus-ring, var(--wb-accent)); outline-offset: 1px; }
  .mini-button { display: inline-flex; align-items: center; justify-content: center; padding: 0; border: 1px solid var(--wb-border-subtle); border-radius: var(--wb-radius-control, 5px); background: var(--wb-surface-chrome); color: var(--wb-text-muted); }
  .refresh-button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 32px; padding: 0 11px; border: 1px solid var(--wb-border-subtle, var(--border)); border-radius: var(--wb-radius-control, 4px); color: var(--wb-text-primary, var(--text)); background: var(--wb-surface-document, var(--surface)); font-size: 12px; font-weight: 600; }
  .mini-button { flex: 0 0 27px; width: 27px; height: 27px; }
  .repository-card, .setup-card, .identity-card, .remote-card, .sync-card, .branches-card, .changes-section, .commit-card, .diff-card, .history-section, .preview-banner, .network-progress, .restore-card, .recovery-section { margin: 10px 20px 0; border: 1px solid var(--wb-border-subtle, var(--border-3)); border-radius: 7px; background: var(--wb-surface-document, var(--surface-2)); }
  .repository-card { display: grid; gap: 8px; padding: 9px 11px; background: var(--wb-surface-chrome, var(--surface-2)); }
  .repository-card.problem { border-color: color-mix(in srgb, var(--danger, #d64545) 50%, var(--border)); }
  .repository-state { gap: 8px; min-width: 0; }
  .repository-state > div { display: grid; min-width: 0; flex: 1; }
  .repository-state strong { color: var(--text-strong); font-size: 12px; }
  .repository-state small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .head-reference { display: inline-flex; align-items: center; gap: 5px; color: var(--wb-text-muted); font-size: 11px; font-weight: 700; }
  .head-reference code { padding: 3px 6px; border: 1px solid var(--wb-border-subtle); border-radius: 4px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 11px; }
  .state-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--warning, #d29a3a); }
  .state-dot.clean { background: var(--success, #3ea66b); }
  .diagnostic, .guard-message, .error-message { font-size: 12px; line-height: 1.45; }
  .diagnostic, .error-message { color: var(--danger, #d64545); }
  .guard-message { gap: 6px; color: var(--warning-strong, #a66a00); }
  .preview-banner, .preview-banner > div { display: flex; align-items: center; gap: 7px; }
  .preview-banner { justify-content: space-between; padding: 8px 9px; border-color: color-mix(in srgb, var(--wb-accent) 45%, var(--wb-border-subtle)); background: var(--wb-accent-soft); font-size: 12px; }
  .preview-banner button { min-height: 27px; padding: 4px 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-document); color: var(--wb-text-primary); font-size: 12px; }
  .setup-card { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 12px; padding: 18px; }
  .setup-icon { display: grid; width: 42px; height: 42px; place-items: center; border-radius: 8px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .setup-card strong { color: var(--text-strong); font-size: 13px; }
  .setup-card p { margin-top: 4px; color: var(--text-muted); font-size: 12px; line-height: 1.45; }
  .setup-card button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 32px; padding: 0 12px; border: 1px solid var(--wb-accent); border-radius: var(--wb-radius-control, 5px); color: #fff; background: var(--wb-accent); font-size: 12px; font-weight: 650; }
  .identity-card { padding: 10px; }
  summary { gap: 7px; color: var(--text-strong); cursor: pointer; font-size: 12px; font-weight: 700; }
  .identity-fields, .commit-card { display: grid; gap: 8px; }
  .identity-fields { grid-template-columns: 1fr 1fr; margin-top: 9px; }
  .identity-fields label, .commit-card label { display: grid; gap: 4px; color: var(--text-muted); font-size: 12px; }
  .identity-fields button { grid-column: 1 / -1; }
  input, textarea, select { width: 100%; border: 1px solid var(--wb-border-subtle); border-radius: var(--wb-radius-control, 5px); background: var(--wb-surface-document); color: var(--wb-text-primary); outline: none; }
  input { min-height: 31px; padding: 5px 7px; }
  textarea { padding: 7px; resize: vertical; }
  input:focus, textarea:focus, select:focus { border-color: var(--wb-accent); }
  .changes-section, .history-section, .diff-card { display: grid; gap: 7px; padding: 10px; }
  .section-heading > div { display: grid; gap: 1px; min-width: 0; }
  .section-heading span { color: var(--text-muted); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .section-heading > button:not(.mini-button), .identity-fields button { min-height: 28px; padding: 4px 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--wb-radius-control, 5px); background: var(--wb-surface-chrome); color: var(--wb-text-primary); font-size: 12px; }
  .file-list, .commit-list { display: grid; gap: 4px; }
  .file-row { gap: 5px; min-width: 0; }
  .file-row.conflict .file-main { border-color: var(--danger, #d64545); }
  .file-main { flex: 1; gap: 8px; min-width: 0; min-height: 31px; padding: 4px 7px; border: 1px solid transparent; border-radius: 5px; background: var(--wb-surface-chrome); color: var(--wb-text-primary); text-align: left; }
  .file-main:hover { border-color: var(--wb-border-subtle); background: var(--wb-control-hover); }
  .file-main b { width: 13px; color: var(--text-muted); font-size: 12px; }
  .file-main span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .commit-card { padding: 10px; }
  .primary-button { justify-content: center; gap: 7px; min-height: 34px; border: 1px solid var(--wb-accent); border-radius: var(--wb-radius-control, 5px); background: var(--wb-accent); color: #fff; }
  .empty-row, .empty-text { color: var(--text-muted); font-size: 12px; }
  .empty-row { justify-content: center; gap: 5px; padding: 9px; }
  .empty-text { margin: 18px 20px 0; padding: 28px 12px; text-align: center; }
  .diff-card pre { max-height: 330px; margin: 0; padding: 9px; overflow: auto; border-radius: 7px; background: #151917; color: #d8e2db; font: 12px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; white-space: pre; }
  .commit-row { gap: 6px; width: 100%; min-width: 0; padding: 4px 5px; border: 1px solid transparent; border-radius: 6px; background: transparent; color: var(--wb-text-primary); text-align: left; }
  .commit-row:hover { background: var(--wb-control-hover); }
  .commit-row.active-preview { border-color: var(--wb-accent); background: var(--wb-accent-soft); }
  .commit-main { display: flex; align-items: center; gap: 8px; min-width: 0; flex: 1; padding: 3px 2px; border: 0; background: transparent; color: var(--text); text-align: left; }
  .commit-graph { align-self: stretch; width: 2px; border-radius: 2px; background: var(--wb-accent); }
  .commit-content { display: grid; min-width: 0; flex: 1; gap: 2px; }
  .commit-content strong, .commit-content small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .commit-content strong { font-size: 12px; }
  .commit-content small, .commit-main code { color: var(--text-muted); font-size: 12px; }
  .load-more { justify-content: center; gap: 5px; min-height: 29px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-chrome); color: var(--wb-text-muted); font-size: 12px; }
  .restore-button { color: #d29a3a; }
  .restore-card { position: sticky; bottom: 4px; z-index: 2; display: grid; gap: 9px; padding: 11px; border-color: color-mix(in srgb, var(--warning, #d29a3a) 55%, var(--wb-border-subtle)); box-shadow: 0 -10px 30px rgba(0, 0, 0, .12); }
  .restore-card p { color: var(--text-muted); font-size: 12px; line-height: 1.45; }
  .restore-card label { display: grid; gap: 4px; color: var(--text-muted); font-size: 12px; }
  .restore-heading, .restore-actions { display: flex; align-items: center; justify-content: space-between; gap: 9px; }
  .restore-heading > div { display: grid; gap: 2px; min-width: 0; }
  .restore-heading strong { overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .restore-heading code { color: var(--warning-strong, #a66a00); font-size: 12px; }
  .restore-actions { justify-content: flex-end; }
  .restore-actions button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 31px; padding: 5px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-chrome); color: var(--wb-text-primary); font-size: 12px; }
  .restore-actions .danger-button { border-color: color-mix(in srgb, var(--warning, #d29a3a) 65%, var(--wb-border-subtle)); background: color-mix(in srgb, var(--warning, #d29a3a) 12%, var(--wb-surface-chrome)); }
  .recovery-section { display: grid; gap: 8px; padding: 10px; border-color: color-mix(in srgb, var(--warning, #d29a3a) 60%, var(--wb-border-subtle)); }
  .recovery-title, .recovery-meta, .recovery-actions { display: flex; align-items: center; gap: 7px; }
  .recovery-title > div { display: grid; gap: 1px; }
  .recovery-title small, .recovery-meta span { color: var(--text-muted); font-size: 12px; }
  .recovery-item { display: grid; gap: 6px; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-chrome); }
  .recovery-item.manual { border-color: color-mix(in srgb, var(--danger, #d64545) 55%, var(--border)); }
  .recovery-item p { color: var(--text-muted); font-size: 12px; line-height: 1.45; }
  .recovery-meta { justify-content: space-between; }
  .recovery-meta code { color: var(--warning-strong, #a66a00); font-size: 12px; }
  .recovery-meta span { text-transform: uppercase; }
  .recovery-actions { flex-wrap: wrap; justify-content: flex-end; }
  .recovery-actions button { min-height: 28px; padding: 4px 8px; border: 1px solid color-mix(in srgb, var(--warning, #d29a3a) 50%, var(--wb-border-subtle)); border-radius: 5px; background: color-mix(in srgb, var(--warning, #d29a3a) 9%, var(--wb-surface-chrome)); color: var(--wb-text-primary); font-size: 12px; }
  .network-progress { display: flex; align-items: center; justify-content: space-between; gap: 9px; padding: 9px; border-color: color-mix(in srgb, var(--wb-accent) 45%, var(--wb-border-subtle)); background: var(--wb-accent-soft); }
  .network-progress > div { display: grid; min-width: 0; gap: 2px; }
  .network-progress strong { font-size: 12px; }
  .network-progress small { max-height: 44px; overflow: hidden; color: var(--text-muted); font-size: 12px; white-space: pre-line; }
  .network-progress button { flex: 0 0 auto; min-height: 28px; padding: 4px 8px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-document); color: var(--wb-text-primary); font-size: 12px; }
  .integration-recovery { border-color: color-mix(in srgb, var(--wb-accent) 45%, var(--wb-border-subtle)); }
  .conflict-list { display: grid; gap: 3px; max-height: 120px; margin: 0; padding: 0 0 0 18px; overflow: auto; color: var(--danger, #d64545); font-size: 12px; }
  .remote-card, .branches-card { padding: 9px; }
  .card-hint { margin-top: 8px; color: var(--text-muted); font-size: 12px; line-height: 1.45; }
  .remote-list, .branch-list { display: grid; gap: 5px; margin-top: 8px; }
  .remote-row { display: grid; grid-template-columns: 1fr auto; gap: 5px; min-width: 0; }
  .remote-row.invalid { color: var(--danger, #d64545); }
  .remote-row > p { grid-column: 1 / -1; color: var(--danger, #d64545); font-size: 12px; line-height: 1.4; }
  .remote-main { display: grid; min-width: 0; padding: 6px 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-chrome); color: var(--wb-text-primary); text-align: left; }
  .remote-main:hover { background: var(--wb-control-hover); }
  .remote-main strong { font-size: 12px; }
  .remote-main small { overflow: hidden; color: var(--text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .remote-form { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; margin-top: 9px; }
  .remote-form label, .sync-selectors label, .integration-plan label, .destructive-confirmation label { display: grid; gap: 4px; color: var(--text-muted); font-size: 12px; }
  .span-2 { grid-column: 1 / -1; }
  .remote-form button, .branch-create button, .wide-button, .button-grid button, .destructive-confirmation button, .branch-row > button:not(.mini-button) { min-height: 29px; padding: 4px 8px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-chrome); color: var(--wb-text-primary); font-size: 12px; }
  .destructive-confirmation { display: grid; gap: 7px; margin-top: 9px; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger, #d64545) 50%, var(--wb-border-subtle)); border-radius: 6px; background: var(--wb-surface-chrome); }
  .destructive-confirmation p { color: var(--text-muted); font-size: 12px; line-height: 1.4; }
  .destructive-confirmation > div { display: flex; justify-content: flex-end; gap: 6px; }
  .destructive-confirmation .danger-button { border-color: color-mix(in srgb, var(--danger, #d64545) 60%, var(--border)); }
  .sync-card { display: grid; gap: 8px; padding: 9px; }
  .sync-badge { padding: 3px 6px; border-radius: 999px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 11px; font-weight: 650; text-transform: uppercase; }
  .sync-selectors { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; }
  select { min-height: 31px; padding: 5px 7px; }
  .sync-counters { display: grid; grid-template-columns: auto auto 1fr; gap: 6px; }
  .sync-counters span { min-width: 0; padding: 6px 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-chrome); color: var(--wb-text-muted); font-size: 11px; }
  .sync-counters b { display: block; overflow: hidden; color: var(--text); text-overflow: ellipsis; white-space: nowrap; }
  .button-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .wide-button { width: 100%; }
  .integration-plan { display: grid; gap: 7px; padding: 8px; border: 1px solid color-mix(in srgb, var(--wb-accent) 40%, var(--wb-border-subtle)); border-radius: 6px; background: var(--wb-surface-chrome); }
  .integration-plan > div:first-child { display: flex; justify-content: space-between; gap: 8px; }
  .integration-plan p { color: var(--text-muted); font-size: 12px; line-height: 1.45; }
  .comparison-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; color: var(--text-muted); font-size: 12px; }
  .comparison-grid span { padding: 5px 6px; border-radius: 6px; background: var(--surface-2); }
  .integration-history { display: grid; gap: 4px; max-height: 150px; padding: 7px; overflow: auto; border: 1px solid var(--border-3); border-radius: 6px; }
  .integration-history strong { margin-top: 3px; color: var(--text-muted); font-size: 12px; }
  .integration-history div { display: grid; grid-template-columns: auto 1fr; align-items: baseline; gap: 6px; min-width: 0; }
  .integration-history code { color: var(--brand); font-size: 12px; }
  .integration-history span { overflow: hidden; color: var(--text); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .integration-diff { border: 1px solid var(--border-3); border-radius: 6px; }
  .integration-diff summary { padding: 6px 7px; color: var(--text-muted); cursor: pointer; font-size: 12px; }
  .integration-diff pre { max-height: 280px; margin: 0; padding: 7px; overflow: auto; border-top: 1px solid var(--border-3); background: var(--surface-2); color: var(--text); font-size: 12px; line-height: 1.45; white-space: pre; }
  .integration-diff p { padding: 0 7px 7px; }
  .branch-create { display: grid; grid-template-columns: 1fr auto; gap: 6px; margin-top: 8px; }
  .branch-row { display: flex; align-items: center; gap: 6px; padding: 5px 6px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; background: var(--wb-surface-chrome); }
  .branch-row.current { border-color: color-mix(in srgb, var(--wb-accent) 45%, var(--wb-border-subtle)); background: var(--wb-accent-soft); }
  .branch-row > div { display: grid; min-width: 0; flex: 1; }
  .branch-row strong { overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .branch-row small { color: var(--text-muted); font-size: 12px; }
  .error-message { position: sticky; bottom: 0; margin: 10px 20px 0; padding: 9px; border: 1px solid color-mix(in srgb, var(--danger, #d64545) 55%, var(--wb-border-subtle)); border-radius: 7px; background: color-mix(in srgb, var(--danger, #d64545) 9%, var(--wb-surface-document)); }

  @media (max-width: 1120px) {
    .header-metrics div { min-width: 70px; }
    .header-metrics div:nth-child(-n + 2) { display: none; }
  }

  @media (max-width: 760px) {
    .versions-panel .panel-header { position: static; align-items: flex-start; min-height: 0; }
    .header-metrics { display: none; }
    .refresh-button { min-width: 32px; padding: 0 8px; font-size: 0; }
    .repository-card, .setup-card, .identity-card, .remote-card, .sync-card, .branches-card, .changes-section, .commit-card, .diff-card, .history-section, .preview-banner, .network-progress, .restore-card, .recovery-section { margin-right: 10px; margin-left: 10px; }
    .setup-card { grid-template-columns: auto minmax(0, 1fr); }
    .setup-card button { grid-column: 1 / -1; }
    .sync-selectors, .identity-fields, .remote-form { grid-template-columns: 1fr; }
    .span-2 { grid-column: auto; }
    .error-message, .empty-text { margin-right: 10px; margin-left: 10px; }
  }
</style>
