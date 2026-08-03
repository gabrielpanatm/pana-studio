<script lang="ts">
  import {
    IconAlertTriangle,
    IconHierarchy3,
    IconLoader2,
    IconPalette,
    IconPointerBolt,
  } from "@tabler/icons-svelte";

  let {
    kind,
    title,
    description,
    codeToken = "",
    descriptionAfter = "",
    actionLabel = "",
    onAction = undefined,
    tone = "neutral",
    loading = false,
  }: {
    kind: "html" | "css" | "js";
    title: string;
    description: string;
    codeToken?: string;
    descriptionAfter?: string;
    actionLabel?: string;
    onAction?: () => void;
    tone?: "neutral" | "danger";
    loading?: boolean;
  } = $props();
</script>

<div
  class="inspector-empty-state"
  class:danger={tone === "danger"}
  role={tone === "danger" ? "alert" : "status"}
  aria-live={loading ? "polite" : undefined}
>
  <div class="empty-orbit" aria-hidden="true">
    <span class="orbit-mark"></span>
    <span class="empty-icon">
      {#if loading}
        <IconLoader2 class="loading-icon" size={21} stroke={1.8} />
      {:else if tone === "danger"}
        <IconAlertTriangle size={21} stroke={1.8} />
      {:else if kind === "html"}
        <IconHierarchy3 size={21} stroke={1.8} />
      {:else if kind === "css"}
        <IconPalette size={21} stroke={1.8} />
      {:else}
        <IconPointerBolt size={21} stroke={1.8} />
      {/if}
    </span>
  </div>

  <div class="empty-copy">
    <strong>{title}</strong>
    <p>
      {description}{#if codeToken} <code>{codeToken}</code>{/if}{#if descriptionAfter} {descriptionAfter}{/if}
    </p>
  </div>

  {#if actionLabel && onAction}
    <button class="ui-button secondary-action" type="button" onclick={onAction}>{actionLabel}</button>
  {/if}
</div>

<style>
  .inspector-empty-state {
    display: flex;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 250px;
    box-sizing: border-box;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 15px;
    padding: 30px 20px 42px;
    color: var(--text-muted);
    text-align: center;
    background: radial-gradient(circle at 50% 38%, color-mix(in srgb, var(--brand) 5%, transparent), transparent 42%);
  }

  .empty-orbit {
    position: relative;
    display: grid;
    width: 70px;
    height: 70px;
    place-items: center;
    border: 1px dashed color-mix(in srgb, var(--brand) 30%, var(--border));
    border-radius: 50%;
    box-shadow: inset 0 1px 2px color-mix(in srgb, var(--shadow-color, #314056) 8%, transparent);
  }

  .orbit-mark {
    position: absolute;
    top: -3px;
    width: 7px;
    height: 7px;
    border: 2px solid var(--material-panel);
    border-radius: 50%;
    background: var(--brand);
    box-shadow: 0 1px 2px color-mix(in srgb, var(--shadow-color, #314056) 20%, transparent);
  }

  .empty-icon {
    display: inline-flex;
    width: 42px;
    height: 42px;
    align-items: center;
    justify-content: center;
    border: 1px solid color-mix(in srgb, var(--brand) 26%, var(--border));
    border-radius: 11px;
    color: var(--brand-strong);
    background: var(--material-control);
    box-shadow: var(--shadow-control), inset 0 1px 0 var(--skeuo-edge-highlight);
  }

  .empty-copy {
    display: grid;
    max-width: 220px;
    gap: 5px;
  }

  .empty-copy strong {
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 780;
  }

  .empty-copy p {
    margin: 0;
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.5;
  }

  .empty-copy code {
    padding: 1px 4px;
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    color: var(--brand-strong);
    background: var(--code-bg);
    font-size: 11px;
  }

  .secondary-action {
    min-height: 29px;
    padding: 0 11px;
    color: var(--brand-strong);
    font-size: 11px;
    font-weight: 750;
  }

  .danger .empty-orbit {
    border-color: color-mix(in srgb, var(--danger) 38%, var(--border));
  }

  .danger .orbit-mark { background: var(--danger); }
  .danger .empty-icon {
    border-color: color-mix(in srgb, var(--danger) 28%, var(--border));
    color: var(--danger);
  }

  .danger .empty-copy p { color: color-mix(in srgb, var(--danger) 70%, var(--text-muted)); }

  :global(.loading-icon) { animation: empty-state-spin 900ms linear infinite; }

  @keyframes empty-state-spin { to { transform: rotate(360deg); } }

  @media (prefers-reduced-motion: reduce) {
    :global(.loading-icon) { animation: none; }
  }
</style>
