<script lang="ts">
  import {
    IconArrowBackUp,
    IconArrowForwardUp,
    IconColorSwatch,
    IconFrame,
    IconHandStop,
    IconMinus,
    IconNote,
    IconPencil,
    IconPhoto,
    IconPointer,
    IconPlus,
    IconRefresh,
    IconShape,
    IconTypography,
    IconWorld,
  } from "@tabler/icons-svelte";
  import type { MoodBoardSaveState, MoodBoardTool } from "$lib/mood-board/model";

  export let tool: MoodBoardTool = "select";
  export let zoom = 1;
  export let canUndo = false;
  export let canRedo = false;
  export let saveState: MoodBoardSaveState = "idle";
  export let saveStatus = "";
  export let setTool: (tool: MoodBoardTool) => void;
  export let undo: () => void;
  export let redo: () => void;
  export let zoomIn: () => void;
  export let zoomOut: () => void;
  export let fit: () => void;
  export let addNote: () => void;
  export let addText: () => void;
  export let addColor: () => void;
  export let addReference: () => void;
  export let addImage: () => void;
  export let addFrame: (preset?: "desktop" | "tablet" | "mobile" | "hero" | "section") => void;
  export let addShape: () => void;

  let frameMenuOpen = false;

  function chooseFrame(preset: "desktop" | "tablet" | "mobile" | "hero" | "section") {
    frameMenuOpen = false;
    addFrame(preset);
  }
</script>

<div class="canvas-dock" aria-label="Mood board tools">
  <div class="dock-group segmented">
    <button type="button" class:active={tool === "select"} title="Select" onclick={() => setTool("select")}>
      <IconPointer size={15} stroke={1.9} />
    </button>
    <button type="button" class:active={tool === "pan"} title="Pan" onclick={() => setTool("pan")}>
      <IconHandStop size={15} stroke={1.9} />
    </button>
  </div>

  <div class="dock-group">
    <button type="button" title="Adaugă notă" onclick={addNote}>
      <IconNote size={15} stroke={1.9} />
    </button>
    <button type="button" title="Adaugă text" onclick={addText}>
      <IconTypography size={15} stroke={1.9} />
    </button>
    <button type="button" title="Adaugă culoare" onclick={addColor}>
      <IconColorSwatch size={15} stroke={1.9} />
    </button>
    <button type="button" title="Adaugă referință" onclick={addReference}>
      <IconWorld size={15} stroke={1.9} />
    </button>
    <button type="button" title="Adaugă imagine" onclick={addImage}>
      <IconPhoto size={15} stroke={1.9} />
    </button>
    <div class="frame-menu-wrap">
      <button type="button" class:active={frameMenuOpen} title="Adaugă frame" onclick={() => (frameMenuOpen = !frameMenuOpen)}>
        <IconFrame size={15} stroke={1.9} />
      </button>
      {#if frameMenuOpen}
        <div class="frame-menu" role="menu" aria-label="Preset frame">
          <button type="button" role="menuitem" onclick={() => chooseFrame("desktop")}>Desktop 1440</button>
          <button type="button" role="menuitem" onclick={() => chooseFrame("tablet")}>Tablet</button>
          <button type="button" role="menuitem" onclick={() => chooseFrame("mobile")}>Mobile</button>
          <button type="button" role="menuitem" onclick={() => chooseFrame("hero")}>Hero</button>
          <button type="button" role="menuitem" onclick={() => chooseFrame("section")}>Section</button>
        </div>
      {/if}
    </div>
    <button type="button" title="Adaugă formă" onclick={addShape}>
      <IconShape size={15} stroke={1.9} />
    </button>
    <button type="button" class:active={tool === "pen"} title="Pen tool" onclick={() => setTool("pen")}>
      <IconPencil size={15} stroke={1.9} />
    </button>
  </div>

  <div class="dock-group">
    <button type="button" title="Undo canvas" disabled={!canUndo} onclick={undo}>
      <IconArrowBackUp size={15} stroke={1.9} />
    </button>
    <button type="button" title="Redo canvas" disabled={!canRedo} onclick={redo}>
      <IconArrowForwardUp size={15} stroke={1.9} />
    </button>
  </div>

  <div class="dock-group zoom-group">
    <button type="button" title="Zoom out" onclick={zoomOut}>
      <IconMinus size={15} stroke={1.9} />
    </button>
    <span>{Math.round(zoom * 100)}%</span>
    <button type="button" title="Zoom in" onclick={zoomIn}>
      <IconPlus size={15} stroke={1.9} />
    </button>
    <button type="button" title="Fit all" onclick={fit}>
      <IconRefresh size={15} stroke={1.9} />
    </button>
  </div>

  <span class:save-error={saveState === "error"} class:save-saving={saveState === "saving"} class="save-chip" title={saveStatus}>
    {saveState === "saving" ? "Se salvează" : saveState === "error" ? "Eroare" : "Salvat"}
  </span>
</div>

<style>
  .canvas-dock {
    position: absolute;
    left: 50%;
    bottom: 14px;
    z-index: 8;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 42px;
    max-width: calc(100% - 28px);
    padding: 6px;
    border: 1px solid var(--border-3);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.22);
    backdrop-filter: blur(10px);
    transform: translateX(-50%);
    overflow: visible;
  }

  .dock-group {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: 0 0 auto;
    padding: 2px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-3);
    overflow: visible;
  }

  .dock-group.segmented {
    gap: 0;
  }

  .frame-menu-wrap {
    position: relative;
    display: inline-grid;
    place-items: center;
  }

  .frame-menu {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 10px);
    z-index: 12;
    display: grid;
    gap: 3px;
    min-width: 128px;
    padding: 5px;
    border: 1px solid var(--border-3);
    border-radius: 9px;
    background: color-mix(in srgb, var(--surface) 96%, transparent);
    box-shadow: 0 14px 32px rgba(0, 0, 0, 0.22);
    backdrop-filter: blur(10px);
    transform: translateX(-50%);
    overflow: visible;
  }

  .frame-menu button {
    display: block;
    width: 100%;
    height: 26px;
    padding: 0 8px;
    text-align: left;
    white-space: nowrap;
    font-size: 11px;
    font-weight: 750;
  }

  button {
    display: inline-grid;
    place-items: center;
    width: 28px;
    height: 26px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--text-muted);
    background: transparent;
  }

  button:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--border-4);
    background: var(--surface-4);
  }

  button.active {
    color: #ffffff;
    border-color: var(--brand);
    background: var(--brand);
  }

  button:disabled {
    opacity: 0.36;
    cursor: not-allowed;
  }

  .zoom-group span,
  .save-chip {
    min-width: 48px;
    color: var(--text-muted);
    text-align: center;
    font-size: 11px;
    font-weight: 800;
  }

  .save-chip {
    flex: 0 0 96px;
    width: 96px;
    min-width: 96px;
    height: 28px;
    padding: 0 8px;
    overflow: hidden;
    border: 1px solid var(--border-3);
    border-radius: 999px;
    background: var(--surface-3);
    line-height: 26px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .save-saving {
    color: #6366f1;
  }

  .save-error {
    color: #cf4a4a;
  }
</style>
