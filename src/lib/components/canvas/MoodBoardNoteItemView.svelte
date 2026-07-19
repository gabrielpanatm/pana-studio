<script lang="ts">
  import type { MoodBoardItem, MoodBoardNoteItem } from "$lib/mood-board/model";
  import { moodBoardNoteWithText } from "$lib/mood-board/item-content-actions";

  export let item: MoodBoardNoteItem;
  export let beginEdit: () => void = () => undefined;
  export let previewEdit: (nextItem: MoodBoardItem) => void = () => undefined;
  export let commitEdit: (nextItem: MoodBoardItem) => void = () => undefined;
  export let cancelEdit: () => void = () => undefined;

  function updateText(value: string) {
    previewEdit(moodBoardNoteWithText(item, value));
  }

  function commitText(value: string) {
    commitEdit(moodBoardNoteWithText(item, value));
  }

  function handleTextareaKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.preventDefault();
    cancelEdit();
    (event.currentTarget as HTMLTextAreaElement).blur();
  }
</script>

<textarea
  value={item.text}
  placeholder="Notă de mood board"
  onfocus={beginEdit}
  oninput={(event) => updateText(event.currentTarget.value)}
  onblur={(event) => commitText(event.currentTarget.value)}
  onkeydown={handleTextareaKeydown}
></textarea>

<style>
  textarea {
    width: 100%;
    min-width: 0;
    min-height: 0;
    height: 100%;
    resize: none;
    padding: 2px;
    border: 1px solid transparent;
    color: var(--text);
    background: transparent;
    font-size: 13px;
    line-height: 1.45;
    outline: none;
  }

  textarea:focus {
    border-color: var(--border-4);
    background: var(--surface-2);
  }
</style>
