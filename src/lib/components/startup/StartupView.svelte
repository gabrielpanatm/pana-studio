<script lang="ts">
  import {
    IconAlertTriangle as AlertTriangle,
    IconArrowLeft as ArrowLeft,
    IconCheck as Check,
    IconFileCode2 as FileCode2,
    IconFolderOpen as FolderOpen,
    IconStack3 as Layers3,
    IconLoader2 as LoaderCircle,
    IconSettings as Settings,
    IconShieldCheck as ShieldCheck,
    IconSparkles as Sparkles,
  } from "@tabler/icons-svelte";
  import WriteAuthorityRecoveryControl from "$lib/components/kernel/WriteAuthorityRecoveryControl.svelte";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type { WriteAuthorityRecoveryScan } from "$lib/kernel/recovery-contract";
  import type {
    StartupCreationCatalog,
    StartupCreationKind,
    StartupCreationPlan,
    StartupFlowSnapshot,
  } from "$lib/project/lifecycle-contract";

  let {
    startupFlow,
    startupError,
    startupPending,
    startupCreationPlan,
    startupCreationCatalog,
    startupSelectedOptionId,
    globalStatus,
    openApplicationSettings,
    cancelStartupCreationPlan,
    applyStartupProject,
    selectStartupCreationOption,
    openProjectFolder,
    planStartupProject,
    retryStartupProjectOpen,
  }: {
    startupFlow: StartupFlowSnapshot;
    startupError: string;
    startupPending: boolean;
    startupCreationPlan: StartupCreationPlan | null;
    startupCreationCatalog: StartupCreationCatalog | null;
    startupSelectedOptionId: string | null;
    globalStatus: GlobalStatusState;
    openApplicationSettings: () => void;
    cancelStartupCreationPlan: () => void;
    applyStartupProject: () => void | Promise<void>;
    selectStartupCreationOption: (optionId: string) => void;
    openProjectFolder: () => void | Promise<void>;
    planStartupProject: () => void | Promise<void>;
    retryStartupProjectOpen: () => void | Promise<void>;
  } = $props();

  const candidate = $derived(startupFlow.candidate);
  const writeAuthorityRecoveryRequired = $derived(
    startupError.includes("WRITE_AUTHORITY_RECOVERY_BLOCKED"),
  );
  let startupRecoveryScan = $state<WriteAuthorityRecoveryScan | null>(null);
  const planning = $derived(Boolean(startupCreationPlan));
  const busyLabel = $derived(
    startupCreationPlan
      ? "Creăm și validăm proiectul…"
      : candidate?.kind === "valid_project"
        ? "Deschidem proiectul valid…"
        : "Inspectăm dosarul în Rust…",
  );

  function kindLabel(kind: StartupCreationKind) {
    if (kind === "minimal") return "MINIMAL";
    return "PUNCT DE PORNIRE";
  }

  function formatBytes(value: number) {
    if (value < 1_024) return `${value} B`;
    if (value < 1_048_576) return `${(value / 1_024).toFixed(1)} KB`;
    return `${(value / 1_048_576).toFixed(1)} MB`;
  }
</script>

