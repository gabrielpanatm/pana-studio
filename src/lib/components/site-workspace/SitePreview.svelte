<script lang="ts">
  import {
    IconExternalLink,
    IconLock,
    IconRefresh,
    IconWorld,
  } from "@tabler/icons-svelte";

  let {
    title = "Preview site",
    route = "/",
    src = "",
    srcdoc = null,
    compact = false,
    showRefresh = false,
    onRefresh = () => {},
    onOpenExternal = () => {},
  }: {
    title?: string;
    route?: string;
    src?: string;
    srcdoc?: string | null;
    compact?: boolean;
    showRefresh?: boolean;
    onRefresh?: () => void | Promise<void>;
    onOpenExternal?: () => void | Promise<void>;
  } = $props();
</script>

<section class:compact class="site-preview" aria-label={`Preview pentru ${title}`}>
  <header class="browser-bar">
    <div class="window-dots" aria-hidden="true"><i></i><i></i><i></i></div>
    <div class="address-bar" title={route}>
      <IconLock size={13} stroke={1.9} />
      <span>{route || "/"}</span>
    </div>
    <div class="browser-actions">
      {#if showRefresh}
        <button type="button" aria-label="Reîncarcă preview-ul" title="Reîncarcă preview-ul" onclick={() => { void onRefresh(); }}>
          <IconRefresh size={15} stroke={1.9} />
        </button>
      {/if}
      <button type="button" aria-label="Deschide site-ul în browser" title="Deschide în browser" onclick={() => { void onOpenExternal(); }}>
        <IconExternalLink size={15} stroke={1.9} />
      </button>
    </div>
  </header>

  {#if src || srcdoc}
    <div class="frame-stage">
      <iframe
        title={title}
        src={srcdoc ? undefined : src}
        srcdoc={srcdoc ?? undefined}
        sandbox=""
      ></iframe>
    </div>
  {:else}
    <div class="preview-empty">
      <IconWorld size={30} stroke={1.5} />
      <strong>Preview indisponibil</strong>
      <span>Pagina va apărea aici după prima randare Zola.</span>
    </div>
  {/if}
</section>

<style>
  .site-preview {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    min-height: 360px;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border) 86%, var(--text-muted));
    border-radius: 14px;
    background: #fff;
    box-shadow: 0 18px 42px color-mix(in srgb, var(--text-strong) 10%, transparent);
  }

  .site-preview.compact {
    min-height: 270px;
  }

  .browser-bar {
    display: grid;
    grid-template-columns: 62px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    min-height: 42px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border-2);
    background: color-mix(in srgb, var(--surface-2) 86%, var(--surface));
  }

  .window-dots {
    display: flex;
    gap: 5px;
  }

  .window-dots i {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    background: var(--border);
  }

  .window-dots i:first-child { background: #ff756b; }
  .window-dots i:nth-child(2) { background: #f7c44a; }
  .window-dots i:last-child { background: #5bc878; }

  .address-bar {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    color: var(--text-muted);
    background: var(--surface);
    font-size: 12px;
    font-weight: 750;
  }

  .address-bar span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .browser-actions {
    display: flex;
    gap: 3px;
  }

  button {
    display: grid;
    width: 28px;
    height: 28px;
    place-items: center;
    padding: 0;
    border: 0;
    border-radius: 7px;
    color: var(--text-muted);
    background: transparent;
    cursor: pointer;
  }

  button:hover,
  button:focus-visible {
    color: var(--brand);
    background: var(--surface-3);
    outline: none;
  }

  button:focus-visible {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--brand) 32%, transparent);
  }

  .frame-stage {
    position: relative;
    min-width: 0;
    min-height: 0;
    background: #fff;
  }

  iframe {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 360px;
    border: 0;
    background: #fff;
  }

  .compact iframe {
    min-height: 270px;
  }

  .preview-empty {
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 7px;
    min-height: 270px;
    padding: 28px;
    color: var(--text-muted);
    text-align: center;
    background:
      linear-gradient(90deg, transparent 23px, color-mix(in srgb, var(--border-2) 42%, transparent) 24px, transparent 25px),
      linear-gradient(transparent 23px, color-mix(in srgb, var(--border-2) 42%, transparent) 24px, transparent 25px);
    background-size: 24px 24px;
  }

  .preview-empty strong {
    color: var(--text-strong);
    font-size: 14px;
  }

  .preview-empty span {
    max-width: 310px;
    font-size: 12px;
  }
</style>
