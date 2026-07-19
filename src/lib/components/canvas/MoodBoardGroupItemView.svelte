<script lang="ts">
  import type { Snippet } from "svelte";
  import type { MoodBoardGroupItem, MoodBoardItem } from "$lib/mood-board/model";
  import { moodBoardGroupWithTitle } from "$lib/mood-board/item-content-actions";

  let {
    item,
    hasChildren = false,
    beginEdit = () => undefined,
    previewEdit = () => undefined,
    commitEdit = () => undefined,
    cancelEdit = () => undefined,
    children,
  }: {
    item: MoodBoardGroupItem;
    hasChildren?: boolean;
    beginEdit?: () => void;
    previewEdit?: (nextItem: MoodBoardItem) => void;
    commitEdit?: (nextItem: MoodBoardItem) => void;
    cancelEdit?: () => void;
    children?: Snippet;
  } = $props();

  function updateGroupTitle(value: string) {
    previewEdit(moodBoardGroupWithTitle(item, value));
  }

  function commitGroupTitle(value: string) {
    commitEdit(moodBoardGroupWithTitle(item, value));
  }

  function handleInputKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      commitGroupTitle((event.currentTarget as HTMLInputElement).value);
      (event.currentTarget as HTMLInputElement).blur();
    } else if (event.key === "Escape") {
      event.preventDefault();
      cancelEdit();
      (event.currentTarget as HTMLInputElement).blur();
    }
  }
</script>

<input
  class="group-title"
  value={item.title}
  placeholder="Grup"
  onfocus={beginEdit}
  oninput={(event) => updateGroupTitle(event.currentTarget.value)}
  onblur={(event) => commitGroupTitle(event.currentTarget.value)}
  onkeydown={handleInputKeydown}
/>
{#if hasChildren && children}
  <div class="group-children">
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

  .group-title {
    position: absolute;
    left: 6px;
    top: -22px;
    z-index: 14;
    max-width: min(180px, 100%);
    padding: 2px 5px;
    border: 1px solid color-mix(in srgb, var(--border-3) 80%, transparent);
    border-radius: 5px;
    color: var(--text-muted);
    background: color-mix(in srgb, var(--surface) 88%, transparent);
    font-size: 11px;
    font-weight: 800;
    opacity: 0;
    pointer-events: none;
  }

  :global(.group-item.selected) .group-title,
  :global(.group-item:focus-within) .group-title {
    opacity: 1;
    pointer-events: auto;
  }

  .group-children {
    position: absolute;
    inset: 0;
    z-index: 2;
    overflow: visible;
    pointer-events: auto;
  }
</style>
