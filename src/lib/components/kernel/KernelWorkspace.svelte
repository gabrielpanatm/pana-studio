<script lang="ts">
  import { IconActivity, IconGitBranch, IconShieldCheck } from "@tabler/icons-svelte";
  import DiskConflictControl from "$lib/components/kernel/DiskConflictControl.svelte";
  import ObservabilityLogControl from "$lib/components/kernel/ObservabilityLogControl.svelte";
  import ProjectTransitionControl from "$lib/components/kernel/ProjectTransitionControl.svelte";
  import RecoveryControl from "$lib/components/kernel/RecoveryControl.svelte";
  import WriteAuthorityRecoveryControl from "$lib/components/kernel/WriteAuthorityRecoveryControl.svelte";

  let {
    currentProjectPath = "",
    projectFileCount = 0,
    sourceNodeCount = 0,
    dirtyAreas = [],
    canSave = false,
    diskBlockedReason = null,
    projectStatus = "",
    onStatusUpdate = undefined as ((text: string, kind: "restored" | "saving" | "error") => void) | undefined,
  }: {
    currentProjectPath?: string;
    projectFileCount?: number;
    sourceNodeCount?: number;
    dirtyAreas?: string[];
    canSave?: boolean;
    diskBlockedReason?: string | null;
    projectStatus?: string;
    onStatusUpdate?: (text: string, kind: "restored" | "saving" | "error") => void;
  } = $props();

  let recoveryRefreshToken = $state(0);
  let diskConflictRefreshToken = $state(0);
  let projectTransitionRefreshToken = $state(0);
  let observabilityRefreshToken = $state(0);
  let writeAuthorityRefreshToken = $state(0);

  const projectName = $derived(currentProjectPath.split(/[\\/]/).filter(Boolean).at(-1) ?? "Proiect");
  const dirtyLabel = $derived(dirtyAreas.length ? dirtyAreas.join(", ") : "curat");
  const statusLabel = $derived(projectStatus || "Sesiunea proiectului este activă");

  function refreshKernelSurfaces() {
    recoveryRefreshToken += 1;
    diskConflictRefreshToken += 1;
    projectTransitionRefreshToken += 1;
    observabilityRefreshToken += 1;
    writeAuthorityRefreshToken += 1;
  }
</script>

<section class="kernel-workspace" aria-label="Nucleu Pană Studio">
  <header class="kernel-header">
    <div>
      <span class="kicker">Autoritate unică</span>
      <h1>Nucleul proiectului</h1>
      <p>
        HTML, CSS/SCSS, JavaScript, codul, previzualizarea și istoricul proiectează aceeași revizie din memorie.
        Numai salvarea trece prin verificarea de conflict și granița de scriere pe disc.
      </p>
    </div>
    <dl>
      <div><dt>Proiect</dt><dd title={currentProjectPath}>{projectName}</dd></div>
      <div><dt>Fișiere</dt><dd>{projectFileCount}</dd></div>
      <div><dt>Noduri sursă</dt><dd>{sourceNodeCount}</dd></div>
      <div><dt>Sesiune</dt><dd class:warning={canSave}>{dirtyLabel}</dd></div>
    </dl>
  </header>

  <div class:blocked={Boolean(diskBlockedReason)} class="kernel-alert">
    <IconShieldCheck size={18} stroke={1.8} />
    <span>{diskBlockedReason || statusLabel}</span>
  </div>

  <section class="authority-flow" aria-labelledby="authority-flow-title">
    <div class="section-title">
      <IconGitBranch size={18} stroke={1.8} />
      <div>
        <h2 id="authority-flow-title">Contractul de editare</h2>
        <p>O singură stare autoritativă, două tipuri clare de efect: proiecție și salvare.</p>
      </div>
    </div>
    <div class="flow">
      <span>Intenție UI</span>
      <span>Sesiunea proiectului</span>
      <span>Revizie + istoric</span>
      <span>Previzualizare / panouri</span>
      <span>Salvare verificată</span>
      <span>Disc</span>
    </div>
    <div class="invariants">
      <span><IconActivity size={15} stroke={1.9} /> Editările nu scriu pe disc</span>
      <span><IconActivity size={15} stroke={1.9} /> Anularea și refacerea schimbă sesiunea</span>
      <span><IconActivity size={15} stroke={1.9} /> Previzualizarea este derivată din revizia curentă</span>
      <span><IconActivity size={15} stroke={1.9} /> Salvarea verifică baza și jurnalizează atomic</span>
    </div>
  </section>

  <div class="kernel-grid">
    <WriteAuthorityRecoveryControl refreshToken={writeAuthorityRefreshToken} {onStatusUpdate} />
    <RecoveryControl
      projectKey={currentProjectPath}
      refreshToken={recoveryRefreshToken}
      {onStatusUpdate}
      onChanged={refreshKernelSurfaces}
    />
    <DiskConflictControl
      projectKey={currentProjectPath}
      refreshToken={diskConflictRefreshToken}
      {onStatusUpdate}
    />
    <ProjectTransitionControl
      projectKey={currentProjectPath}
      refreshToken={projectTransitionRefreshToken}
      {onStatusUpdate}
    />
    <ObservabilityLogControl
      projectKey={currentProjectPath}
      refreshToken={observabilityRefreshToken}
      {onStatusUpdate}
    />
  </div>
