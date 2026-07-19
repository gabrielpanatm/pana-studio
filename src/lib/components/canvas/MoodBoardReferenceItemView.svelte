<script lang="ts">
  import type { MoodBoardItem, MoodBoardReferenceItem } from "$lib/mood-board/model";
  import {
    moodBoardReferenceWithNote,
    moodBoardReferenceWithTitle,
    moodBoardReferenceWithUrl,
  } from "$lib/mood-board/item-content-actions";

  export let item: MoodBoardReferenceItem;
  export let beginEdit: () => void = () => undefined;
  export let previewEdit: (nextItem: MoodBoardItem) => void = () => undefined;
  export let commitEdit: (nextItem: MoodBoardItem) => void = () => undefined;
  export let cancelEdit: () => void = () => undefined;

  function updateReferenceTitle(value: string) {
    previewEdit(moodBoardReferenceWithTitle(item, value));
  }

  function commitReferenceTitle(value: string) {
    commitEdit(moodBoardReferenceWithTitle(item, value));
  }

  function updateReferenceUrl(value: string) {
    previewEdit(moodBoardReferenceWithUrl(item, value));
  }

  function commitReferenceUrl(value: string) {
    commitEdit(moodBoardReferenceWithUrl(item, value));
  }

  function updateReferenceNote(value: string) {
    previewEdit(moodBoardReferenceWithNote(item, value));
  }

  function commitReferenceNote(value: string) {
    commitEdit(moodBoardReferenceWithNote(item, value));
  }

  function handleInputKeydown(event: KeyboardEvent, commitValue: (value: string) => void) {
    if (event.key === "Enter") {
      event.preventDefault();
      commitValue((event.currentTarget as HTMLInputElement).value);
      (event.currentTarget as HTMLInputElement).blur();
    } else if (event.key === "Escape") {
      event.preventDefault();
      cancelEdit();
      (event.currentTarget as HTMLInputElement).blur();
    }
  }

  function handleTextareaKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.preventDefault();
    cancelEdit();
    (event.currentTarget as HTMLTextAreaElement).blur();
  }
</script>

<input
  class="item-title"
  value={item.title}
  placeholder="Titlu referință"
  onfocus={beginEdit}
  oninput={(event) => updateReferenceTitle(event.currentTarget.value)}
  onblur={(event) => commitReferenceTitle(event.currentTarget.value)}
  onkeydown={(event) => handleInputKeydown(event, commitReferenceTitle)}
/>
<input
  class="item-url"
  value={item.url}
  placeholder="https://..."
  onfocus={beginEdit}
  oninput={(event) => updateReferenceUrl(event.currentTarget.value)}
  onblur={(event) => commitReferenceUrl(event.currentTarget.value)}
  onkeydown={(event) => handleInputKeydown(event, commitReferenceUrl)}
/>
<textarea
  value={item.note}
  placeholder="De ce contează?"
  onfocus={beginEdit}
  oninput={(event) => updateReferenceNote(event.currentTarget.value)}
  onblur={(event) => commitReferenceNote(event.currentTarget.value)}
  onkeydown={handleTextareaKeydown}
></textarea>

<style>
  input,
  textarea {
    min-width: 0;
    border: 1px solid transparent;
    color: var(--text);
    background: transparent;
    outline: none;
  }

  input:focus,
  textarea:focus {
    border-color: var(--border-4);
    background: var(--surface-2);
  }

  textarea {
    width: 100%;
    min-height: 0;
    height: 100%;
    resize: none;
    padding: 2px;
    font-size: 13px;
    line-height: 1.45;
  }

  .item-title {
    padding: 2px 24px 2px 2px;
    font-size: 13px;
    font-weight: 800;
  }

  .item-url {
    padding: 2px;
    color: var(--brand-strong);
    font-size: 12px;
  }
</style>
