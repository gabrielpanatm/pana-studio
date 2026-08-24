<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import {
    IconCloudUpload,
    IconEye,
    IconEyeOff,
    IconPlugConnected,
    IconPlus,
    IconTrash,
    IconX,
  } from "@tabler/icons-svelte";
  import CheckboxControl from "$lib/components/ui/CheckboxControl.svelte";
  import EmptyState from "$lib/components/ui/EmptyState.svelte";
  import InlineMessage from "$lib/components/ui/InlineMessage.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import {
  cancelPublishOperation,
} from "$lib/deploy/io";
  import {
    deleteDeployCredential,
    executeDeploy,
    planDeploy,
    readDeployConfiguration,
    saveDeployCredential,
    saveDeploySettings,
    testDeployConnection,
  } from "$lib/deploy/io";
  import type {
    DeployCommandError,
    DeployConfigurationSnapshot,
    DeployCredentialKind,
    DeployCredentialWriteInput,
    DeployPlan,
    DeployProgressEvent,
    DeployProviderKind,
    DeployReceipt,
    DeploySettings,
    DeployTarget,
  } from "$lib/deploy/contracts";
  import type {
    PublishBuildReceipt,
    PublishPreflightReceipt,
  } from "$lib/deploy/contracts";
  import { errorMessage } from "$lib/util";

  let {
    scannedProject = false,
    mode = "configuration",
    projectRoot = "",
    runtimeSessionId = "",
    publishPreflight = null as PublishPreflightReceipt | null,
    publishBuild = null as PublishBuildReceipt | null,
    invalidatePublishAuthorization = () => {},
    disabled = false,
    onStatusUpdate = undefined as ((text: string, kind: string) => void) | undefined,
    onRunningChange = undefined as ((running: boolean) => void) | undefined,
  }: {
    scannedProject?: boolean;
    mode?: "release" | "configuration";
    projectRoot?: string;
    runtimeSessionId?: string;
    publishPreflight?: PublishPreflightReceipt | null;
    publishBuild?: PublishBuildReceipt | null;
    invalidatePublishAuthorization?: () => void;
    disabled?: boolean;
    onStatusUpdate?: (text: string, kind: string) => void;
    onRunningChange?: (running: boolean) => void;
  } = $props();

  const providerOptions: Array<{ value: DeployProviderKind; label: string }> = [
    { value: "bunny", label: "Bunny.net" },
    { value: "s3", label: "Amazon S3 / Cloudflare R2" },
    { value: "sftp", label: "SFTP" },
    { value: "ftp", label: "FTP / FTPS" },
    { value: "cloudflare_pages", label: "Cloudflare Pages" },
  ];

  let snapshot = $state<DeployConfigurationSnapshot | null>(null);
  let settings = $state<DeploySettings>({
    schemaVersion: 1,
    revision: 0,
    activeTargetId: null,
    targets: [],
  });
  let selectedTargetId = $state("");
  let loadedProjectKey = $state("");
  let loading = $state(false);
  let settingsDirty = $state(false);
  let savingSettings = $state(false);
  let credentialKind = $state<DeployCredentialKind>("bunny");
  let secretDraft = $state<Record<string, string>>({});
  let showSecrets = $state<Record<string, boolean>>({});
  let savingCredential = $state(false);
  let testingConnection = $state(false);
  let planning = $state(false);
  let deployRunning = $state(false);
  let cancelRunning = $state(false);
  let plan = $state<DeployPlan | null>(null);
  let receipt = $state<DeployReceipt | null>(null);
  let progress = $state<DeployProgressEvent | null>(null);
  let panelMessage = $state("");
  let panelError = $state(false);

  const selectedTarget = $derived(
    settings.targets.find((target) => target.id === selectedTargetId) ?? null,
  );
  const activeTarget = $derived(
    settings.targets.find((target) => target.id === settings.activeTargetId) ?? null,
  );
  const selectedCredentialStatus = $derived(
    selectedTarget ? credentialStatusForTarget(selectedTarget) : null,
  );
  const selectedCredentialConfigured = $derived(
    selectedCredentialStatus?.configured === true,
  );
  const selectedCapabilities = $derived(
    selectedTarget
      ? snapshot?.targetCapabilities.find((item) => item.targetId === selectedTarget.id)?.capabilities
      : null,
  );
  $effect(() => {
    if (
      plan
      && (
        !publishBuild
        || plan.preflightToken !== publishBuild.preflightToken
        || plan.buildToken !== publishBuild.buildToken
        || plan.artifactId !== publishBuild.artifactId
      )
    ) {
      plan = null;
      receipt = null;
    }
  });

  $effect(() => {
    const key = scannedProject ? projectRoot || "active-project" : "";
    if (!key) {
      loadedProjectKey = "";
      snapshot = null;
      return;
    }
    if (key === loadedProjectKey) return;
    loadedProjectKey = key;
    void loadConfiguration();
  });

  onMount(() => {
    let disposed = false;
    let unlisten: () => void = () => {};
    void listen<DeployProgressEvent>("deploy-progress", (event) => {
      if (!deployRunning || event.payload.targetId !== settings.activeTargetId) return;
      progress = event.payload;
    }).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten();
    };
  });

  async function loadConfiguration(preferredTargetId?: string) {
    loading = true;
    try {
      applySnapshot(await readDeployConfiguration(), preferredTargetId);
      panelMessage = "";
      panelError = false;
    } catch (error) {
      fail(`Configurația deploy nu poate fi citită: ${errorMessage(error)}`);
    } finally {
      loading = false;
    }
  }

  function applySnapshot(next: DeployConfigurationSnapshot, preferredTargetId?: string) {
    snapshot = next;
    settings = structuredClone(next.settings);
    const preferred = preferredTargetId || selectedTargetId || next.settings.activeTargetId || "";
    selectedTargetId = next.settings.targets.some((target) => target.id === preferred)
      ? preferred
      : next.settings.targets[0]?.id ?? "";
    settingsDirty = false;
    plan = null;
    receipt = null;
    syncCredentialKind();
  }

  function syncCredentialKind() {
    credentialKind = selectedTarget?.provider === "sftp"
      ? credentialStatusForTarget(selectedTarget)?.kind ?? "sftp_password"
      : defaultCredentialKind(selectedTarget?.provider ?? "bunny");
    secretDraft = {};
    showSecrets = {};
  }

  function credentialStatusForTarget(target: DeployTarget) {
    return snapshot?.credentialStatuses.find(
      (status) => status.credentialEnvPrefix === target.credentialEnvPrefix
        && credentialKindSupportsProvider(status.kind, target.provider),
    );
  }

  function credentialKindSupportsProvider(
    kind: DeployCredentialKind,
    provider: DeployProviderKind,
  ) {
    return kind === provider
      || provider === "sftp" && (kind === "sftp_password" || kind === "sftp_private_key");
  }

  function selectTarget(id: string) {
    selectedTargetId = id;
    plan = null;
    receipt = null;
    syncCredentialKind();
  }

  function markSettingsDirty() {
    settingsDirty = true;
    plan = null;
    receipt = null;
  }

  function updateTarget(patch: Partial<DeployTarget>) {
    settings = {
      ...settings,
      targets: settings.targets.map((target) =>
        target.id === selectedTargetId ? ({ ...target, ...patch } as DeployTarget) : target,
      ),
    };
    if (typeof patch.id === "string") {
      if (settings.activeTargetId === selectedTargetId) settings.activeTargetId = patch.id;
      selectedTargetId = patch.id;
    }
    markSettingsDirty();
  }

  function updateProviderConfig(patch: Record<string, unknown>) {
    if (!selectedTarget) return;
    updateTarget({
      config: { ...selectedTarget.config, ...patch },
    } as Partial<DeployTarget>);
  }

  function changeProvider(provider: DeployProviderKind) {
    if (!selectedTarget) return;
    const next = newTarget(provider, selectedTarget.id);
    updateTarget({
      provider,
      config: next.config,
      cleanupPolicy: next.cleanupPolicy,
      credentialEnvPrefix: selectedTarget.credentialEnvPrefix,
    } as Partial<DeployTarget>);
    syncCredentialKind();
  }

  function addTarget() {
    const id = uniqueTargetId("production");
    const target = newTarget("bunny", id);
    settings = {
      ...settings,
      activeTargetId: settings.activeTargetId ?? id,
      targets: [...settings.targets, target],
    };
    selectedTargetId = id;
    markSettingsDirty();
    syncCredentialKind();
  }

  function removeSelectedTarget() {
    if (!selectedTarget) return;
    if (selectedCredentialConfigured) {
      fail("Șterge mai întâi credentialele țintei; Pană Studio nu lasă secrete orfane în .env.");
      return;
    }
    const removedId = selectedTarget.id;
    const targets = settings.targets.filter((target) => target.id !== removedId);
    settings = {
      ...settings,
      targets,
      activeTargetId:
        settings.activeTargetId === removedId ? (targets[0]?.id ?? null) : settings.activeTargetId,
    };
    selectedTargetId = targets[0]?.id ?? "";
    markSettingsDirty();
    syncCredentialKind();
  }

  function makeActive() {
    if (!selectedTarget) return;
    settings = { ...settings, activeTargetId: selectedTarget.id };
    markSettingsDirty();
  }

  async function persistSettings() {
    if (!settingsDirty) return snapshot;
    savingSettings = true;
    try {
      const preferred = selectedTargetId;
      const next = await saveDeploySettings(settings);
      invalidatePublishAuthorization();
      applySnapshot(next, preferred);
      succeed("Configurația țintelor deploy a fost salvată.");
      return next;
    } catch (error) {
      fail(`Configurația țintelor nu a putut fi salvată: ${errorMessage(error)}`);
      return null;
    } finally {
      savingSettings = false;
    }
  }

  async function persistCredential() {
    if (!selectedTarget || settingsDirty) {
      fail("Salvează mai întâi configurația țintei.");
      return;
    }
    savingCredential = true;
    try {
      await saveDeployCredential(selectedTarget.id, credentialInput(selectedTarget));
      invalidatePublishAuthorization();
      await loadConfiguration(selectedTarget.id);
      secretDraft = {};
      succeed("Credentialele au fost salvate în .env din rădăcina proiectului.");
    } catch (error) {
      fail(`Credentialele nu au putut fi salvate: ${errorMessage(error)}`);
    } finally {
      savingCredential = false;
    }
  }

  async function removeCredential() {
    if (!selectedTarget || settingsDirty) return;
    try {
      await deleteDeployCredential(selectedTarget.credentialEnvPrefix);
      invalidatePublishAuthorization();
      await loadConfiguration(selectedTarget.id);
      succeed("Credentialele țintei au fost eliminate.");
    } catch (error) {
      fail(`Credentialele nu au putut fi eliminate: ${errorMessage(error)}`);
    }
  }

  async function runConnectionTest() {
    const target = mode === "release" ? activeTarget : selectedTarget;
    if (!target || settingsDirty) return;
    testingConnection = true;
    try {
      const result = await testDeployConnection(target.id);
      const inventory = result.observedRemoteObjects === undefined
        ? ""
        : ` Obiecte observate: ${result.observedRemoteObjects}.`;
      succeed(`Conexiunea ${providerLabel(result.provider)} este validă.${inventory}`);
    } catch (error) {
      fail(`Testul conexiunii a eșuat: ${errorMessage(error)}`);
    } finally {
      testingConnection = false;
    }
  }

  async function createPlan() {
    const target = activeTarget;
    const build = publishBuild;
    if (!target || !build || settingsDirty || disabled) return;
    planning = true;
    receipt = null;
    progress = null;
    try {
      plan = await planDeploy({
        targetId: target.id,
        expectedBuildToken: build.buildToken,
        expectedArtifactId: build.artifactId,
      });
      succeed(
        `Plan calculat: ${plan.uploadFiles} upload, ${plan.skippedFiles} neschimbate, ${plan.deleteFiles} ștergeri.`,
      );
    } catch (error) {
      plan = null;
      fail(`Planul deploy nu a putut fi calculat: ${errorMessage(error)}`);
    } finally {
      planning = false;
    }
  }

  async function executeCurrentPlan() {
    if (!plan || !activeTarget || settingsDirty || disabled) return;
    deployRunning = true;
    onRunningChange?.(true);
    receipt = null;
    progress = null;
    try {
      receipt = await executeDeploy({
        targetId: activeTarget.id,
        expectedSettingsRevision: plan.settingsRevision,
        expectedPlanToken: plan.planToken,
        expectedPreflightToken: plan.preflightToken,
        expectedBuildToken: plan.buildToken,
        expectedArtifactId: plan.artifactId,
      });
      succeed(`Deploy ${receipt.status}: ${receipt.uploadedFiles} upload, ${receipt.deletedFiles} ștergeri.`);
      plan = null;
    } catch (error) {
      const structured = deployCommandError(error);
      receipt = structured?.receipt ?? null;
      fail(`Deploy-ul a eșuat: ${structured?.message ?? errorMessage(error)}`);
    } finally {
      deployRunning = false;
      onRunningChange?.(false);
    }
  }

  async function cancelDeploy() {
    if (!deployRunning || cancelRunning || !projectRoot || !runtimeSessionId) return;
    cancelRunning = true;
    try {
      await cancelPublishOperation({
        expectedProjectRoot: projectRoot,
        expectedSessionId: runtimeSessionId,
      });
      succeed("Anularea deploy-ului a fost solicitată.");
    } catch (error) {
      fail(`Deploy-ul nu a putut fi anulat: ${errorMessage(error)}`);
    } finally {
      cancelRunning = false;
    }
  }

  function credentialInput(target: DeployTarget): DeployCredentialWriteInput {
    const credentialEnvPrefix = target.credentialEnvPrefix;
    switch (credentialKind) {
      case "bunny":
        return {
          credentialEnvPrefix,
          kind: "bunny",
          storageKey: secret("storageKey"),
          cdnApiKey: secret("cdnApiKey"),
        };
      case "ftp":
        return {
          credentialEnvPrefix,
          kind: "ftp",
          username: secret("username"),
          password: secret("password"),
        };
      case "sftp_password":
        return {
          credentialEnvPrefix,
          kind: "sftp_password",
          username: secret("username"),
          password: secret("password"),
        };
      case "sftp_private_key":
        return {
          credentialEnvPrefix,
          kind: "sftp_private_key",
          username: secret("username"),
          privateKeyPem: secret("privateKeyPem"),
          passphrase: optionalSecret("passphrase"),
        };
      case "s3":
        return {
          credentialEnvPrefix,
          kind: "s3",
          accessKeyId: secret("accessKeyId"),
          secretAccessKey: secret("secretAccessKey"),
          sessionToken: optionalSecret("sessionToken"),
        };
      case "cloudflare_pages":
        return { credentialEnvPrefix, kind: "cloudflare_pages", apiToken: secret("apiToken") };
    }
  }

  function secret(key: string) {
    return secretDraft[key] ?? "";
  }

  function optionalSecret(key: string) {
    const value = secret(key).trim();
    return value ? value : null;
  }

  function setSecret(key: string, value: string) {
    secretDraft = { ...secretDraft, [key]: value };
  }

  function toggleSecret(key: string) {
    showSecrets = { ...showSecrets, [key]: !showSecrets[key] };
  }

  function succeed(message: string) {
    panelMessage = message;
    panelError = false;
    onStatusUpdate?.(message, "saved");
  }

  function fail(message: string) {
    panelMessage = message;
    panelError = true;
    onStatusUpdate?.(message, "error");
  }

  function uniqueTargetId(base: string) {
    let candidate = base;
    let suffix = 2;
    while (settings.targets.some((target) => target.id === candidate)) {
      candidate = `${base}-${suffix}`;
      suffix += 1;
    }
    return candidate;
  }

  function defaultCredentialKind(provider: DeployProviderKind): DeployCredentialKind {
    switch (provider) {
      case "bunny": return "bunny";
      case "ftp": return "ftp";
      case "sftp": return "sftp_password";
      case "s3": return "s3";
      case "cloudflare_pages": return "cloudflare_pages";
    }
  }

  function providerLabel(provider: DeployProviderKind) {
    return providerOptions.find((option) => option.value === provider)?.label ?? provider;
  }

  function newTarget(provider: DeployProviderKind, id: string): DeployTarget {
    const common = {
      id,
      name: "Production",
      credentialEnvPrefix: envPrefixForTargetId(id),
      cleanupPolicy: "managed_only" as const,
    };
    switch (provider) {
      case "bunny":
        return { ...common, provider, config: { storageZone: "", storageRegion: "de", pullZoneId: "", remotePrefix: "" } };
      case "ftp":
        return { ...common, provider, config: { host: "", port: 21, remoteRoot: "/public_html", security: "ftps_explicit", allowInsecureFtp: false } };
      case "sftp":
        return { ...common, provider, config: { host: "", port: 22, remoteRoot: "/var/www/html", expectedHostKeySha256: "" } };
      case "s3":
        return { ...common, provider, config: { bucket: "", prefix: "", region: "us-east-1", endpoint: null, forcePathStyle: false, allowInsecureEndpoint: false, cacheControl: null } };
      case "cloudflare_pages":
        return { ...common, provider, config: { accountId: "", projectName: "", branch: null } };
    }
  }

  function envPrefixForTargetId(id: string) {
    const suffix = id
      .toUpperCase()
      .replace(/[^A-Z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "") || "TARGET";
    return `PANA_DEPLOY_${suffix}`;
  }

  function targetScopeIsRoot(target: DeployTarget) {
    switch (target.provider) {
      case "bunny": return target.config.remotePrefix === "";
      case "s3": return target.config.prefix === "";
      case "ftp":
      case "sftp": return target.config.remoteRoot === "/";
      case "cloudflare_pages": return false;
    }
  }

  function deleteActionLabel(action: DeployPlan["actions"][number]) {
    if (action.kind !== "delete") return action.kind;
    return action.deleteOrigin === "unmanaged" ? "delete extern" : "delete Pană";
  }

  function deployCommandError(error: unknown): DeployCommandError | null {
    let value = error;
    if (typeof value === "string") {
      try { value = JSON.parse(value); } catch { return null; }
    }
    if (!value || typeof value !== "object") return null;
    const candidate = value as Partial<DeployCommandError>;
    return typeof candidate.message === "string" && typeof candidate.code === "string"
      ? candidate as DeployCommandError
      : null;
  }

  function formatBytes(value: number) {
    if (value < 1024) return `${value} B`;
    if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
    return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
  }
</script>

{#if loading}
  <EmptyState compact title="Se încarcă țintele deploy…" />
{:else if mode === "release"}
  <section class="deploy-release-card" aria-label="Deploy target">
    <div class="release-target">
      <span>Țintă activă</span>
      <strong>{activeTarget?.name ?? "Nicio țintă configurată"}</strong>
      {#if activeTarget}<small>{providerLabel(activeTarget.provider)}</small>{/if}
    </div>
    <div class="release-buttons">
      <button type="button" class="ui-button compact" onclick={runConnectionTest} disabled={!activeTarget || settingsDirty || testingConnection || deployRunning || disabled}>
        <IconPlugConnected size={14} /> {testingConnection ? "Se testează…" : "Test conexiune"}
      </button>
      <button type="button" class="ui-button compact" onclick={createPlan} disabled={!activeTarget || !publishBuild || planning || deployRunning || disabled}>
        {planning ? "Se calculează…" : "Calculează planul"}
      </button>
      <button type="button" class="ui-button primary compact" onclick={executeCurrentPlan} disabled={!plan || deployRunning || disabled}>
        <IconCloudUpload size={14} /> {deployRunning ? "Se publică…" : "Execută deploy"}
      </button>
      {#if deployRunning}
        <button type="button" class="ui-button danger compact" onclick={cancelDeploy} disabled={cancelRunning}>
          <IconX size={14} /> {cancelRunning ? "Se anulează…" : "Anulează"}
        </button>
      {/if}
    </div>
  </section>
{:else}
  <section class="deploy-targets-section">
    <header class="section-header">
      <div>
        <h3>Ținte deploy</h3>
        <p>Configurația publică păstrează doar referințe; secretele sunt salvate în <code>.env</code> din rădăcina proiectului.</p>
      </div>
      <button type="button" class="ui-button compact" onclick={addTarget} disabled={deployRunning}>
        <IconPlus size={14} /> Adaugă țintă
      </button>
    </header>

    {#if settings.targets.length === 0}
      <EmptyState compact title="Nu există nicio țintă deploy" description="Adaugă Bunny, S3/R2, SFTP, FTP/FTPS sau Cloudflare Pages." />
    {:else}
      <div class="ui-tabs target-tabs" role="tablist" aria-label="Ținte deploy">
        {#each settings.targets as target (target.id)}
          <button type="button" class="ui-tab" class:active={target.id === selectedTargetId} role="tab" aria-selected={target.id === selectedTargetId} onclick={() => selectTarget(target.id)}>
            {target.id === settings.activeTargetId ? "● " : ""}{target.name}
          </button>
        {/each}
      </div>

      {#if selectedTarget}
        <div class="target-editor">
          <div class="field-grid">
            <label class="ui-form-field"><span class="ui-form-label">Nume</span><input class="ui-input compact" value={selectedTarget.name} oninput={(event) => updateTarget({ name: event.currentTarget.value })} /></label>
            <label class="ui-form-field"><span class="ui-form-label">ID stabil</span><input class="ui-input compact" value={selectedTarget.id} oninput={(event) => updateTarget({ id: event.currentTarget.value })} /></label>
            <div class="ui-form-field"><span class="ui-form-label">Provider</span><SelectControl value={selectedTarget.provider} options={providerOptions} ariaLabel="Provider deploy" onchange={(value) => changeProvider(value as DeployProviderKind)} /></div>
            <label class="ui-form-field"><span class="ui-form-label">Prefix ENV credentiale</span><input class="ui-input compact" value={selectedTarget.credentialEnvPrefix} oninput={(event) => updateTarget({ credentialEnvPrefix: event.currentTarget.value.toUpperCase() })} /></label>
          </div>

          {#if selectedTarget.provider === "bunny"}
            <div class="field-grid provider-fields">
              <label class="ui-form-field"><span class="ui-form-label">Storage zone</span><input class="ui-input compact" value={selectedTarget.config.storageZone} oninput={(event) => updateProviderConfig({ storageZone: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Storage region</span><input class="ui-input compact" value={selectedTarget.config.storageRegion} placeholder="de" oninput={(event) => updateProviderConfig({ storageRegion: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Pull zone ID</span><input class="ui-input compact" value={selectedTarget.config.pullZoneId} oninput={(event) => updateProviderConfig({ pullZoneId: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Remote prefix</span><input class="ui-input compact" value={selectedTarget.config.remotePrefix} placeholder="site/production" oninput={(event) => updateProviderConfig({ remotePrefix: event.currentTarget.value })} /></label>
            </div>
          {:else if selectedTarget.provider === "s3"}
            <div class="field-grid provider-fields">
              <label class="ui-form-field"><span class="ui-form-label">Bucket</span><input class="ui-input compact" value={selectedTarget.config.bucket} oninput={(event) => updateProviderConfig({ bucket: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Region</span><input class="ui-input compact" value={selectedTarget.config.region} placeholder="us-east-1 / auto" oninput={(event) => updateProviderConfig({ region: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Prefix</span><input class="ui-input compact" value={selectedTarget.config.prefix} placeholder="production" oninput={(event) => updateProviderConfig({ prefix: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Endpoint (R2/compatibil)</span><input class="ui-input compact" value={selectedTarget.config.endpoint ?? ""} placeholder="https://…r2.cloudflarestorage.com" oninput={(event) => updateProviderConfig({ endpoint: event.currentTarget.value || null })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Cache-Control</span><input class="ui-input compact" value={selectedTarget.config.cacheControl ?? ""} placeholder="public, max-age=3600" oninput={(event) => updateProviderConfig({ cacheControl: event.currentTarget.value || null })} /></label>
              <CheckboxControl compact label="Force path style" checked={selectedTarget.config.forcePathStyle} onchange={(checked) => updateProviderConfig({ forcePathStyle: checked })} />
              <CheckboxControl compact label="Permit endpoint HTTP" checked={selectedTarget.config.allowInsecureEndpoint} onchange={(checked) => updateProviderConfig({ allowInsecureEndpoint: checked })} />
            </div>
          {:else if selectedTarget.provider === "sftp"}
            <div class="field-grid provider-fields">
              <label class="ui-form-field"><span class="ui-form-label">Host</span><input class="ui-input compact" value={selectedTarget.config.host} oninput={(event) => updateProviderConfig({ host: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Port</span><input class="ui-input compact" type="number" min="1" max="65535" value={selectedTarget.config.port} oninput={(event) => updateProviderConfig({ port: Number(event.currentTarget.value) })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Remote root</span><input class="ui-input compact" value={selectedTarget.config.remoteRoot} oninput={(event) => updateProviderConfig({ remoteRoot: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Host key SHA-256</span><input class="ui-input compact" value={selectedTarget.config.expectedHostKeySha256} placeholder="SHA256:…" oninput={(event) => updateProviderConfig({ expectedHostKeySha256: event.currentTarget.value })} /></label>
            </div>
          {:else if selectedTarget.provider === "ftp"}
            <div class="field-grid provider-fields">
              <label class="ui-form-field"><span class="ui-form-label">Host</span><input class="ui-input compact" value={selectedTarget.config.host} oninput={(event) => updateProviderConfig({ host: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Port</span><input class="ui-input compact" type="number" min="1" max="65535" value={selectedTarget.config.port} oninput={(event) => updateProviderConfig({ port: Number(event.currentTarget.value) })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Remote root</span><input class="ui-input compact" value={selectedTarget.config.remoteRoot} oninput={(event) => updateProviderConfig({ remoteRoot: event.currentTarget.value })} /></label>
              <div class="ui-form-field"><span class="ui-form-label">Securitate</span><SelectControl value={selectedTarget.config.security} options={[{ value: "ftps_explicit", label: "FTPS explicit (TLS)" }, { value: "plain", label: "FTP necriptat" }]} ariaLabel="Securitatea conexiunii FTP" onchange={(value) => updateProviderConfig({ security: value })} /></div>
              {#if selectedTarget.config.security === "plain"}
                <div class="insecure"><CheckboxControl compact label="Confirm explicit FTP necriptat" checked={selectedTarget.config.allowInsecureFtp} onchange={(checked) => updateProviderConfig({ allowInsecureFtp: checked })} /></div>
              {/if}
            </div>
          {:else}
            <div class="field-grid provider-fields">
              <label class="ui-form-field"><span class="ui-form-label">Account ID</span><input class="ui-input compact" value={selectedTarget.config.accountId} oninput={(event) => updateProviderConfig({ accountId: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Project name</span><input class="ui-input compact" value={selectedTarget.config.projectName} oninput={(event) => updateProviderConfig({ projectName: event.currentTarget.value })} /></label>
              <label class="ui-form-field"><span class="ui-form-label">Branch (opțional)</span><input class="ui-input compact" value={selectedTarget.config.branch ?? ""} oninput={(event) => updateProviderConfig({ branch: event.currentTarget.value || null })} /></label>
            </div>
          {/if}

          {#if selectedTarget.provider !== "cloudflare_pages"}
            <div class:root-warning={selectedTarget.cleanupPolicy === "mirror_destination" && targetScopeIsRoot(selectedTarget)} class="cleanup-policy-card">
              <CheckboxControl compact label="Elimină din destinație fișierele care nu există în build" checked={selectedTarget.cleanupPolicy === "mirror_destination"} onchange={(checked) => updateTarget({ cleanupPolicy: checked ? "mirror_destination" : "managed_only" })} />
              <p>Transformă folderul remote într-o copie exactă a buildului curent. Pot fi șterse inclusiv fișiere încărcate manual sau de alte aplicații.</p>
              {#if selectedTarget.cleanupPolicy === "mirror_destination" && targetScopeIsRoot(selectedTarget)}
                <p class="warning">Atenție: oglindirea este activă pe rădăcina destinației.</p>
              {/if}
            </div>
          {/if}

          <div class="target-actions">
            <button type="button" class="ui-button compact" onclick={makeActive} disabled={selectedTarget.id === settings.activeTargetId}>Setează activă</button>
            <button type="button" class="ui-button primary compact" onclick={persistSettings} disabled={!settingsDirty || savingSettings}>{savingSettings ? "Se salvează…" : "Salvează țintele"}</button>
            <button type="button" class="ui-button danger compact" onclick={removeSelectedTarget} disabled={deployRunning}><IconTrash size={14} /> Elimină ținta</button>
          </div>

          <div class="capabilities">
            {#if selectedCapabilities?.deleteStale}<span>{selectedTarget.cleanupPolicy === "mirror_destination" ? "oglindire destinație" : "curățare administrată"}</span>{/if}
            {#if selectedCapabilities?.atomicActivation}<span>activare versionată</span>{/if}
            {#if selectedCapabilities?.cacheInvalidation}<span>cache purge</span>{/if}
            {#if selectedCapabilities?.metadataHeaders}<span>metadata</span>{/if}
          </div>

          <div class="credentials-card">
            <header>
              <div>
                <strong>Credentiale</strong>
                <small>
                  {selectedCredentialConfigured
                    ? "Configurate"
                    : selectedCredentialStatus?.missingFields.length
                      ? `Lipsesc: ${selectedCredentialStatus.missingFields.join(", ")}`
                      : "Lipsesc"}
                </small>
              </div>
              <code>{selectedTarget.credentialEnvPrefix}__*</code>
            </header>
            {#if selectedTarget.provider === "sftp"}
              <div class="ui-form-field"><span class="ui-form-label">Autentificare</span><SelectControl value={credentialKind} options={[{ value: "sftp_password", label: "Parolă" }, { value: "sftp_private_key", label: "Cheie privată" }]} ariaLabel="Autentificare SFTP" onchange={(value) => { credentialKind = value as DeployCredentialKind; secretDraft = {}; }} /></div>
            {/if}
            <div class="field-grid credential-fields">
              {#if credentialKind === "bunny"}
                {@render secretField("storageKey", "Storage key")}{@render secretField("cdnApiKey", "CDN API key")}
              {:else if credentialKind === "ftp" || credentialKind === "sftp_password"}
                {@render textSecretField("username", "Utilizator")}{@render secretField("password", "Parolă")}
              {:else if credentialKind === "sftp_private_key"}
                {@render textSecretField("username", "Utilizator")}
                <label class="ui-form-field wide"><span class="ui-form-label">Cheie privată PEM/OpenSSH</span><textarea class="ui-textarea code" rows="5" value={secret("privateKeyPem")} oninput={(event) => setSecret("privateKeyPem", event.currentTarget.value)} autocomplete="off"></textarea></label>
                {@render secretField("passphrase", "Passphrase (opțional)")}
              {:else if credentialKind === "s3"}
                {@render textSecretField("accessKeyId", "Access key ID")}{@render secretField("secretAccessKey", "Secret access key")}{@render secretField("sessionToken", "Session token (opțional)")}
              {:else}
                {@render secretField("apiToken", "Cloudflare API token")}
              {/if}
            </div>
            <p>Valorile existente nu sunt returnate în interfață. Sunt păstrate exclusiv în .env, sub prefixul țintei.</p>
            <div class="target-actions">
              <button type="button" class="ui-button primary compact" onclick={persistCredential} disabled={settingsDirty || savingCredential}>{savingCredential ? "Se salvează…" : "Salvează credentialele"}</button>
              <button type="button" class="ui-button compact" onclick={runConnectionTest} disabled={settingsDirty || !selectedCredentialConfigured || testingConnection}><IconPlugConnected size={14} /> {testingConnection ? "Se testează…" : "Test conexiune"}</button>
              {#if selectedCredentialConfigured}<button type="button" class="ui-button danger compact" onclick={removeCredential}>Șterge credentialele</button>{/if}
            </div>
          </div>
        </div>
      {/if}
    {/if}
  </section>

  <section class="deploy-execution-section">
    <header class="section-header">
      <div><h3>Plan și execuție</h3><p>Preflight: {publishPreflight?.status ?? "nerulat"} · Build: {publishBuild ? "autorizat" : "neautorizat"}. Deploy-ul rulează numai pe artifactul capturat de Rust.</p></div>
      <strong>{activeTarget?.name ?? "Nicio țintă activă"}</strong>
    </header>
    <div class="target-actions">
      <button type="button" class="ui-button compact" onclick={createPlan} disabled={!activeTarget || !publishBuild || settingsDirty || planning || deployRunning || disabled}>{planning ? "Se calculează…" : "Calculează planul"}</button>
      <button type="button" class="ui-button primary compact" onclick={executeCurrentPlan} disabled={!plan || deployRunning || disabled}><IconCloudUpload size={14} /> {deployRunning ? "Se publică…" : "Execută deploy"}</button>
      {#if deployRunning}<button type="button" class="ui-button danger compact" onclick={cancelDeploy} disabled={cancelRunning}><IconX size={14} /> {cancelRunning ? "Se anulează…" : "Anulează"}</button>{/if}
    </div>
  </section>
{/if}

{#if plan}
  <section class="result-card plan-card">
    <header><strong>Plan confirmabil</strong><code>{plan.provider}</code></header>
    <div class="metrics"><span><b>{plan.uploadFiles}</b> upload</span><span><b>{formatBytes(plan.uploadBytes)}</b></span><span><b>{plan.skippedFiles}</b> neschimbate</span><span><b>{plan.deleteFiles}</b> ștergeri</span><span><b>{plan.managedDeleteFiles}</b> fișiere Pană</span><span class:danger-metric={plan.unmanagedDeleteFiles > 0}><b>{plan.unmanagedDeleteFiles}</b> fișiere externe</span></div>
    {#if plan.warnings.length}{#each plan.warnings as warning}<p class="warning">{warning}</p>{/each}{/if}
    <details><summary>Operații ({plan.actions.length})</summary><ul>{#each plan.actions.slice(0, 100) as action}<li><code>{deleteActionLabel(action)}</code><span>{action.path}</span><small>{formatBytes(action.sizeBytes)}</small></li>{/each}</ul>{#if plan.actions.length > 100}<p>Primele 100 operații sunt afișate.</p>{/if}</details>
  </section>
{/if}

{#if progress}
  <section class="result-card progress-card" aria-live="polite">
    <header><strong>Progres: {progress.phase}</strong><span>{progress.completedFiles}/{progress.totalFiles}</span></header>
    <progress max={Math.max(progress.totalFiles, 1)} value={progress.completedFiles}></progress>
    {#if progress.currentPath}<code>{progress.currentPath}</code>{/if}
  </section>
{/if}

{#if receipt}
  <section class:partial={receipt.status === "partial"} class:failed={receipt.status === "failed"} class="result-card receipt-card">
    <header><strong>Receipt: {receipt.status}</strong><span>{providerLabel(receipt.provider)}</span></header>
    <div class="metrics"><span><b>{receipt.uploadedFiles}</b> upload</span><span><b>{receipt.deletedFiles}</b> ștergeri</span><span><b>{receipt.deletedManagedFiles}</b> fișiere Pană</span><span class:danger-metric={receipt.deletedUnmanagedFiles > 0}><b>{receipt.deletedUnmanagedFiles}</b> fișiere externe</span><span><b>{receipt.skippedFiles}</b> neschimbate</span>{#if receipt.provider === "bunny"}<span class:danger-metric={!receipt.cacheInvalidated}>{receipt.cacheInvalidated ? "Cache CDN invalidat" : "Cache CDN neinvalidat"}</span>{/if}</div>
    {#if receipt.deploymentId}<p>Deployment ID: <code>{receipt.deploymentId}</code></p>{/if}
    {#if receipt.deploymentUrl}<p>URL: <a href={receipt.deploymentUrl} target="_blank" rel="noreferrer">{receipt.deploymentUrl}</a></p>{/if}
    {#each receipt.warnings as warning}<p class="warning">{warning}</p>{/each}
  </section>
{/if}

{#if panelMessage}
  <InlineMessage message={panelMessage} tone={panelError ? "error" : "success"} />
{/if}

{#snippet textSecretField(key: string, label: string)}
  <label class="ui-form-field"><span class="ui-form-label">{label}</span><input class="ui-input compact" value={secret(key)} oninput={(event) => setSecret(key, event.currentTarget.value)} autocomplete="off" /></label>
{/snippet}

{#snippet secretField(key: string, label: string)}
  <label class="ui-form-field"><span class="ui-form-label">{label}</span><div class="secret-input"><input class="ui-input compact" type={showSecrets[key] ? "text" : "password"} value={secret(key)} oninput={(event) => setSecret(key, event.currentTarget.value)} autocomplete="new-password" /><button class="ui-icon-button compact" type="button" onclick={() => toggleSecret(key)} aria-label={showSecrets[key] ? "Ascunde secretul" : "Arată secretul"}>{#if showSecrets[key]}<IconEyeOff size={14} />{:else}<IconEye size={14} />{/if}</button></div></label>
{/snippet}

<style>
  .deploy-targets-section, .deploy-execution-section, .result-card, .deploy-release-card { display: grid; gap: 10px; padding: 10px; border: 1px solid var(--border-2); border-radius: 8px; background: color-mix(in srgb, var(--surface-4) 62%, transparent); }
  .section-header, .credentials-card > header, .result-card > header, .deploy-release-card { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .section-header h3 { margin: 0; color: var(--text-muted); font-size: 12px; font-weight: 900; letter-spacing: .08em; text-transform: uppercase; }
  .section-header p, .credentials-card p { margin: 3px 0 0; color: var(--text-muted); font-size: 12px; }
  .target-tabs { overflow-x: auto; }
  .target-tabs :global(.ui-tab) { flex: 0 0 auto; }
  .target-editor, .credentials-card { display: grid; gap: 10px; }
  .field-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  .provider-fields { padding-top: 10px; border-top: 1px solid var(--border-2); }
  .cleanup-policy-card { display: grid; gap: 6px; padding: 9px; border: 1px solid var(--border-3); border-radius: 7px; background: var(--surface-4); }
  .cleanup-policy-card.root-warning { border-color: #d97706; }
  .cleanup-policy-card p { margin: 0; color: var(--text-muted); font-size: 12px; }
  .cleanup-policy-card .warning { color: #b45309; font-weight: 700; }
  .provider-fields :global(.ui-checkbox) { align-self: end; min-height: 29px; }
  .insecure :global(.ui-checkbox) { border-color: #d97706; color: #b45309; }
  .target-actions, .release-buttons, .capabilities, .metrics { display: flex; align-items: center; flex-wrap: wrap; gap: 7px; }
  .capabilities span { padding: 3px 6px; border-radius: 999px; background: color-mix(in srgb, var(--brand) 10%, var(--surface-5)); color: var(--text-muted); font-size: 12px; }
  .credentials-card { padding: 10px; border: 1px solid var(--border-3); border-radius: 7px; background: color-mix(in srgb, var(--surface-5) 62%, transparent); }
  .credentials-card header > div, .release-target { display: grid; gap: 2px; }
  .credentials-card small, .release-target span, .release-target small { color: var(--text-muted); font-size: 12px; }
  .credentials-card code { overflow: hidden; color: var(--text-muted); font-size: 12px; text-overflow: ellipsis; }
  .wide { grid-column: 1 / -1; }
  .secret-input { display: grid; grid-template-columns: minmax(0, 1fr) 30px; }
  .secret-input :global(.ui-input) { border-radius: 7px 0 0 7px; }
  .secret-input :global(.ui-icon-button) { border-left: 0; border-radius: 0 7px 7px 0; }
  .deploy-execution-section { margin-top: 10px; }
  .release-target strong { color: var(--text); font-size: 13px; }
  .deploy-release-card { align-items: start; padding: 9px; }
  .result-card { margin-top: 10px; }
  .result-card.partial { border-color: #d97706; }
  .result-card.failed { border-color: var(--danger, #dc2626); }
  .metrics span { padding: 5px 7px; border: 1px solid var(--border-3); border-radius: 6px; color: var(--text-muted); font-size: 12px; }
  .metrics b { color: var(--text); }
  .metrics .danger-metric { border-color: color-mix(in srgb, var(--danger, #dc2626) 50%, var(--border-3)); color: var(--danger, #dc2626); }
  .warning { margin: 0; color: #b45309; font-size: 12px; }
  details { font-size: 12px; }
  details summary { cursor: pointer; color: var(--text-muted); }
  details ul { display: grid; gap: 3px; max-height: 230px; margin: 8px 0 0; padding: 0; overflow: auto; list-style: none; }
  details li { display: grid; grid-template-columns: 54px minmax(0, 1fr) auto; gap: 7px; padding: 4px 6px; border-radius: 5px; background: var(--surface-5); }
  details li span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  progress { width: 100%; accent-color: var(--brand); }
  @media (max-width: 720px) { .field-grid { grid-template-columns: 1fr; } .wide { grid-column: auto; } .section-header, .deploy-release-card { align-items: stretch; flex-direction: column; } }
</style>