</section>

<style>
  .kernel-workspace { display: flex; flex-direction: column; min-width: 0; min-height: 0; height: 100%; overflow: auto; border: 1px solid var(--border); border-radius: 10px; background: var(--surface); box-shadow: var(--shadow); }
  .kernel-header { display: grid; grid-template-columns: minmax(0, 1fr) minmax(320px, 500px); gap: 24px; padding: 22px; border-bottom: 1px solid var(--border); background: var(--surface-2); }
  .kicker { color: var(--brand-strong); font-size: 12px; font-weight: 850; text-transform: uppercase; }
  h1 { margin: 7px 0 0; color: var(--text-strong); font-size: 30px; }
  .kernel-header p { max-width: 760px; margin: 9px 0 0; color: var(--text-muted); font-size: 13px; line-height: 1.5; }
  dl { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; margin: 0; }
  dl div { min-width: 0; padding: 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface-4); }
  dt { color: var(--text-muted); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  dd { margin: 5px 0 0; overflow: hidden; color: var(--text-strong); font-size: 12px; font-weight: 750; text-overflow: ellipsis; white-space: nowrap; }
  dd.warning { color: #d97706; }
  .kernel-alert { display: flex; align-items: center; gap: 8px; padding: 10px 22px; border-bottom: 1px solid var(--border); color: var(--text-muted); font-size: 12px; }
  .kernel-alert.blocked { color: #dc2626; background: color-mix(in srgb, #ef4444 7%, var(--surface)); }
  .authority-flow { display: grid; gap: 12px; margin: 14px; padding: 14px; border: 1px solid var(--border); border-radius: 9px; background: var(--surface-3); }
  .section-title { display: flex; align-items: center; gap: 9px; }
  h2 { margin: 0; font-size: 14px; }
  .section-title p { margin: 3px 0 0; color: var(--text-muted); font-size: 12px; }
  .flow { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: 6px; }
  .flow span { padding: 9px 7px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface); color: var(--text-strong); font-size: 12px; font-weight: 750; text-align: center; }
  .invariants { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; }
  .invariants span { display: flex; align-items: center; gap: 6px; color: var(--text-muted); font-size: 12px; }
  .kernel-grid { display: grid; gap: 12px; padding: 0 14px 14px; }
  :global(.kernel-grid > *) { min-width: 0; }
  @media (max-width: 980px) {
    .kernel-header { grid-template-columns: 1fr; }
    .flow { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  }
  @media (max-width: 680px) {
    .flow,
    .invariants { grid-template-columns: 1fr; }
  }
</style>
