<script lang="ts">
  import {
    IconAlertTriangle,
    IconCircleCheck,
    IconCloudUpload,
    IconDeviceFloppy,
    IconHammer,
    IconRefresh,
    IconRocket,
    IconSettings,
    IconShieldCheck,
  } from "@tabler/icons-svelte";
  import DeployPane from "$lib/components/DeployPane.svelte";
  import type { AppState } from "$lib/state/app.svelte";

  let { app }: { app: AppState } = $props();

  type PublishView = "release" | "configuration";
  const views: { id: PublishView; label: string }[] = [
    { id: "release", label: "Pregătire publicare" },
    { id: "configuration", label: "Configurare" },
  ];

  let activeView = $state<PublishView>("release");
  let preflightRunning = $state(false);

  const audit = $derived(app.projectAuditSnapshot);
  const auditCurrent = $derived(Boolean(
    audit
    && audit.projectRoot === app.sessionProjectRoot
    && audit.runtimeSessionId === app.kernelProjectSessionId
    && audit.workspaceRevision === app.projectWorkspaceSnapshot?.revision,
  ));
  const auditErrors = $derived(auditCurrent ? (audit?.summary.errors ?? 0) : 0);
  const auditWarnings = $derived(auditCurrent ? (audit?.summary.warnings ?? 0) : 0);
  const validationValid = $derived(app.controlledPreview.validation === "valid");
  const sourceSaved = $derived(!app.globalDirtyState.dirty);
  const releaseReady = $derived(sourceSaved && auditCurrent && auditErrors === 0 && validationValid);

  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    void app.refreshProjectAudit();
  });

  function validationLabel() {
    switch (app.controlledPreview.validation) {
      case "valid": return "Construire Zola validă";
      case "invalid": return "Construire Zola invalidă";
      case "error": return "Validare indisponibilă";
      case "queued": return "Validare programată";
      case "running": return "Validare în curs";
      default: return "Construire nevalidată";
    }
  }

  async function runPreflight() {
    if (preflightRunning) return;
    preflightRunning = true;
    try {
      await app.runZolaValidation("manual");
      await app.refreshProjectAudit(true);
      app.setGlobalStatus("Verificarea înainte de publicare a fost actualizată.", "saved");
    } catch (error) {
      app.setGlobalStatus(
        `Verificarea nu a putut fi finalizată: ${error instanceof Error ? error.message : String(error)}`,
        "error",
      );
    } finally {
      preflightRunning = false;
    }
  }

  async function saveSession() {
    const saved = await app.saveActiveFile();
    if (saved) await app.refreshProjectAudit(true);
  }

  function selectView(view: PublishView) {
    activeView = view;
  }

  function handleTabKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + views.length) % views.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % views.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = views.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const next = views[nextIndex];
    if (!next) return;
    selectView(next.id);
    requestAnimationFrame(() => document.getElementById(`publish-tab-${next.id}`)?.focus());
  }
</script>

