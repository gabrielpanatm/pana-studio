<script lang="ts">
  import type { MoodBoardColorItem, MoodBoardItem } from "$lib/mood-board/model";
  import { moodBoardColorWithLabel } from "$lib/mood-board/item-content-actions";

  export let item: MoodBoardColorItem;
  export let beginEdit: () => void = () => undefined;
  export let previewEdit: (nextItem: MoodBoardItem) => void = () => undefined;
  export let commitEdit: (nextItem: MoodBoardItem) => void = () => undefined;
  export let cancelEdit: () => void = () => undefined;

  function updateColorLabel(value: string) {
    previewEdit(moodBoardColorWithLabel(item, value));
  }

  function commitColorLabel(value: string) {
    commitEdit(moodBoardColorWithLabel(item, value));
  }

  function handleInputKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      commitColorLabel((event.currentTarget as HTMLInputElement).value);
      (event.currentTarget as HTMLInputElement).blur();
    } else if (event.key === "Escape") {
      event.preventDefault();
      cancelEdit();
      (event.currentTarget as HTMLInputElement).blur();
    }
  }
</script>

<div class="color-swatch" style={`--swatch:${item.color};`}></div>
<input
  class="item-title"
  value={item.label}
  placeholder="Nume culoare"
  onfocus={beginEdit}
  oninput={(event) => updateColorLabel(event.currentTarget.value)}
  onblur={(event) => commitColorLabel(event.currentTarget.value)}
  onkeydown={handleInputKeydown}
/>
<code>{item.color}</code>

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

  .color-swatch {
    display: block;
    min-height: 0;
    border: 1px solid color-mix(in srgb, #ffffff 22%, transparent);
    border-radius: 7px;
    background: var(--swatch);
    overflow: hidden;
  }

  code {
    color: var(--text-muted);
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    font-size: 11px;
  }
</style>
