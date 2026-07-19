<script lang="ts">
  import type { Snippet } from "svelte";
  import type { MoodBoardFrameItem, MoodBoardItem } from "$lib/mood-board/model";
  import { moodBoardFrameWithTitle } from "$lib/mood-board/item-content-actions";

  let {
    item,
    hasChildren = false,
    beginEdit = () => undefined,
    previewEdit = () => undefined,
    commitEdit = () => undefined,
    cancelEdit = () => undefined,
    children,
  }: {
    item: MoodBoardFrameItem;
    hasChildren?: boolean;
    beginEdit?: () => void;
    previewEdit?: (nextItem: MoodBoardItem) => void;
    commitEdit?: (nextItem: MoodBoardItem) => void;
    cancelEdit?: () => void;
    children?: Snippet;
  } = $props();

  function updateFrameTitle(value: string) {
    previewEdit(moodBoardFrameWithTitle(item, value));
  }

  function commitFrameTitle(value: string) {
    commitEdit(moodBoardFrameWithTitle(item, value));
  }

  function handleInputKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      commitFrameTitle((event.currentTarget as HTMLInputElement).value);
      (event.currentTarget as HTMLInputElement).blur();
    } else if (event.key === "Escape") {
      event.preventDefault();
      cancelEdit();
      (event.currentTarget as HTMLInputElement).blur();
    }
  }
</script>

<div class="frame-header">
  <input
    class="item-title"
    value={item.title}
    placeholder="Titlu frame"
    onfocus={beginEdit}
    oninput={(event) => updateFrameTitle(event.currentTarget.value)}
    onblur={(event) => commitFrameTitle(event.currentTarget.value)}
    onkeydown={handleInputKeydown}
  />
</div>
<div class="frame-body">
  <span>{Math.round(item.width)} × {Math.round(item.height)}</span>
</div>
{#if hasChildren && children}
  <div class="frame-children">
    {@render children()}
  </div>
{/if}

<style>
  input {
    min-width: 0;
    border: 1px solid transparent;
    color: var(--text);
    background: transparent;
    outline: none;
  }

  input:focus {
    border-color: var(--border-4);
    background: var(--surface-2);
  }

  .item-title {
    padding: 2px 24px 2px 2px;
    font-size: 13px;
    font-weight: 800;
  }

  .frame-header {
    position: relative;
    z-index: 3;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    align-items: center;
    min-height: 30px;
    padding: 4px 30px 4px 8px;
    border-bottom: 1px solid color-mix(in srgb, var(--frame-tone) 18%, var(--border-3));
    background: color-mix(in srgb, var(--frame-tone) 5%, #ffffff);
  }

  .frame-body {
    position: relative;
    z-index: 0;
    min-width: 0;
    min-height: 0;
    background: var(--frame-bg);
    pointer-events: none;
  }

  .frame-body span {
    position: absolute;
    right: 10px;
    bottom: 8px;
    color: color-mix(in srgb, var(--text-muted) 72%, transparent);
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    font-size: 11px;
    font-weight: 700;
  }

  .frame-children {
    position: absolute;
    inset: 0;
    z-index: 2;
    overflow: hidden;
    pointer-events: auto;
  }
</style>
