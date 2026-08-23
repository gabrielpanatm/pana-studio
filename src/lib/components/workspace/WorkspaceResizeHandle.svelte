<script lang="ts">
  export let kind: "left" | "right" | "terminal" | "timeline";
  export let active = false;
  export let withBottomPanel = false;
  export let ariaLabel = "";
  export let onDrag: (event: PointerEvent) => void;
  export let onReset: () => void;
</script>

<button
  class="workspace-resize-handle"
  class:workspace-resize-handle-left={kind === "left"}
  class:workspace-resize-handle-right={kind === "right"}
  class:workspace-resize-handle-terminal={kind === "terminal"}
  class:workspace-resize-handle-timeline={kind === "timeline"}
  class:with-bottom-panel={kind === "timeline" && withBottomPanel}
  class:active
  type="button"
  aria-label={ariaLabel}
  onpointerdown={onDrag}
  ondblclick={onReset}
></button>

<style>
  .workspace-resize-handle {
    position: absolute;
    z-index: 20;
    min-width: 0;
    min-height: 0;
    padding: 0;
    border: 0;
    border-radius: 0;
    appearance: none;
    color: inherit;
    background: transparent;
    box-shadow: none;
    transition: none;
  }

  .workspace-resize-handle:hover:not(:disabled) {
    border: 0;
    color: inherit;
    background: transparent;
  }

  .workspace-resize-handle::after {
    content: "";
    position: absolute;
    inset: 0;
    margin: auto;
    border-radius: 999px;
    background: color-mix(in srgb, var(--brand) 42%, transparent);
    opacity: 0;
    transition: opacity 140ms ease, background 140ms ease;
  }

  .workspace-resize-handle:hover::after,
  .workspace-resize-handle:focus-visible::after,
  .workspace-resize-handle.active::after {
    opacity: 1;
  }

  .workspace-resize-handle:focus-visible {
    outline: none;
  }

  .workspace-resize-handle-left,
  .workspace-resize-handle-right {
    top: 16px;
    bottom: 16px;
    width: 12px;
    cursor: col-resize;
  }

  .workspace-resize-handle-left {
    left: calc(8px + var(--left-pane-width) + 4px - 6px);
  }

  .workspace-resize-handle-right {
    right: calc(8px + var(--right-pane-width) + 4px - 6px);
  }

  .workspace-resize-handle-left::after,
  .workspace-resize-handle-right::after {
    width: 4px;
    height: 100%;
  }

  .workspace-resize-handle-terminal,
  .workspace-resize-handle-timeline {
    left: 8px;
    right: 8px;
    height: 12px;
    cursor: row-resize;
    touch-action: none;
  }

  .workspace-resize-handle-terminal {
    bottom: calc(var(--terminal-pane-height) + 4px - 6px);
  }

  .workspace-resize-handle-timeline {
    z-index: 24;
    bottom: calc(var(--motion-timeline-height) + 4px - 6px);
  }

  .workspace-resize-handle-timeline.with-bottom-panel {
    bottom: calc(var(--terminal-pane-height) + var(--motion-timeline-height) + 12px - 6px);
  }

  .workspace-resize-handle-terminal::after,
  .workspace-resize-handle-timeline::after {
    width: 100%;
    height: 4px;
  }
</style>
