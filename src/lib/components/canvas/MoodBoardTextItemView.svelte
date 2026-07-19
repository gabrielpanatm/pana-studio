<script lang="ts">
  import { tick } from "svelte";
  import type { MoodBoardItem, MoodBoardTextItem } from "$lib/mood-board/model";
  import { moodBoardTextItemWithText } from "$lib/mood-board/item-content-actions";

  export let item: MoodBoardTextItem;
  export let beginEdit: () => void = () => undefined;
  export let previewEdit: (nextItem: MoodBoardItem) => void = () => undefined;
  export let commitEdit: (nextItem: MoodBoardItem) => void = () => undefined;
  export let cancelEdit: () => void = () => undefined;

  let textEditing = false;
  let textEditorEl: HTMLTextAreaElement | null = null;

  function beginDesignTextEdit(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    textEditing = true;
    beginEdit();
    tick().then(() => textEditorEl?.focus());
  }

  function updateDesignText(value: string) {
    previewEdit(moodBoardTextItemWithText(item, value));
  }

  function commitDesignText(value: string) {
    textEditing = false;
    commitEdit(moodBoardTextItemWithText(item, value));
  }

  function handleDesignTextKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.preventDefault();
    textEditing = false;
    cancelEdit();
    (event.currentTarget as HTMLTextAreaElement).blur();
  }
</script>

<div
  class="design-text-preview"
  role="textbox"
  tabindex="0"
  aria-label="Text canvas"
  style={`--text-color:${item.color};--text-size:${item.fontSize}px;--text-weight:${item.fontWeight};--text-align:${item.textAlign};`}
  onpointerdown={(event) => {
    if (event.detail >= 2) event.stopPropagation();
  }}
  ondblclick={beginDesignTextEdit}
>
  {#if textEditing}
    <textarea
      bind:this={textEditorEl}
      value={item.text}
      oninput={(event) => updateDesignText(event.currentTarget.value)}
      onblur={(event) => commitDesignText(event.currentTarget.value)}
      onkeydown={handleDesignTextKeydown}
    ></textarea>
  {:else}
    <span>{item.text || "Text"}</span>
  {/if}
</div>

<style>
  .design-text-preview {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    color: var(--text-color);
    font-size: var(--text-size);
    font-weight: var(--text-weight);
    text-align: var(--text-align);
    line-height: 1.08;
    white-space: pre-wrap;
    overflow: hidden;
  }

  .design-text-preview span {
    display: block;
  }

  .design-text-preview textarea {
    width: 100%;
    min-width: 0;
    min-height: 0;
    height: 100%;
    padding: 0;
    border: 1px solid transparent;
    color: inherit;
    background: color-mix(in srgb, var(--surface) 72%, transparent);
    font: inherit;
    line-height: inherit;
    resize: none;
    outline: none;
  }

  .design-text-preview textarea:focus {
    border-color: var(--border-4);
  }
</style>
