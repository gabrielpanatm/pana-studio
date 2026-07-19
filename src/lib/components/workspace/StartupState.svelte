<script lang="ts">
  let {
    scannedProject = false,
    isEmpty = false,
    isZola = false,
    openProjectFolder,
    initZolaProject,
  }: {
    scannedProject?: boolean;
    isEmpty?: boolean;
    isZola?: boolean;
    openProjectFolder: () => void | Promise<void>;
    initZolaProject: () => void | Promise<void>;
  } = $props();
</script>

{#if !scannedProject}
  <div class="empty-state">
    <p class="empty-title">Pană Studio</p>
    <p class="empty-sub">Studio local pentru proiecte web Zola.<br>Deschide root-ul proiectului pentru a începe.</p>
    <button type="button" class="empty-open-btn" onclick={openProjectFolder}>
      Deschide dosar
    </button>
  </div>
{:else if isEmpty}
  <div class="empty-state">
    <p class="empty-title">Dosar gol</p>
    <p class="empty-sub">
      Acest dosar poate deveni un proiect web Pană Studio.<br>
      Aplicația va crea brief, structură, resurse și Zola în <code>sursa/</code>.
    </p>
    <button type="button" class="empty-open-btn" onclick={initZolaProject}>
      Inițializează proiect Pană Studio
    </button>
    <button type="button" class="empty-secondary-btn" onclick={openProjectFolder}>
      Deschide alt dosar
    </button>
  </div>
{:else if !isZola}
  <div class="empty-state">
    <p class="empty-title">Nu este un proiect Pană Studio</p>
    <p class="empty-sub">
      Deschide root-ul proiectului complet, cu Zola în <code>sursa/</code>.<br>
      Sau alege un dosar gol pentru inițializare.
    </p>
    <button type="button" class="empty-open-btn" onclick={openProjectFolder}>
      Deschide alt dosar
    </button>
  </div>
{/if}

<style>
  .empty-state {
    position: absolute;
    inset: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 24px;
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    text-align: center;
    background:
      linear-gradient(135deg, color-mix(in srgb, var(--brand) 13%, transparent), transparent 34%),
      linear-gradient(315deg, color-mix(in srgb, var(--brand-strong) 10%, transparent), transparent 38%),
      repeating-linear-gradient(
        90deg,
        color-mix(in srgb, var(--border-3) 26%, transparent) 0,
        color-mix(in srgb, var(--border-3) 26%, transparent) 1px,
        transparent 1px,
        transparent 56px
      ),
      repeating-linear-gradient(
        0deg,
        color-mix(in srgb, var(--border-3) 18%, transparent) 0,
        color-mix(in srgb, var(--border-3) 18%, transparent) 1px,
        transparent 1px,
        transparent 56px
      ),
      var(--surface-6);
    box-shadow: var(--shadow);
  }

  .empty-state::before {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, color-mix(in srgb, var(--surface-2) 54%, transparent), transparent 42%);
    pointer-events: none;
  }

  .empty-state > * {
    position: relative;
    z-index: 1;
  }

  .empty-title {
    margin: 0;
    color: var(--text);
    font-size: 22px;
    font-weight: 800;
    letter-spacing: 0;
  }

  .empty-sub {
    max-width: 480px;
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
    line-height: 1.65;
  }

  .empty-open-btn {
    height: 34px;
    margin-top: 4px;
    padding: 0 22px;
    border: none;
    border-radius: 8px;
    color: #ffffff;
    font-size: 14px;
    font-weight: 700;
    background: var(--brand);
    cursor: pointer;
    transition: opacity 80ms;
  }

  .empty-open-btn:hover {
    opacity: 0.88;
  }

  .empty-secondary-btn {
    width: 240px;
    height: 34px;
    border: 1px solid var(--border-4);
    border-radius: 8px;
    color: var(--text-muted);
    font-size: 13px;
    font-weight: 600;
    background: var(--surface-4);
    cursor: pointer;
    transition: opacity 80ms;
  }

  .empty-secondary-btn:hover {
    opacity: 0.8;
  }
</style>