<section class="publish-workspace" aria-labelledby="publish-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconRocket size={15} stroke={1.9} /> Spațiu de publicare</span>
      <h1 id="publish-title">Publicare</h1>
      <p>Verificare, construire și publicare într-un singur flux. Sursele sunt salvate înainte ca rezultatul Zola să fie publicat.</p>
    </div>
    <div class="release-state" class:ready={releaseReady} class:blocked={!releaseReady}>
      {#if releaseReady}<IconCircleCheck size={19} stroke={1.9} />{:else}<IconAlertTriangle size={19} stroke={1.9} />{/if}
      <div><span>Stare publicare</span><strong>{releaseReady ? "Pregătit pentru construire" : "Necesită verificare"}</strong></div>
    </div>
  </header>

  <div class="publish-tabs" role="tablist" aria-label="Vizualizări Publicare">
    {#each views as view, index (view.id)}
      <button
        id={`publish-tab-${view.id}`}
        type="button"
        role="tab"
        aria-selected={activeView === view.id ? "true" : "false"}
        aria-controls={`publish-panel-${view.id}`}
        tabindex={activeView === view.id ? 0 : -1}
        class:active={activeView === view.id}
        onclick={() => selectView(view.id)}
        onkeydown={(event) => handleTabKeydown(event, index)}
      >
        {#if view.id === "configuration"}<IconSettings size={14} />{:else}<IconShieldCheck size={14} />{/if}
        {view.label}
      </button>
    {/each}
  </div>

  {#if activeView === "release"}
    <div id="publish-panel-release" class="release-panel" role="tabpanel" aria-labelledby="publish-tab-release">
      <section class="preflight-card" aria-labelledby="preflight-title">
        <header>
          <div><span>Praguri de calitate</span><h2 id="preflight-title">Verificare înainte de publicare</h2></div>
          <button type="button" disabled={preflightRunning} onclick={() => { void runPreflight(); }}>
            <IconRefresh class={preflightRunning ? "spin" : undefined} size={14} />
            {preflightRunning ? "Se verifică…" : "Rulează preflight"}
          </button>
        </header>
        <div class="gate-list">
          <article class:passed={sourceSaved} class:failed={!sourceSaved}>
            <span class="gate-icon">{#if sourceSaved}<IconCircleCheck size={17} />{:else}<IconAlertTriangle size={17} />{/if}</span>
            <div><strong>Surse salvate</strong><p>{sourceSaved ? "Sesiunea proiectului și discul sunt sincronizate." : `${app.globalDirtyState.areas.length} zone conțin modificări nepersistate.`}</p></div>
            {#if !sourceSaved}<button type="button" disabled={!app.globalDirtyState.canSave} onclick={() => { void saveSession(); }}><IconDeviceFloppy size={13} /> Salvează</button>{/if}
          </article>
          <article class:passed={auditCurrent && auditErrors === 0} class:failed={!auditCurrent || auditErrors > 0}>
            <span class="gate-icon">{#if auditCurrent && auditErrors === 0}<IconCircleCheck size={17} />{:else}<IconAlertTriangle size={17} />{/if}</span>
            <div><strong>Audit proiect</strong><p>{auditCurrent ? `${auditErrors} erori · ${auditWarnings} avertismente` : "Auditul nu corespunde reviziei curente."}</p></div>
            <button type="button" onclick={() => { void app.setWorkbenchActivity("audit"); }}>Deschide auditul</button>
          </article>
          <article class:passed={validationValid} class:failed={!validationValid}>
            <span class="gate-icon">{#if validationValid}<IconCircleCheck size={17} />{:else}<IconHammer size={17} />{/if}</span>
            <div><strong>{validationLabel()}</strong><p>{app.controlledPreview.validationMessage || "Rulează preflight pentru validarea proiectului cu Zola."}</p></div>
            <button type="button" disabled={preflightRunning} onclick={() => { void runPreflight(); }}>Verifică</button>
          </article>
          <article class="target-gate">
            <span class="gate-icon"><IconCloudUpload size={17} /></span>
            <div><strong>Țintă Bunny CDN</strong><p>Credentialele rămân în .env și sunt folosite numai de comanda Rust de deploy.</p></div>
            <button type="button" onclick={() => selectView("configuration")}>Configurează</button>
          </article>
        </div>
      </section>

      <aside class="release-actions" aria-labelledby="release-actions-title">
        <div class="release-copy">
          <span>Construire și publicare</span>
          <h2 id="release-actions-title">Livrează versiunea curentă</h2>
          <p>Construirea generează rezultatul local. Publicarea trimite rezultatul configurat către Bunny CDN și nu pornește automat.</p>
        </div>
        {#if !releaseReady}
          <div class="release-warning" role="status"><IconAlertTriangle size={15} /><span>Rezolvă pragurile înainte de publicare. Acțiunile rămân disponibile pentru verificări controlate.</span></div>
        {/if}
        <DeployPane
          scannedProject={!!app.scannedProject}
          isZola={app.scannedProject?.isZola ?? false}
          isEmpty={app.scannedProject?.isEmpty ?? false}
          cachebustAssets={app.cachebustAssets}
          projectRoot={app.sessionProjectRoot}
          runtimeSessionId={app.kernelProjectSessionId}
          workspaceMode
          actionsOnly
          onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind as import("$lib/types").SaveState)}
          onCachebustAssetsChange={(value) => { app.cachebustAssets = value; }}
        />
        <button class="output-link" type="button" onclick={() => { void app.setWorkbenchBottomPanel(true, "output"); }}>Deschide jurnalul</button>
      </aside>
    </div>
  {:else}
    <div id="publish-panel-configuration" class="configuration-panel" role="tabpanel" aria-labelledby="publish-tab-configuration">
      <header>
        <div><span>config.toml · .env · setări Pană</span><h2>Configurare build și destinație</h2><p>Setările sunt citite și scrise prin comenzile proiectului, fără o configurație paralelă în interfață.</p></div>
      </header>
      <div class="configuration-scroll">
        <DeployPane
          scannedProject={!!app.scannedProject}
          isZola={app.scannedProject?.isZola ?? false}
          isEmpty={app.scannedProject?.isEmpty ?? false}
          cachebustAssets={app.cachebustAssets}
          projectRoot={app.sessionProjectRoot}
          runtimeSessionId={app.kernelProjectSessionId}
          workspaceMode
          onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind as import("$lib/types").SaveState)}
          onCachebustAssetsChange={(value) => { app.cachebustAssets = value; }}
        />
      </div>
    </div>
  {/if}
</section>

<style>
  .publish-workspace { display: grid; grid-template-rows: auto 38px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-panel); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .workspace-header, .eyebrow, .release-state, .publish-tabs, .publish-tabs button, .preflight-card > header, .preflight-card > header button, .gate-list article, .gate-list article > button, .gate-icon, .release-warning, .output-link { display: flex; align-items: center; }
  .workspace-header { justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .eyebrow { gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 20px; font-weight: 650; letter-spacing: -.015em; }
  .workspace-header p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; }
  .release-state { min-width: 194px; gap: 9px; padding: 9px 11px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); background: var(--wb-surface-document); }
  .release-state.ready { border-color: color-mix(in srgb, var(--success) 48%, var(--wb-border-subtle)); color: var(--success); }
  .release-state.blocked { border-color: color-mix(in srgb, var(--wb-warning) 48%, var(--wb-border-subtle)); color: var(--wb-warning); }
  .release-state > div { display: grid; gap: 2px; }
  .release-state span, .release-copy > span, .configuration-panel > header span, .preflight-card > header span { color: var(--wb-text-muted); font-size: 12px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  .release-state strong { color: var(--text-strong); font-size: 12px; }
  .publish-tabs { gap: 2px; padding: 0 10px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .publish-tabs button { align-self: stretch; gap: 5px; padding: 0 11px; border: 0; border-bottom: 2px solid transparent; color: var(--wb-text-muted); background: transparent; font-size: 12px; font-weight: 600; }
  .publish-tabs button.active { border-bottom-color: var(--wb-accent); color: var(--wb-accent-strong); }
  .release-panel { display: grid; grid-template-columns: minmax(430px, 1fr) minmax(330px, .72fr); gap: 11px; min-width: 0; min-height: 0; padding: 11px; overflow: auto; }
  .preflight-card, .release-actions, .configuration-panel { min-width: 0; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-panel); background: var(--wb-surface-document); }
  .preflight-card { overflow: hidden; align-self: start; }
  .preflight-card > header { justify-content: space-between; gap: 10px; min-height: 58px; padding: 9px 11px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .preflight-card > header > div { display: grid; gap: 3px; }
  h2 { margin: 0; color: var(--text-strong); font-size: 14px; }
  .preflight-card > header button, .gate-list article > button, .output-link { justify-content: center; gap: 5px; min-height: 29px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; font-weight: 600; }
  .gate-list { display: grid; }
  .gate-list article { display: grid; grid-template-columns: 30px minmax(0, 1fr) auto; align-items: center; gap: 8px; min-height: 72px; padding: 10px 11px; border-bottom: 1px solid var(--wb-border-subtle); border-left: 3px solid var(--wb-border-strong); }
  .gate-list article:last-child { border-bottom: 0; }
  .gate-list article.passed { border-left-color: var(--success); }
  .gate-list article.failed { border-left-color: var(--wb-warning); }
  .gate-icon { width: 28px; height: 28px; justify-content: center; border-radius: 7px; color: var(--wb-text-muted); background: var(--surface-4); }
  article.passed .gate-icon { color: var(--success); background: color-mix(in srgb, var(--success) 9%, var(--wb-surface-document)); }
  article.failed .gate-icon { color: var(--wb-warning); background: color-mix(in srgb, var(--wb-warning) 9%, var(--wb-surface-document)); }
  .gate-list article > div { min-width: 0; }
  .gate-list strong { color: var(--text-strong); font-size: 12px; }
  .gate-list p { margin: 4px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.35; }
  .release-actions { align-self: start; padding: 15px; background: var(--wb-surface-chrome); }
  .release-copy { display: grid; gap: 5px; margin-bottom: 12px; }
  .release-copy h2 { font-size: 16px; }
  .release-copy p, .configuration-panel > header p { margin: 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  .release-warning { align-items: flex-start; gap: 7px; margin-bottom: 10px; padding: 8px; border: 1px solid color-mix(in srgb, var(--wb-warning) 40%, var(--wb-border-subtle)); border-radius: 7px; color: var(--wb-warning); background: color-mix(in srgb, var(--wb-warning) 7%, var(--wb-surface-document)); font-size: 12px; line-height: 1.4; }
  .output-link { width: 100%; margin-top: 9px; }
  .configuration-panel { display: grid; grid-template-rows: auto minmax(0, 1fr); min-height: 0; margin: 11px; overflow: hidden; }
  .configuration-panel > header { padding: 14px 16px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .configuration-panel > header > div { display: grid; gap: 4px; }
  .configuration-scroll { min-height: 0; padding: 12px 16px 24px; overflow: auto; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { cursor: default; opacity: .55; }
  button:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  :global(.spin) { animation: publish-spin .8s linear infinite; }
  @keyframes publish-spin { to { transform: rotate(360deg); } }
  @media (max-width: 980px) { .release-panel { grid-template-columns: 1fr; } .release-state { display: none; } }
</style>