<section class="startup-view" aria-label="Pornire Pană Studio">
  <div class="startup-ambient startup-ambient-a"></div>
  <div class="startup-ambient startup-ambient-b"></div>

  <header class="startup-brand">
    <div class="startup-mark" aria-hidden="true">
      <Sparkles size={18} stroke={1.8} />
    </div>
    <div class="startup-brand-copy">
      <strong>Pană Studio</strong>
      <span>Editor vizual Rust-first pentru Zola</span>
    </div>
    <button
      type="button"
      class="startup-settings ui-icon-button"
      aria-label="Setările aplicației"
      title="Setările aplicației"
      onclick={openApplicationSettings}
    >
      <Settings size={17} stroke={1.8} />
    </button>
  </header>

  <main class="startup-content">
    {#if startupPending}
      <section class="startup-state-card startup-progress" aria-live="polite" aria-busy="true">
        <div class="startup-state-icon is-progress">
          <LoaderCircle size={26} stroke={1.8} />
        </div>
        <p class="startup-eyebrow">FLUX AUTORITATIV RUST</p>
        <h1>{busyLabel}</h1>
        <p>
          Verificăm rădăcina, structura și compatibilitatea Zola înainte de a monta editorul.
        </p>
        <div class="startup-progress-track"><span></span></div>
      </section>
    {:else if planning && startupCreationPlan}
      <section class="startup-review" aria-labelledby="startup-review-title">
        <button
          type="button"
          class="startup-back ui-button"
          onclick={cancelStartupCreationPlan}
        >
          <ArrowLeft size={15} />
          Înapoi la catalog
        </button>

        <div class="startup-review-grid">
          <div class="startup-state-card startup-review-summary">
            <div class="startup-state-icon">
              <ShieldCheck size={25} stroke={1.7} />
            </div>
            <p class="startup-eyebrow">PLAN RUST CONFIRMAT</p>
            <h1 id="startup-review-title">{startupCreationPlan.optionName}</h1>
            <p>
              Planul este legat de snapshot-ul dosarului gol. Orice schimbare pe disk îl invalidează.
            </p>
            <dl class="startup-plan-facts">
              <div>
                <dt>Fișiere noi</dt>
                <dd>{startupCreationPlan.affectedFiles.length}</dd>
              </div>
              <div>
                <dt>Dimensiune</dt>
                <dd>{formatBytes(startupCreationPlan.totalBytes)}</dd>
              </div>
              <div>
                <dt>Suprascrieri</dt>
                <dd>0</dd>
              </div>
            </dl>
            <button
              type="button"
              class="startup-primary ui-button ui-button-accent"
              onclick={() => { void applyStartupProject(); }}
            >
              <Check size={16} stroke={2} />
              Creează și deschide proiectul
            </button>
          </div>

          <div class="startup-plan-files">
            <div class="startup-panel-heading">
              <div>
                <span>PUBLICAȚII PLANIFICATE</span>
                <strong>Conținutul proiectului</strong>
              </div>
              <span class="startup-count">{startupCreationPlan.affectedFiles.length}</span>
            </div>
            <div class="startup-file-list">
              {#each startupCreationPlan.affectedFiles as path}
                <div class="startup-file-row">
                  <FileCode2 size={15} stroke={1.7} />
                  <code>{path}</code>
                </div>
              {/each}
            </div>
          </div>
        </div>
      </section>
    {:else if candidate?.kind === "empty_directory"}
      <section class="startup-catalog" aria-labelledby="startup-catalog-title">
        <div class="startup-catalog-heading">
          <div>
            <p class="startup-eyebrow">DOSAR GOL CONFIRMAT DE RUST</p>
            <h1 id="startup-catalog-title">Alege punctul de plecare</h1>
            <p>
              Proiectele sunt publicate fără suprascriere și validate cu motorul Zola embedded.
            </p>
          </div>
          <div class="startup-folder-chip" title={candidate.root}>
            <FolderOpen size={16} stroke={1.7} />
            <span>{candidate.displayName}</span>
          </div>
        </div>

        {#if startupError}
          <p class="startup-inline-error" role="alert">{startupError}</p>
        {/if}

        <div class="startup-option-grid">
          {#each startupCreationCatalog?.options ?? [] as option}
            <button
              type="button"
              class="startup-option ui-entity-selectable"
              class:has-preview={Boolean(option.previewDataUrl)}
              data-ui-selected={startupSelectedOptionId === option.id ? "true" : undefined}
              aria-pressed={startupSelectedOptionId === option.id}
              onclick={() => selectStartupCreationOption(option.id)}
            >
              <div class="startup-option-preview">
                {#if option.previewDataUrl}
                  <img src={option.previewDataUrl} alt="" />
                {:else}
                  <div class="startup-minimal-preview" aria-hidden="true">
                    <span></span><span></span><span></span>
                    <strong>&lt;main&gt;</strong>
                  </div>
                {/if}
                <span class="startup-option-kind">{kindLabel(option.kind)}</span>
                {#if startupSelectedOptionId === option.id}
                  <span class="startup-selected-mark"><Check size={13} stroke={2.2} /></span>
                {/if}
              </div>
              <div class="startup-option-copy">
                <strong>{option.name}</strong>
                <p>{option.description}</p>
                <span>{option.compatibilityLabel}</span>
              </div>
            </button>
          {/each}
        </div>

        <footer class="startup-catalog-actions">
          <button type="button" class="ui-button" onclick={() => { void openProjectFolder(); }}>
            <FolderOpen size={15} />
            Alege alt dosar
          </button>
          <button
            type="button"
            class="ui-button ui-button-accent"
            disabled={!startupSelectedOptionId}
            onclick={() => { void planStartupProject(); }}
          >
            Continuă cu selecția
          </button>
        </footer>
      </section>
    {:else if candidate?.kind === "valid_project"}
      <section
        class="startup-state-card startup-result"
        class:has-recovery={writeAuthorityRecoveryRequired}
        aria-live="polite"
      >
        <div class="startup-state-icon">
          <ShieldCheck size={26} stroke={1.8} />
        </div>
        <p class="startup-eyebrow">PROIECT ZOLA VALID</p>
        <h1>Proiectul este pregătit pentru deschidere</h1>
        <p class="startup-path">{candidate.root}</p>
        <p>
          Rust a validat structura proiectului. Deschiderea continuă în Workbench; dacă Preview-ul
          găsește o eroare Zola, fișierul diagnostic va fi deschis direct în Cod pentru reparare.
        </p>
        {#if startupError && !writeAuthorityRecoveryRequired}
          <p class="startup-inline-error">{startupError}</p>
        {/if}
        {#if writeAuthorityRecoveryRequired}
          <div class="startup-recovery-intro" role="alert">
            <AlertTriangle size={18} stroke={1.9} />
            <div>
              <strong>Proiectul nu este pierdut</strong>
              <span>O scriere întreruptă trebuie reconciliată înainte ca editorul să primească din nou drept de modificare. Alege acțiunea oferită de Rust pentru fiecare operație.</span>
            </div>
          </div>
          <div class="startup-recovery-control">
            <WriteAuthorityRecoveryControl
              onScanUpdate={(scan) => { startupRecoveryScan = scan; }}
              onStatusUpdate={(text, kind) => globalStatus.set(text, kind)}
            />
          </div>
          <div class="startup-recovery-actions">
            <button type="button" class="ui-button" onclick={() => { void openProjectFolder(); }}>
              <FolderOpen size={16} />
              Alege alt dosar
            </button>
            <button
              type="button"
              class="ui-button ui-button-accent"
              disabled={startupRecoveryScan?.blocked !== false || startupPending}
              onclick={() => { void retryStartupProjectOpen(); }}
            >
              <Check size={16} stroke={2} />
              Redeschide proiectul după recuperare
            </button>
          </div>
        {:else}
          <button
            type="button"
            class="startup-primary ui-button"
            onclick={() => { void openProjectFolder(); }}
          >
            <FolderOpen size={16} />
            Alege alt dosar
          </button>
        {/if}
      </section>
    {:else if candidate}
      <section class="startup-state-card startup-result" aria-live="polite">
        <div
          class:danger={candidate.kind === "invalid_zola_project" || candidate.kind === "inaccessible"}
          class="startup-state-icon"
        >
          <AlertTriangle size={26} stroke={1.8} />
        </div>
        <p class="startup-eyebrow">
          {candidate.kind === "unrecognized_directory"
            ? "DOSAR NERECUNOSCUT"
            : candidate.kind === "invalid_zola_project"
              ? "PROIECT ZOLA INVALID"
              : "DOSAR INACCESIBIL"}
        </p>
        <h1>
          {candidate.kind === "unrecognized_directory"
            ? "Acest dosar nu va fi modificat"
            : candidate.kind === "invalid_zola_project"
              ? "Proiectul necesită corectare înainte de deschidere"
              : "Dosarul nu poate fi inspectat"}
        </h1>
        <p class="startup-path">{candidate.root}</p>
        <div class="startup-diagnostics">
          {#each candidate.diagnostics as diagnostic}
            <div class="startup-diagnostic">
              <AlertTriangle size={15} stroke={1.8} />
              <div>
                <strong>{diagnostic.message}</strong>
                {#if diagnostic.detail}<span>{diagnostic.detail}</span>{/if}
              </div>
            </div>
          {/each}
        </div>
        {#if startupError}
          <p class="startup-inline-error">{startupError}</p>
        {/if}
        <button
          type="button"
          class="startup-primary ui-button ui-button-accent"
          onclick={() => { void openProjectFolder(); }}
        >
          <FolderOpen size={16} />
          Alege alt dosar
        </button>
      </section>
    {:else}
      <section class="startup-idle">
        <div class="startup-hero">
          <p class="startup-eyebrow">SPAȚIU DE LUCRU LOCAL · ZOLA EMBEDDED</p>
          <h1>Construiește vizual.<br /><span>Păstrează controlul sursei.</span></h1>
          <p class="startup-lead">
            Deschide un proiect Zola existent sau alege un dosar gol pentru a porni dintr-un
            punct de pornire verificat.
          </p>
          <button
            type="button"
            class="startup-open ui-button ui-button-accent"
            onclick={() => { void openProjectFolder(); }}
          >
            <FolderOpen size={18} stroke={1.8} />
            Deschide dosar
          </button>
          {#if startupError}
            <p class="startup-inline-error">{startupError}</p>
          {/if}
        </div>

        <div class="startup-feature-grid">
          <article>
            <div><ShieldCheck size={18} stroke={1.7} /></div>
            <strong>Validare înainte de sesiune</strong>
            <p>Editorul se montează numai după confirmarea Rust și Zola.</p>
          </article>
          <article>
            <div><Layers3 size={18} stroke={1.7} /></div>
            <strong>Startere și șabloane</strong>
            <p>Catalog bundled, compatibil și fără suprascrieri ascunse.</p>
          </article>
          <article>
            <div><FileCode2 size={18} stroke={1.7} /></div>
            <strong>Fișierele rămân ale tale</strong>
            <p>Structură Zola reală, editabilă în orice instrument.</p>
          </article>
        </div>
      </section>
    {/if}
  </main>

  <footer class="startup-footer">
    <span><i></i> Motor Rust disponibil</span>
    <span>Zola embedded</span>
  </footer>
</section>

<style>
  .startup-view {
    position: relative;
    isolation: isolate;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    width: 100%;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    color: var(--text);
    background:
      radial-gradient(circle at 50% -20%, color-mix(in srgb, var(--brand) 14%, transparent), transparent 45%),
      linear-gradient(145deg, var(--surface-base), color-mix(in srgb, var(--surface-base) 94%, var(--surface-panel)));
  }

  .startup-ambient {
    position: absolute;
    z-index: -1;
    width: 420px;
    height: 420px;
    border-radius: 50%;
    opacity: 0.24;
    filter: blur(90px);
    pointer-events: none;
  }

  .startup-ambient-a {
    top: -240px;
    right: 8%;
    background: var(--brand);
  }

  .startup-ambient-b {
    bottom: -300px;
    left: 6%;
    background: color-mix(in srgb, var(--info) 55%, var(--brand));
  }

  .startup-brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 22px 28px;
  }

  .startup-brand-copy {
    display: grid;
    gap: 2px;
  }

  .startup-settings {
    margin-left: auto;
  }

  .startup-brand strong {
    color: var(--text-strong);
    font-size: 14px;
    letter-spacing: -0.01em;
  }

  .startup-brand span {
    color: var(--text-muted);
    font-size: 11px;
  }

  .startup-mark,
  .startup-state-icon,
  .startup-feature-grid article > div {
    display: grid;
    place-items: center;
    color: var(--brand-strong);
    border: 1px solid color-mix(in srgb, var(--brand) 26%, var(--border-subtle));
    background: var(--material-control);
    box-shadow: var(--shadow-control);
  }

  .startup-mark {
    width: 34px;
    height: 34px;
    border-radius: 10px;
  }

  .startup-content {
    min-height: 0;
    overflow: auto;
    padding: 22px clamp(28px, 6vw, 92px) 38px;
  }

  .startup-idle {
    display: grid;
    gap: 52px;
    width: min(1040px, 100%);
    margin: clamp(26px, 7vh, 90px) auto 0;
  }

  .startup-hero {
    max-width: 760px;
  }

  .startup-eyebrow,
  .startup-panel-heading span {
    margin: 0 0 13px;
    color: var(--brand-strong);
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.12em;
  }

  h1 {
    margin: 0;
    color: var(--text-strong);
    font-size: clamp(32px, 4.4vw, 62px);
    line-height: 0.98;
    letter-spacing: -0.052em;
  }

  h1 span {
    color: color-mix(in srgb, var(--text-strong) 62%, var(--brand));
  }

  .startup-lead,
  .startup-state-card > p:not(.startup-eyebrow, .startup-path),
  .startup-catalog-heading > div > p:last-child {
    max-width: 630px;
    margin: 24px 0 0;
    color: var(--text-muted);
    font-size: 15px;
    line-height: 1.62;
  }

  .startup-open,
  .startup-primary {
    min-height: 40px;
    margin-top: 26px;
    padding-inline: 18px;
    font-weight: 700;
  }

  .startup-feature-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }

  .startup-feature-grid article,
  .startup-state-card,
  .startup-plan-files {
    border: 1px solid var(--border-subtle);
    background: var(--material-panel);
    box-shadow: var(--shadow-panel);
  }

  .startup-feature-grid article {
    min-height: 134px;
    padding: 18px;
    border-radius: var(--radius-panel);
  }

  .startup-feature-grid article > div {
    width: 31px;
    height: 31px;
    margin-bottom: 16px;
    border-radius: 9px;
  }

  .startup-feature-grid strong {
    display: block;
    margin-bottom: 7px;
    color: var(--text-strong);
    font-size: 13px;
  }

  .startup-feature-grid p {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.5;
  }

  .startup-state-card {
    width: min(620px, 100%);
    margin: clamp(38px, 10vh, 130px) auto 0;
    padding: clamp(28px, 5vw, 54px);
    border-radius: 18px;
    text-align: center;
  }

  .startup-state-card h1 {
    font-size: clamp(27px, 3vw, 42px);
    line-height: 1.08;
  }

  .startup-state-card > p:not(.startup-eyebrow, .startup-path) {
    margin-inline: auto;
  }

  .startup-state-card.has-recovery {
    width: min(900px, 100%);
    margin-top: 18px;
    padding: clamp(22px, 3vw, 34px);
  }

  .startup-recovery-intro {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin-top: 22px;
    padding: 11px 12px;
    color: var(--warning);
    text-align: left;
    border: 1px solid color-mix(in srgb, var(--warning) 44%, var(--border-subtle));
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--warning) 8%, var(--surface-panel));
  }

  .startup-recovery-intro > div {
    display: grid;
    gap: 3px;
  }

  .startup-recovery-intro strong {
    color: var(--text-strong);
    font-size: 13px;
  }

  .startup-recovery-intro span {
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.5;
  }

  .startup-recovery-control {
    margin-top: 12px;
    padding: 14px;
    text-align: left;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-panel);
    background: var(--surface-inset);
    box-shadow: var(--shadow-inset);
  }

  .startup-recovery-actions {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    margin-top: 14px;
  }

  .startup-state-icon {
    width: 48px;
    height: 48px;
    margin: 0 auto 22px;
    border-radius: 14px;
  }

  .startup-state-icon.danger {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 32%, var(--border-subtle));
  }

  .startup-state-icon.is-progress :global(svg) {
    animation: startup-spin 0.9s linear infinite;
  }

  .startup-progress-track {
    height: 4px;
    margin-top: 30px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-inset);
    box-shadow: var(--shadow-inset);
  }

  .startup-progress-track span {
    display: block;
    width: 42%;
    height: 100%;
    border-radius: inherit;
    background: var(--brand);
    animation: startup-progress 1.4s ease-in-out infinite alternate;
  }

  .startup-catalog,
  .startup-review {
    width: min(1180px, 100%);
    margin: 12px auto 0;
  }

  .startup-catalog-heading {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
    margin-bottom: 24px;
  }

  .startup-catalog-heading h1 {
    font-size: clamp(28px, 3.4vw, 46px);
  }

  .startup-folder-chip {
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 260px;
    padding: 8px 11px;
    overflow: hidden;
    color: var(--text-muted);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-control);
    background: var(--material-control);
    box-shadow: var(--shadow-control);
  }

  .startup-folder-chip span {
    overflow: hidden;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .startup-option-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 13px;
  }

  .startup-option {
    display: grid;
    grid-template-rows: 146px auto;
    min-width: 0;
    min-height: 286px;
    padding: 0;
    overflow: hidden;
    color: inherit;
    text-align: left;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-panel);
    background: var(--material-panel);
    box-shadow: var(--shadow-panel);
  }

  .startup-option-preview {
    position: relative;
    overflow: hidden;
    background: var(--surface-inset);
    box-shadow: var(--shadow-inset);
  }

  .startup-option-preview img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .startup-minimal-preview {
    display: grid;
    align-content: center;
    gap: 8px;
    width: 72%;
    height: 100%;
    margin: auto;
  }

  .startup-minimal-preview span {
    display: block;
    height: 7px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text-muted) 22%, transparent);
  }

  .startup-minimal-preview span:nth-child(2) { width: 72%; }
  .startup-minimal-preview span:nth-child(3) { width: 48%; }

  .startup-minimal-preview strong {
    margin-top: 8px;
    color: var(--brand-strong);
    font-family: var(--font-mono);
    font-size: 18px;
  }

  .startup-option-kind,
  .startup-selected-mark {
    position: absolute;
    top: 10px;
    padding: 4px 7px;
    border: 1px solid color-mix(in srgb, var(--border-strong) 72%, transparent);
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface-raised) 88%, transparent);
    box-shadow: var(--shadow-control);
  }

  .startup-option-kind {
    left: 10px;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.08em;
  }

  .startup-selected-mark {
    right: 10px;
    display: grid;
    place-items: center;
    width: 23px;
    height: 23px;
    padding: 0;
    color: var(--text-on-accent);
    border-color: color-mix(in srgb, var(--brand) 72%, white);
    background: var(--brand);
  }

  .startup-option-copy {
    padding: 16px;
  }

  .startup-option-copy strong {
    color: var(--text-strong);
    font-size: 14px;
  }

  .startup-option-copy p {
    min-height: 55px;
    margin: 8px 0 13px;
    color: var(--text-muted);
    font-size: 11.5px;
    line-height: 1.5;
  }

  .startup-option-copy span {
    color: var(--text-muted);
    font-size: 11px;
  }

  .startup-catalog-actions {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin-top: 22px;
    padding-top: 18px;
    border-top: 1px solid var(--border-subtle);
  }

  .startup-review > .startup-back {
    margin-bottom: 18px;
  }

  .startup-review-grid {
    display: grid;
    grid-template-columns: minmax(360px, 0.85fr) minmax(420px, 1.15fr);
    gap: 16px;
  }

  .startup-review-summary {
    width: auto;
    margin: 0;
    text-align: left;
  }

  .startup-review-summary .startup-state-icon {
    margin-inline: 0;
  }

  .startup-review-summary > p:not(.startup-eyebrow) {
    margin-inline: 0;
  }

  .startup-plan-facts {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    margin: 26px 0 0;
  }

  .startup-plan-facts div {
    padding: 12px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-control);
    background: var(--surface-inset);
    box-shadow: var(--shadow-inset);
  }

  .startup-plan-facts dt {
    color: var(--text-muted);
    font-size: 11px;
  }

  .startup-plan-facts dd {
    margin: 5px 0 0;
    color: var(--text-strong);
    font-size: 15px;
    font-weight: 800;
  }

  .startup-plan-files {
    min-height: 0;
    overflow: hidden;
    border-radius: 18px;
  }

  .startup-panel-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 17px 18px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .startup-panel-heading > div {
    display: grid;
    gap: 4px;
  }

  .startup-panel-heading span {
    margin: 0;
  }

  .startup-panel-heading strong {
    color: var(--text-strong);
    font-size: 13px;
  }

  .startup-panel-heading .startup-count {
    display: grid;
    place-items: center;
    min-width: 27px;
    height: 24px;
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--surface-inset);
    box-shadow: var(--shadow-inset);
  }

  .startup-file-list {
    max-height: min(54vh, 520px);
    overflow: auto;
    padding: 8px;
  }

  .startup-file-row {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 32px;
    padding: 0 10px;
    color: var(--text-muted);
    border-radius: 7px;
  }

  .startup-file-row:nth-child(odd) {
    background: color-mix(in srgb, var(--surface-inset) 54%, transparent);
  }

  .startup-file-row code {
    overflow: hidden;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .startup-path {
    margin: 15px auto 0;
    overflow-wrap: anywhere;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .startup-diagnostics {
    display: grid;
    gap: 8px;
    margin-top: 24px;
    text-align: left;
  }

  .startup-diagnostic {
    display: flex;
    gap: 10px;
    padding: 12px;
    color: var(--warning);
    border: 1px solid color-mix(in srgb, var(--warning) 24%, var(--border-subtle));
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--warning) 7%, var(--surface-inset));
  }

  .startup-diagnostic > div {
    display: grid;
    gap: 5px;
  }

  .startup-diagnostic strong,
  .startup-diagnostic span {
    color: var(--text);
    font-size: 11px;
    line-height: 1.45;
  }

  .startup-diagnostic span {
    color: var(--text-muted);
  }

  .startup-inline-error {
    margin: 16px auto 0;
    color: var(--danger);
    font-size: 11px;
    line-height: 1.45;
  }

  .startup-footer {
    display: flex;
    justify-content: space-between;
    padding: 12px 28px 16px;
    color: var(--text-muted);
    font-size: 11px;
  }

  .startup-footer span {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .startup-footer i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--success) 14%, transparent);
  }

  @keyframes startup-spin {
    to { transform: rotate(360deg); }
  }

  @keyframes startup-progress {
    from { transform: translateX(-12%); }
    to { transform: translateX(145%); }
  }

  @media (max-width: 980px) {
    .startup-option-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .startup-review-grid { grid-template-columns: 1fr; }
  }

  @media (max-width: 720px) {
    .startup-content { padding-inline: 20px; }
    .startup-feature-grid { grid-template-columns: 1fr; }
    .startup-catalog-heading { align-items: start; flex-direction: column; }
    .startup-option-grid { grid-template-columns: 1fr; }
    .startup-option { grid-template-rows: 130px auto; }
  }
</style>
