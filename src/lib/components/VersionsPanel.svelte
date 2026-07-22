<script lang="ts">
  import {
    IconAlertTriangle,
    IconCheck,
    IconChevronDown,
    IconGitBranch,
    IconGitCommit,
    IconEye,
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
  import { UI_TERMS } from "$lib/i18n/ui-terms";
  import type {
    ProjectWorkspaceSnapshot,
    SaveState,
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
    onStatusUpdate: (text: string, kind: SaveState) => void;
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
  let integrationMessage = $state("Integrare versiuni remote");
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
      ? "Salvează modificările editorului înainte de o operație Git."
      : recovery?.items.length
        ? "Rezolvă restaurarea pendentă înainte de alte operații Git."
        : integrationRecovery?.items.length
          ? "Rezolvă integrarea pendentă înainte de alte operații Git."
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
    if (!snapshot) throw new Error("Starea Git nu este încă disponibilă.");
    const identity = readIdentity();
    if (!identity) throw new Error("ProjectSession nu este disponibilă.");
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
      error = `${label}, dar actualizarea interfeței a eșuat: ${errorMessage(reason)} Efectul nu trebuie repetat automat; reîncarcă proiectul și verifică Recuperare.`;
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
    if (!integrationMessage.trim() || integrationMessage === "Integrare versiuni remote") {
      integrationMessage = selectedRemoteBranch
        ? `Integrare ${selectedRemote}/${selectedRemoteBranch}`
        : "Integrare versiuni remote";
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
        `${label} în backend`,
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
      error = "Mesajul versiunii este obligatoriu.";
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
        "Commit-ul Git a fost publicat în backend",
        () => refreshHistory(true),
      ))) return;
      diff = null;
      integrationPlan = null;
      integrationDiff = null;
      const diagnostic = receipt.diagnostic ? ` ${receipt.diagnostic}` : "";
      onStatusUpdate(
        `Versiunea ${receipt.commitOid.slice(0, 8)} a fost creată.${diagnostic}`,
        receipt.publicationStatus === "published" ? "saved" : "error",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`Commit Git blocat: ${error}`, "error");
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
        `Previzualizezi versiunea ${receipt.shortOid}; sursele curente nu au fost modificate.`,
        "saved",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`Preview versiune blocat: ${error}`, "error");
    } finally {
      busyAction = "";
    }
  }

  function requestRestore(entry: VersionHistoryEntry) {
    if (entry.oid === snapshot?.headOid) {
      error = "Aceasta este deja versiunea curentă.";
      return;
    }
    restoreEntry = entry;
    restoreMessage = `Restaurare versiune ${entry.shortOid}: ${entry.subject}`;
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
      error = "Restaurarea cere un repository Git complet curat.";
      return;
    }
    if (workspaceDirty) {
      error = mutationBlockedReason;
      return;
    }
    if (!restoreMessage.trim()) {
      error = "Mesajul commit-ului de restaurare este obligatoriu.";
      return;
    }
    if (restoreConfirmation.trim() !== entry.shortOid) {
      error = `Confirmarea trebuie să fie exact ${entry.shortOid}.`;
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
        "Restaurarea Git a ajuns la o stare terminală în backend",
        () => afterRestore(receipt),
      ))) return;
      if (receipt.status === "recovery_required") {
        await refresh();
        error = receipt.diagnostic ?? "Restaurarea cere recuperare explicită.";
        onStatusUpdate(error, "error");
        return;
      }
      await refresh();
      cancelRestore();
      const diagnostic = receipt.diagnostic ? ` ${receipt.diagnostic}` : "";
      onStatusUpdate(
        receipt.status === "noop"
          ? `Versiunea ${entry.shortOid} are deja același conținut.${diagnostic}`
          : `Versiunea ${entry.shortOid} a fost restaurată printr-un commit nou.${diagnostic}`,
        "restored",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`Restaurare versiune blocată: ${error}`, "error");
    } finally {
      busyAction = "";
    }
  }

  function recoveryActionLabel(action: VersionRestoreRecoveryAction) {
    if (action === "finalize") return "Finalizează restaurarea";
    if (action === "rollback") return "Revino la starea anterioară";
    return "Curăță marker-ul";
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
        "Recovery-ul restaurării a produs un rezultat în backend",
        () => afterRecovery(receipt),
      ))) return;
      await refresh();
      if (!receipt.resolved) {
        error = receipt.diagnostic ?? "Recovery-ul Git nu s-a încheiat.";
        onStatusUpdate(error, "error");
        return;
      }
      onStatusUpdate(
        receipt.diagnostic ?? `Recovery Git rezolvat: ${recoveryActionLabel(action)}.`,
        "restored",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`Recovery Git blocat: ${error}`, "error");
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
      error = "Numele și URL-ul fetch sunt obligatorii.";
      return;
    }
    await runSnapshotMutation("Remote Git salvat", () => configureVersionRemote(
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
      error = "Confirmarea eliminării remote-ului nu corespunde.";
      return;
    }
    const name = pendingRemoteRemoval;
    await runSnapshotMutation(`Remote ${name} eliminat`, () => removeVersionRemote(
      mutationIdentity(),
      name,
    ));
    pendingRemoteRemoval = "";
    remoteRemovalConfirmation = "";
  }

  async function fetchRemote() {
    if (!selectedRemote) {
      error = "Alege un remote utilizabil.";
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
      message: "Fetch pornește…",
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
        `Fetch ${selectedRemote} s-a încheiat în backend`,
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
          ? `Fetch ${selectedRemote} a actualizat referințele remote.`
          : `Fetch ${selectedRemote}: nicio referință nouă.`,
        "saved",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`Fetch blocat: ${error}`, "error");
    } finally {
      busyAction = "";
    }
  }

  async function pushBranch() {
    if (!snapshot?.branch || !selectedRemote) {
      error = "Push cere un branch local și un remote selectat.";
      return;
    }
    const remoteBranch = selectedRemoteBranch || snapshot.branch;
    const operationId = networkOperationId("push");
    activeNetwork = {
      schemaVersion: 2,
      projectRoot,
      sessionId,
      operationId,
      kind: "push",
      status: "started",
      message: "Push pornește…",
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
        `Branch-ul ${snapshot.branch} a fost publicat în ${selectedRemote}/${remoteBranch}.`,
        "saved",
      );
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`Push blocat: ${error}`, "error");
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
      error = "Alege branch-ul remote care va fi upstream.";
      return;
    }
    await runSnapshotMutation("Upstream Git configurat", () => configureVersionUpstream(
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
    await runSnapshotMutation("Upstream Git eliminat", () => clearVersionUpstream(
      mutationIdentity(),
      snapshot!.branch!,
    ));
  }

  async function createBranch() {
    const name = newBranchName.trim();
    if (!name) {
      error = "Numele branch-ului este obligatoriu.";
      return;
    }
    await runSnapshotMutation(`Branch ${name} creat`, () => createVersionBranch(
      mutationIdentity(),
      name,
    ));
    newBranchName = "";
  }

  async function switchBranch(branch: string, oid: string | null) {
    if (!oid || !snapshot?.clean) {
      error = "Schimbarea branch-ului cere un repository complet curat.";
      return;
    }
    busyAction = `switch:${branch}`;
    error = "";
    try {
      const receipt = await switchVersionBranch(mutationIdentity(), branch, oid);
      if (receipt.snapshot) snapshot = receipt.snapshot;
      if (!(await settlePublishedEffect(
        `Branch-ul ${branch} a fost schimbat în backend`,
        () => afterIntegration(receipt),
      ))) return;
      await refresh();
      onStatusUpdate(`Branch activ: ${branch}.`, "restored");
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`Schimbarea branch-ului a fost blocată: ${error}`, "error");
    } finally {
      busyAction = "";
    }
  }

  async function deleteBranch(branch: string) {
    if (branchRemovalConfirmation !== branch) {
      error = `Scrie exact ${branch} pentru a confirma ștergerea.`;
      return;
    }
    await runSnapshotMutation(`Branch ${branch} eliminat`, () => deleteVersionBranch(
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
      error = "Alege un branch remote-tracking pentru integrare.";
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
      integrationMessage = `Integrare ${target.remote}/${target.name}`;
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
        "Integrarea Git a produs un rezultat în backend",
        () => afterIntegration(receipt),
      ))) return;
      await refresh();
      if (receipt.status === "conflict_resolution_required") {
        onStatusUpdate(
          `Merge-ul cere rezolvarea a ${receipt.conflictPaths.length} fișier(e).`,
          "error",
        );
      } else if (receipt.status === "recovery_required") {
        error = receipt.diagnostic ?? "Integrarea cere recuperare explicită.";
        onStatusUpdate(error, "error");
      } else {
        onStatusUpdate(
          receipt.status === "noop" ? "Ținta este deja integrată." : "Integrarea Git a fost publicată.",
          "restored",
        );
      }
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`Integrare Git blocată: ${error}`, "error");
    } finally {
      busyAction = "";
    }
  }

  function integrationRecoveryActionLabel(action: VersionIntegrationRecoveryAction) {
    if (action === "finalize") return "Finalizează integrarea";
    if (action === "continue") return "Continuă merge-ul";
    if (action === "rollback") return "Anulează și revino";
    return "Curăță marker-ul";
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
        "Recovery-ul integrării a produs un rezultat în backend",
        () => afterIntegrationRecovery(receipt),
      ))) return;
      await refresh();
      if (!receipt.resolved) {
        error = receipt.diagnostic ?? "Integrarea necesită încă recuperare.";
        onStatusUpdate(error, "error");
      } else {
        onStatusUpdate(
          receipt.diagnostic ?? `Recovery integrare: ${integrationRecoveryActionLabel(action)}.`,
          "restored",
        );
      }
    } catch (reason) {
      error = errorMessage(reason);
      onStatusUpdate(`Recovery integrare blocat: ${error}`, "error");
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

  function formatDate(value: string) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return new Intl.DateTimeFormat("ro-RO", {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(date);
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

<section class="versions-panel" aria-label={UI_TERMS.versionControl}>
    <header class="panel-header">
      <div class="title-block">
        <span class="title-icon"><IconGitBranch size={20} stroke={1.8} /></span>
        <div>
          <span class="eyebrow">Repository · sursa/</span>
          <h1>{UI_TERMS.versionControl}</h1>
          <p>Modificări, versiuni, ramuri și sincronizare Git într-un singur flux.</p>
        </div>
      </div>
      <div class="header-actions">
        <button type="button" class="refresh-button" disabled={loading || !!busyAction} onclick={() => refresh()}>
          <IconRefresh size={16} stroke={1.9} /> Actualizează
        </button>
      </div>
    </header>

    {#if loading && !snapshot}
      <p class="empty-text">Se citește repository-ul Git…</p>
    {:else if !projectRoot || !sessionId}
      <p class="empty-text">Deschide un proiect pentru versionare.</p>
    {:else if snapshot}
      <section class="repository-card" class:problem={snapshot.repositoryState !== "ready" && snapshot.repositoryState !== "uninitialized"}>
        <div class="repository-state">
          <span class="state-dot" class:clean={snapshot.repositoryState === "ready" && snapshot.clean}></span>
          <div>
            <strong>{snapshot.repositoryState === "ready" ? (snapshot.branch ?? "detached HEAD") : snapshot.repositoryState}</strong>
            <small title={snapshot.repositoryRoot}>{snapshot.repositoryRoot}</small>
          </div>
          {#if snapshot.headOid}<code>{snapshot.headOid.slice(0, 8)}</code>{/if}
        </div>
        <div class="status-grid">
          <span>Editor <b class:warning={workspaceDirty}>{workspaceDirty ? "nesalvat" : "salvat"}</b></span>
          <span>Git <b>{snapshot.repositoryState === "ready" ? (snapshot.clean ? "curat" : "modificat") : "neinițializat"}</b></span>
          <span>Pregătite <b>{snapshot.stagedCount}</b></span>
          <span>Nepregătite <b>{snapshot.unstagedCount}</b></span>
        </div>
        {#if snapshot.diagnostic}<p class="diagnostic">{snapshot.diagnostic}</p>{/if}
        {#if mutationBlockedReason}<p class="guard-message"><IconAlertTriangle size={14} /> {mutationBlockedReason}</p>{/if}
      </section>

      {#if activePreviewCommitOid}
        <section class="preview-banner">
          <div><IconEye size={15} /><span>Previzualizare izolată <code>{activePreviewCommitOid.slice(0, 8)}</code></span></div>
          <button type="button" onclick={returnToLivePreview}>Revino la versiunea curentă</button>
        </section>
      {/if}

      {#if activeNetwork}
        <section class="network-progress" aria-live="polite">
          <div>
            <strong>{activeNetwork.kind.toUpperCase()} · {activeNetwork.status.replaceAll("_", " ")}</strong>
            <small>{activeNetwork.message}</small>
          </div>
          {#if activeNetwork.status === "started" || activeNetwork.status === "progress"}
            <button type="button" onclick={cancelNetwork}>Anulează</button>
          {/if}
        </section>
      {/if}

      {#if recovery?.items.length}
        <section class="recovery-section" aria-label="Restaurări Git întrerupte">
          <div class="recovery-title"><IconAlertTriangle size={16} /><div><strong>Recuperarea restaurării</strong><small>{recovery.items.length} tranzacție(i) pendinte</small></div></div>
          {#each recovery.items as item (item.recoveryRef)}
            <article class="recovery-item" class:manual={item.state === "manual_review"}>
              <div class="recovery-meta"><code>{item.targetCommitOid.slice(0, 8)}</code><span>{item.state.replaceAll("_", " ")}</span></div>
              <p>{item.diagnostic}</p>
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
        <section class="recovery-section integration-recovery" aria-label="Integrări Git active sau întrerupte">
          <div class="recovery-title"><IconAlertTriangle size={16} /><div><strong>Integrare Git</strong><small>{integrationRecovery.items.length} tranzacție(i) activă(e)</small></div></div>
          {#each integrationRecovery.items as item (item.recoveryRef)}
            <article class="recovery-item" class:manual={item.state === "manual_review"}>
              <div class="recovery-meta"><code>{item.targetOid.slice(0, 8)}</code><span>{item.kind.replaceAll("_", " ")} · {item.state.replaceAll("_", " ")}</span></div>
              <p>{item.diagnostic}</p>
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
          <IconGitCommit size={20} stroke={1.8} />
          <div>
            <strong>Versionarea nu este inițializată</strong>
            <p>Repository-ul va fi creat strict în <code>sursa/</code>. Restul proiectului rămâne în afara Git.</p>
          </div>
          <button type="button" disabled={!!busyAction || workspaceDirty} onclick={() => runSnapshotMutation("Repository Git inițializat", () => initializeVersioning(mutationIdentity()))}>
            Inițializează Git
          </button>
        </section>
      {:else if snapshot.repositoryState === "ready"}
        <details class="identity-card" open={!snapshot.userName || !snapshot.userEmail}>
          <summary><IconSettings size={15} stroke={1.8} /> Identitate locală</summary>
          <div class="identity-fields">
            <label>Nume<input bind:value={identityName} autocomplete="name" /></label>
            <label>Email<input type="email" bind:value={identityEmail} autocomplete="email" /></label>
            <button type="button" disabled={!!busyAction || workspaceDirty || !identityName.trim() || !identityEmail.trim()} onclick={() => runSnapshotMutation("Identitatea Git a fost salvată", () => configureVersioningIdentity(mutationIdentity(), { name: identityName, email: identityEmail }))}>
              Salvează identitatea
            </button>
          </div>
        </details>

        <details class="remote-card" open={snapshot.remotes.length === 0}>
          <summary><IconSettings size={15} stroke={1.8} /> Remote-uri și autentificare</summary>
          <p class="card-hint">Secretele nu sunt salvate în proiect. HTTPS folosește credential helper-ul Git, iar SSH folosește cheia și agentul sistemului.</p>
          {#if snapshot.remotes.length}
            <div class="remote-list">
              {#each snapshot.remotes as remote (remote.name)}
                <article class="remote-row" class:invalid={!remote.usable}>
                  <button type="button" class="remote-main" onclick={() => editRemote(remote.name)}>
                    <strong>{remote.name}</strong>
                    <small title={remote.fetchUrl}>{remote.fetchUrl}</small>
                  </button>
                  <button type="button" class="mini-button" title="Elimină remote" disabled={!!busyAction || !!mutationBlockedReason} onclick={() => { pendingRemoteRemoval = remote.name; remoteRemovalConfirmation = ""; }}>×</button>
                  {#if remote.diagnostic}<p>{remote.diagnostic}</p>{/if}
                </article>
              {/each}
            </div>
          {/if}
          <div class="remote-form">
            <label>Nume<input bind:value={remoteName} placeholder="origin" autocomplete="off" /></label>
            <label class="span-2">URL fetch<input bind:value={remoteFetchUrl} placeholder="https://github.com/organizatie/site.git" autocomplete="off" spellcheck="false" /></label>
            <label class="span-2">URL push separat (opțional)<input bind:value={remotePushUrl} placeholder="ssh://git@github.com/organizatie/site.git" autocomplete="off" spellcheck="false" /></label>
            <button type="button" class="span-2" disabled={!!busyAction || !!mutationBlockedReason || !remoteName.trim() || !remoteFetchUrl.trim()} onclick={saveRemote}>Salvează remote</button>
          </div>
          {#if pendingRemoteRemoval}
            <div class="destructive-confirmation">
              <p>Eliminarea șterge configurația și referințele remote-tracking locale, nu repository-ul de pe server.</p>
              <label>Scrie <code>{pendingRemoteRemoval}</code><input bind:value={remoteRemovalConfirmation} autocomplete="off" /></label>
              <div><button type="button" onclick={() => { pendingRemoteRemoval = ""; }}>Renunță</button><button type="button" class="danger-button" disabled={remoteRemovalConfirmation !== pendingRemoteRemoval} onclick={removeRemoteConfirmed}>Elimină</button></div>
            </div>
          {/if}
        </details>

        {#if snapshot.remotes.length}
          <section class="sync-card">
            <div class="section-heading">
              <div><p class="section-label">Sincronizare remote</p><span>Fetch → analiză → fast-forward/merge explicit</span></div>
              <span class="sync-badge">{snapshot.syncState.replaceAll("_", " ")}</span>
            </div>
            <div class="sync-selectors">
              <label>Remote
                <select bind:value={selectedRemote} onchange={() => { selectedRemoteBranch = snapshot?.remoteBranches.find((branch) => branch.remote === selectedRemote)?.name ?? ""; integrationPlan = null; integrationDiff = null; }}>
                  <option value="">Alege remote</option>
                  {#each usableRemotes as remote}<option value={remote.name}>{remote.name}</option>{/each}
                </select>
              </label>
              <label>Branch remote
                <select bind:value={selectedRemoteBranch} onchange={() => { integrationPlan = null; integrationDiff = null; }}>
                  <option value="">Alege branch</option>
                  {#each selectedRemoteBranches as branch}<option value={branch.name}>{branch.name}</option>{/each}
                </select>
              </label>
            </div>
            <div class="sync-counters">
              <span>Ahead <b>{snapshot.upstream?.ahead ?? 0}</b></span>
              <span>Behind <b>{snapshot.upstream?.behind ?? 0}</b></span>
              <span>Upstream <b>{snapshot.upstream ? `${snapshot.upstream.remote}/${snapshot.upstream.remoteBranch}` : "neconfigurat"}</b></span>
            </div>
            <div class="button-grid">
              <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !selectedRemote} onclick={fetchRemote}>Fetch + prune</button>
              <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.branch || !selectedRemote} onclick={pushBranch}>Push sigur</button>
              <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.branch || !selectedRemoteBranch} onclick={saveUpstream}>Setează upstream</button>
              <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.upstream} onclick={removeUpstream}>Șterge upstream</button>
            </div>
            <p class="card-hint">Pană Studio nu rulează <code>git pull</code>. După Fetch, ținta este analizată și integrarea este aleasă explicit.</p>
            <button type="button" class="wide-button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.clean || !selectedTarget()} onclick={analyzeIntegration}>Analizează integrarea</button>
            {#if integrationPlan}
              <article class="integration-plan">
                <div><strong>{integrationPlan.relationship.replaceAll("_", " ")}</strong><code>{integrationPlan.targetOid.slice(0, 8)}</code></div>
                <p>{integrationPlan.diagnostic}</p>
                <div class="comparison-grid">
                  <span>Doar local <b>{integrationPlan.ahead}</b></span>
                  <span>De integrat <b>{integrationPlan.behind}</b></span>
                </div>
                {#if integrationPlan.localOnly.length || integrationPlan.targetOnly.length}
                  <div class="integration-history">
                    {#if integrationPlan.targetOnly.length}
                      <strong>Commit-uri care intră din țintă</strong>
                      {#each integrationPlan.targetOnly as entry (entry.oid)}
                        <div><code>{entry.shortOid}</code><span>{entry.subject}</span></div>
                      {/each}
                    {/if}
                    {#if integrationPlan.localOnly.length}
                      <strong>Commit-uri păstrate numai local</strong>
                      {#each integrationPlan.localOnly as entry (entry.oid)}
                        <div><code>{entry.shortOid}</code><span>{entry.subject}</span></div>
                      {/each}
                    {/if}
                  </div>
                {/if}
                {#if integrationDiff}
                  <details class="integration-diff">
                    <summary>Previzualizare patch din țintă{integrationDiff.truncated ? " (trunchiat)" : ""}</summary>
                    {#if integrationDiff.binary}
                      <p>Previzualizarea include fișiere binare; conținutul lor nu este afișat textual.</p>
                    {:else if integrationDiff.patch}
                      <pre>{integrationDiff.patch}{integrationDiff.truncated ? "\n\n… diff trunchiat la limita de siguranță" : ""}</pre>
                    {:else}
                      <p>Ținta nu aduce diferențe de fișiere față de baza comună.</p>
                    {/if}
                  </details>
                {/if}
                <label>Mesaj merge<textarea rows="2" bind:value={integrationMessage}></textarea></label>
                <div class="button-grid">
                  <button type="button" class="primary-button" disabled={!integrationPlan.fastForwardAllowed || !!busyAction} onclick={() => applyIntegration("fast_forward")}>Fast-forward</button>
                  <button type="button" class="primary-button" disabled={!integrationPlan.mergeAllowed || !!busyAction || !integrationMessage.trim()} onclick={() => applyIntegration("merge")}>Merge explicit</button>
                </div>
              </article>
            {/if}
          </section>
        {/if}

        <details class="branches-card">
          <summary><IconGitCommit size={15} stroke={1.8} /> Branch-uri locale</summary>
          <div class="branch-create">
            <input bind:value={newBranchName} placeholder="feature/pagina-noua" autocomplete="off" spellcheck="false" />
            <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.headOid || !newBranchName.trim()} onclick={createBranch}>Creează</button>
          </div>
          <div class="branch-list">
            {#each snapshot.branches as branch (branch.name)}
              <article class="branch-row" class:current={branch.current}>
                <div><strong>{branch.name}</strong><small>{branch.current ? "activ" : branch.syncState.replaceAll("_", " ")}</small></div>
                {#if !branch.current}
                  <button type="button" disabled={!!busyAction || !!mutationBlockedReason || !snapshot.clean || !branch.oid} onclick={() => switchBranch(branch.name, branch.oid)}>Deschide</button>
                  <button type="button" class="mini-button" title="Șterge dacă este integrat" disabled={!!busyAction || !!mutationBlockedReason} onclick={() => { pendingBranchRemoval = branch.name; branchRemovalConfirmation = ""; }}>×</button>
                {/if}
              </article>
            {/each}
          </div>
          {#if pendingBranchRemoval}
            <div class="destructive-confirmation">
              <p>Branch-ul poate fi șters numai dacă toate commit-urile sale sunt deja integrate în HEAD. Scrie exact <code>{pendingBranchRemoval}</code> pentru confirmare.</p>
              <label>Confirmare<input bind:value={branchRemovalConfirmation} autocomplete="off" spellcheck="false" /></label>
              <div>
                <button type="button" onclick={() => { pendingBranchRemoval = ""; branchRemovalConfirmation = ""; }}>Renunță</button>
                <button type="button" class="danger-button" disabled={!!busyAction || branchRemovalConfirmation !== pendingBranchRemoval} onclick={() => deleteBranch(pendingBranchRemoval)}>Șterge branch</button>
              </div>
            </div>
          {/if}
        </details>

        <section class="changes-section">
          <div class="section-heading">
            <div><p class="section-label">Staged</p><span>{stagedFiles.length} fișier(e)</span></div>
            <button type="button" disabled={!!busyAction || workspaceDirty || stagedFiles.length === 0} onclick={() => runFileMutation("Indexul Git a fost golit", () => unstageAllVersioning(mutationIdentity()))}>Unstage toate</button>
          </div>
          {#if stagedFiles.length === 0}
            <p class="empty-row">Nicio modificare pregătită.</p>
          {:else}
            <div class="file-list">
              {#each stagedFiles as file (`staged:${file.path}`)}
                <article class:conflict={file.conflicted} class="file-row">
                  <button type="button" class="file-main" title="Arată diff staged" onclick={() => showFileDiff(file, "staged")}>
                    <b>{kindLabel(file)}</b><span>{file.path}</span>
                  </button>
                  <button type="button" class="mini-button" title="Unstage" disabled={!!busyAction || workspaceDirty} onclick={() => runFileMutation(`Scos din staged: ${file.path}`, () => unstageVersioningPaths(mutationIdentity(), [file.path]))}>−</button>
                </article>
              {/each}
            </div>
          {/if}
        </section>

        <section class="commit-card">
          <label for="version-message">Mesajul versiunii</label>
          <textarea id="version-message" rows="3" bind:value={commitMessage} placeholder="Ex.: Finalizare pagină Despre noi"></textarea>
          <button type="button" class="primary-button" disabled={!!busyAction || workspaceDirty || stagedFiles.length === 0 || snapshot.conflictedCount > 0 || !snapshot.userName || !snapshot.userEmail || !commitMessage.trim()} onclick={commit}>
            <IconGitCommit size={16} stroke={1.9} /> Creează versiunea
          </button>
        </section>

        <section class="changes-section">
          <div class="section-heading">
            <div><p class="section-label">Modificări</p><span>{unstagedFiles.length} fișier(e)</span></div>
            <button type="button" disabled={!!busyAction || workspaceDirty || unstagedFiles.length === 0} onclick={() => runFileMutation("Toate modificările au fost pregătite", () => stageAllVersioning(mutationIdentity()))}>Stage toate</button>
          </div>
          {#if unstagedFiles.length === 0}
            <p class="empty-row"><IconCheck size={14} /> Arborele de lucru nu are modificări.</p>
          {:else}
            <div class="file-list">
              {#each unstagedFiles as file (`unstaged:${file.path}`)}
                <article class:conflict={file.conflicted} class="file-row">
                  <button type="button" class="file-main" title="Arată diff" onclick={() => showFileDiff(file, "unstaged")}>
                    <b>{kindLabel(file)}</b><span>{file.path}</span>
                  </button>
                  <button type="button" class="mini-button" title="Stage" disabled={!!busyAction || workspaceDirty} onclick={() => runFileMutation(`Pregătit: ${file.path}`, () => stageVersioningPaths(mutationIdentity(), [file.path]))}><IconPlus size={13} /></button>
                </article>
              {/each}
            </div>
          {/if}
        </section>

        {#if diff}
          <section class="diff-card">
            <div class="section-heading">
              <div><p class="section-label">Diff {diff.kind}</p><span>{diff.path ?? diff.commitOid?.slice(0, 8) ?? "versiune"}</span></div>
              <button type="button" class="mini-button" title="Închide diff" onclick={() => { diff = null; }}><IconX size={13} /></button>
            </div>
            {#if diff.binary}
              <p class="empty-row">Fișier binar — conținutul nu este afișat.</p>
            {:else if !diff.patch}
              <p class="empty-row">Git nu a produs un diff textual pentru această selecție.</p>
            {:else}
              <pre>{diff.patch}{diff.truncated ? "\n\n… diff trunchiat la limita de siguranță" : ""}</pre>
            {/if}
          </section>
        {/if}

        <section class="history-section">
          <div class="section-heading">
            <div><p class="section-label">Istoric Git</p><span>{history.length} versiune(i) încărcate</span></div>
          </div>
          {#if history.length === 0}
            <p class="empty-row">Primul commit va apărea aici.</p>
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
                  <button type="button" class="mini-button" title="Previzualizează această versiune" disabled={!!busyAction} onclick={() => previewCommit(entry)}><IconEye size={14} /></button>
                  <button type="button" class="mini-button restore-button" title="Restaurează această versiune" disabled={!!busyAction || workspaceDirty || !snapshot.clean || entry.oid === snapshot.headOid} onclick={() => requestRestore(entry)}><IconRestore size={14} /></button>
                </article>
              {/each}
            </div>
            {#if historyHasMore}
              <button type="button" class="load-more" disabled={!!busyAction} onclick={() => refreshHistory(false)}><IconChevronDown size={15} /> Încarcă versiuni mai vechi</button>
            {/if}
          {/if}
        </section>

        {#if restoreEntry}
          <section class="restore-card" aria-label="Confirmare restaurare versiune">
            <div class="restore-heading">
              <div>
                <p class="section-label">Restaurare sigură</p>
                <strong>{restoreEntry.subject}</strong>
              </div>
              <code>{restoreEntry.shortOid}</code>
            </div>
            <p>Fișierele din <code>sursa/</code> vor reveni la această versiune. Istoricul nu este rescris: rezultatul devine un commit nou, copil al versiunii curente.</p>
            <label>Mesajul commit-ului<textarea rows="3" bind:value={restoreMessage}></textarea></label>
            <label>Scrie <code>{restoreEntry.shortOid}</code> pentru confirmare<input bind:value={restoreConfirmation} autocomplete="off" spellcheck="false" /></label>
            <div class="restore-actions">
              <button type="button" disabled={!!busyAction} onclick={cancelRestore}>Renunță</button>
              <button type="button" class="danger-button" disabled={!!busyAction || restoreConfirmation.trim() !== restoreEntry.shortOid || !restoreMessage.trim()} onclick={restoreCommit}><IconRestore size={15} /> Restaurează prin commit nou</button>
            </div>
          </section>
        {/if}
      {/if}
    {/if}

    {#if error}<p class="error-message" role="alert">{error}</p>{/if}
</section>

<style>
  .versions-panel { position: relative; display: flex; flex-direction: column; gap: 11px; width: min(100%, 1120px); height: 100%; margin: 0 auto; padding: 18px 20px 30px; overflow-y: auto; border-right: 1px solid var(--wb-border-subtle, var(--border)); border-left: 1px solid var(--wb-border-subtle, var(--border)); background: var(--wb-surface-document, var(--surface)); color: var(--wb-text-primary, var(--text)); }
  .versions-panel .panel-header { position: sticky; top: -18px; z-index: 3; min-height: 76px; margin: -18px -20px 3px; padding: 12px 20px; border-bottom: 1px solid var(--wb-border-subtle, var(--border)); background: color-mix(in srgb, var(--wb-surface-chrome, var(--surface)) 94%, transparent); backdrop-filter: blur(12px); }
  .panel-header, .title-block, .header-actions, .repository-state, .section-heading, .file-row, .file-main, .commit-row, .guard-message, summary, .primary-button, .load-more, .empty-row { display: flex; align-items: center; }
  .panel-header, .section-heading { justify-content: space-between; gap: 10px; }
  .title-block { gap: 12px; min-width: 0; }
  .title-block > div { min-width: 0; }
  .title-icon { display: grid; flex: 0 0 auto; width: 40px; height: 40px; place-items: center; border-radius: 10px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .panel-header h1, .eyebrow, .section-label, p { margin: 0; }
  .panel-header h1 { margin-top: 2px; color: var(--text-strong); font-size: 24px; line-height: 1.15; }
  .title-block p { margin-top: 4px; color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; }
  .eyebrow, .section-label { color: var(--text-muted); font-size: 12px; font-weight: 850; letter-spacing: .09em; text-transform: uppercase; }
  .header-actions { gap: 6px; }
  button, input, textarea, select { font: inherit; }
  button { cursor: pointer; }
  button:disabled { cursor: default; opacity: .45; }
  .mini-button { display: inline-flex; align-items: center; justify-content: center; padding: 0; border: 1px solid var(--border-3); border-radius: 7px; background: var(--surface-3); color: var(--text-muted); }
  .refresh-button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 32px; padding: 0 11px; border: 1px solid var(--wb-border-subtle, var(--border)); border-radius: var(--wb-radius-control, 7px); color: var(--wb-text-primary, var(--text)); background: var(--wb-surface-document, var(--surface)); font-size: 12px; font-weight: 750; }
  .mini-button { flex: 0 0 27px; width: 27px; height: 27px; }
  .repository-card, .setup-card, .identity-card, .remote-card, .sync-card, .branches-card, .changes-section, .commit-card, .diff-card, .history-section, .preview-banner, .network-progress, .restore-card, .recovery-section { border: 1px solid var(--border-3); border-radius: 9px; background: var(--surface-2); }
  .repository-card { display: grid; gap: 9px; padding: 10px; }
  .repository-card.problem { border-color: color-mix(in srgb, var(--danger, #d64545) 50%, var(--border)); }
  .repository-state { gap: 8px; min-width: 0; }
  .repository-state > div { display: grid; min-width: 0; flex: 1; }
  .repository-state small { color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .repository-state code { font-size: 12px; color: var(--text-muted); }
  .state-dot { width: 9px; height: 9px; border-radius: 50%; background: #d29a3a; }
  .state-dot.clean { background: #3ea66b; }
  .status-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 5px 10px; }
  .status-grid span { color: var(--text-muted); font-size: 12px; }
  .status-grid b { color: var(--text); }
  .status-grid b.warning { color: #d29a3a; }
  .diagnostic, .guard-message, .error-message { font-size: 12px; line-height: 1.45; }
  .diagnostic, .error-message { color: var(--danger, #d64545); }
  .guard-message { gap: 6px; color: #d29a3a; }
  .preview-banner, .preview-banner > div { display: flex; align-items: center; gap: 7px; }
  .preview-banner { justify-content: space-between; padding: 8px 9px; border-color: color-mix(in srgb, var(--brand) 55%, var(--border)); font-size: 12px; }
  .preview-banner button { min-height: 27px; padding: 4px 7px; border: 1px solid var(--border-3); border-radius: 6px; background: var(--surface-3); color: var(--text); font-size: 12px; }
  .setup-card { display: grid; grid-template-columns: auto 1fr; gap: 10px; padding: 12px; }
  .setup-card p { margin-top: 4px; color: var(--text-muted); font-size: 12px; line-height: 1.45; }
  .setup-card button { grid-column: 1 / -1; min-height: 32px; }
  .identity-card { padding: 9px; }
  summary { gap: 7px; cursor: pointer; font-size: 12px; font-weight: 750; }
  .identity-fields, .commit-card { display: grid; gap: 8px; }
  .identity-fields { grid-template-columns: 1fr 1fr; margin-top: 9px; }
  .identity-fields label, .commit-card label { display: grid; gap: 4px; color: var(--text-muted); font-size: 12px; }
  .identity-fields button { grid-column: 1 / -1; }
  input, textarea, select { width: 100%; border: 1px solid var(--border-3); border-radius: 7px; background: var(--surface); color: var(--text); outline: none; }
  input { min-height: 31px; padding: 5px 7px; }
  textarea { padding: 7px; resize: vertical; }
  input:focus, textarea:focus, select:focus { border-color: var(--brand); }
  .changes-section, .history-section, .diff-card { display: grid; gap: 7px; padding: 9px; }
  .section-heading > div { display: grid; gap: 1px; min-width: 0; }
  .section-heading span { color: var(--text-muted); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .section-heading > button:not(.mini-button), .identity-fields button, .setup-card button { min-height: 28px; padding: 4px 8px; border: 1px solid var(--border-3); border-radius: 6px; background: var(--surface-3); color: var(--text); font-size: 12px; }
  .file-list, .commit-list { display: grid; gap: 4px; }
  .file-row { gap: 5px; min-width: 0; }
  .file-row.conflict .file-main { border-color: var(--danger, #d64545); }
  .file-main { flex: 1; gap: 8px; min-width: 0; min-height: 29px; padding: 4px 7px; border: 1px solid transparent; border-radius: 6px; background: var(--surface-3); color: var(--text); text-align: left; }
  .file-main b { width: 13px; color: var(--text-muted); font-size: 12px; }
  .file-main span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .commit-card { padding: 9px; }
  .primary-button { justify-content: center; gap: 7px; min-height: 34px; border: 1px solid color-mix(in srgb, var(--brand) 70%, var(--border)); border-radius: 7px; background: color-mix(in srgb, var(--brand) 18%, var(--surface-3)); color: var(--text-strong); }
  .empty-row, .empty-text { color: var(--text-muted); font-size: 12px; }
  .empty-row { justify-content: center; gap: 5px; padding: 9px; }
  .empty-text { padding: 15px 5px; text-align: center; }
  .diff-card pre { max-height: 330px; margin: 0; padding: 9px; overflow: auto; border-radius: 7px; background: #151917; color: #d8e2db; font: 12px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; white-space: pre; }
  .commit-row { gap: 6px; width: 100%; min-width: 0; padding: 4px 5px; border: 1px solid transparent; border-radius: 7px; background: transparent; color: var(--text); text-align: left; }
  .commit-row:hover { background: var(--surface-3); }
  .commit-row.active-preview { border-color: var(--brand); }
  .commit-main { display: flex; align-items: center; gap: 8px; min-width: 0; flex: 1; padding: 3px 2px; border: 0; background: transparent; color: var(--text); text-align: left; }
  .commit-graph { align-self: stretch; width: 2px; border-radius: 2px; background: var(--brand); }
  .commit-content { display: grid; min-width: 0; flex: 1; gap: 2px; }
  .commit-content strong, .commit-content small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .commit-content strong { font-size: 12px; }
  .commit-content small, .commit-main code { color: var(--text-muted); font-size: 12px; }
  .load-more { justify-content: center; gap: 5px; min-height: 29px; border: 1px solid var(--border-3); border-radius: 7px; background: var(--surface-3); color: var(--text-muted); font-size: 12px; }
  .restore-button { color: #d29a3a; }
  .restore-card { position: sticky; bottom: 4px; z-index: 2; display: grid; gap: 9px; padding: 11px; border-color: color-mix(in srgb, #d29a3a 60%, var(--border)); box-shadow: 0 -10px 30px rgba(0, 0, 0, .18); }
  .restore-card p { color: var(--text-muted); font-size: 12px; line-height: 1.45; }
  .restore-card label { display: grid; gap: 4px; color: var(--text-muted); font-size: 12px; }
  .restore-heading, .restore-actions { display: flex; align-items: center; justify-content: space-between; gap: 9px; }
  .restore-heading > div { display: grid; gap: 2px; min-width: 0; }
  .restore-heading strong { overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .restore-heading code { color: #d29a3a; font-size: 12px; }
  .restore-actions { justify-content: flex-end; }
  .restore-actions button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 31px; padding: 5px 9px; border: 1px solid var(--border-3); border-radius: 7px; background: var(--surface-3); color: var(--text); font-size: 12px; }
  .restore-actions .danger-button { border-color: color-mix(in srgb, #d29a3a 70%, var(--border)); background: color-mix(in srgb, #d29a3a 14%, var(--surface-3)); }
  .recovery-section { display: grid; gap: 8px; padding: 10px; border-color: color-mix(in srgb, #d29a3a 65%, var(--border)); }
  .recovery-title, .recovery-meta, .recovery-actions { display: flex; align-items: center; gap: 7px; }
  .recovery-title > div { display: grid; gap: 1px; }
  .recovery-title small, .recovery-meta span { color: var(--text-muted); font-size: 12px; }
  .recovery-item { display: grid; gap: 6px; padding: 8px; border: 1px solid var(--border-3); border-radius: 7px; background: var(--surface); }
  .recovery-item.manual { border-color: color-mix(in srgb, var(--danger, #d64545) 55%, var(--border)); }
  .recovery-item p { color: var(--text-muted); font-size: 12px; line-height: 1.45; }
  .recovery-meta { justify-content: space-between; }
  .recovery-meta code { color: #d29a3a; font-size: 12px; }
  .recovery-meta span { text-transform: uppercase; }
  .recovery-actions { flex-wrap: wrap; justify-content: flex-end; }
  .recovery-actions button { min-height: 28px; padding: 4px 8px; border: 1px solid color-mix(in srgb, #d29a3a 55%, var(--border)); border-radius: 6px; background: color-mix(in srgb, #d29a3a 10%, var(--surface-3)); color: var(--text); font-size: 12px; }
  .network-progress { display: flex; align-items: center; justify-content: space-between; gap: 9px; padding: 9px; border-color: color-mix(in srgb, var(--brand) 55%, var(--border)); }
  .network-progress > div { display: grid; min-width: 0; gap: 2px; }
  .network-progress strong { font-size: 12px; }
  .network-progress small { max-height: 44px; overflow: hidden; color: var(--text-muted); font-size: 12px; white-space: pre-line; }
  .network-progress button { flex: 0 0 auto; min-height: 28px; padding: 4px 8px; border: 1px solid var(--border-3); border-radius: 6px; background: var(--surface-3); color: var(--text); font-size: 12px; }
  .integration-recovery { border-color: color-mix(in srgb, var(--brand) 55%, var(--border)); }
  .conflict-list { display: grid; gap: 3px; max-height: 120px; margin: 0; padding: 0 0 0 18px; overflow: auto; color: var(--danger, #d64545); font-size: 12px; }
  .remote-card, .branches-card { padding: 9px; }
  .card-hint { margin-top: 8px; color: var(--text-muted); font-size: 12px; line-height: 1.45; }
  .remote-list, .branch-list { display: grid; gap: 5px; margin-top: 8px; }
  .remote-row { display: grid; grid-template-columns: 1fr auto; gap: 5px; min-width: 0; }
  .remote-row.invalid { color: var(--danger, #d64545); }
  .remote-row > p { grid-column: 1 / -1; color: var(--danger, #d64545); font-size: 12px; line-height: 1.4; }
  .remote-main { display: grid; min-width: 0; padding: 6px 7px; border: 1px solid var(--border-3); border-radius: 7px; background: var(--surface); color: var(--text); text-align: left; }
  .remote-main strong { font-size: 12px; }
  .remote-main small { overflow: hidden; color: var(--text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .remote-form { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; margin-top: 9px; }
  .remote-form label, .sync-selectors label, .integration-plan label, .destructive-confirmation label { display: grid; gap: 4px; color: var(--text-muted); font-size: 12px; }
  .span-2 { grid-column: 1 / -1; }
  .remote-form button, .branch-create button, .wide-button, .button-grid button, .destructive-confirmation button, .branch-row > button:not(.mini-button) { min-height: 29px; padding: 4px 8px; border: 1px solid var(--border-3); border-radius: 6px; background: var(--surface-3); color: var(--text); font-size: 12px; }
  .destructive-confirmation { display: grid; gap: 7px; margin-top: 9px; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger, #d64545) 50%, var(--border)); border-radius: 7px; background: var(--surface); }
  .destructive-confirmation p { color: var(--text-muted); font-size: 12px; line-height: 1.4; }
  .destructive-confirmation > div { display: flex; justify-content: flex-end; gap: 6px; }
  .destructive-confirmation .danger-button { border-color: color-mix(in srgb, var(--danger, #d64545) 60%, var(--border)); }
  .sync-card { display: grid; gap: 8px; padding: 9px; }
  .sync-badge { padding: 3px 6px; border-radius: 999px; background: var(--surface-3); color: var(--text-muted); font-size: 12px; text-transform: uppercase; }
  .sync-selectors { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; }
  select { min-height: 31px; padding: 5px 7px; }
  .sync-counters { display: grid; grid-template-columns: auto auto 1fr; gap: 6px; }
  .sync-counters span { min-width: 0; padding: 5px 6px; border-radius: 6px; background: var(--surface); color: var(--text-muted); font-size: 12px; }
  .sync-counters b { display: block; overflow: hidden; color: var(--text); text-overflow: ellipsis; white-space: nowrap; }
  .button-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .wide-button { width: 100%; }
  .integration-plan { display: grid; gap: 7px; padding: 8px; border: 1px solid color-mix(in srgb, var(--brand) 45%, var(--border)); border-radius: 7px; background: var(--surface); }
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
  .branch-row { display: flex; align-items: center; gap: 6px; padding: 5px 6px; border: 1px solid var(--border-3); border-radius: 7px; background: var(--surface); }
  .branch-row.current { border-color: color-mix(in srgb, var(--brand) 55%, var(--border)); }
  .branch-row > div { display: grid; min-width: 0; flex: 1; }
  .branch-row strong { overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .branch-row small { color: var(--text-muted); font-size: 12px; }
  .error-message { position: sticky; bottom: 0; padding: 9px; border: 1px solid color-mix(in srgb, var(--danger, #d64545) 55%, var(--border)); border-radius: 8px; background: color-mix(in srgb, var(--danger, #d64545) 10%, var(--surface)); }
</style>
