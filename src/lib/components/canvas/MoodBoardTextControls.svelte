<script lang="ts">
  import {
    IconAlignCenter,
    IconAlignLeft,
    IconAlignRight,
    IconBold,
  } from "@tabler/icons-svelte";
  import {
    moodBoardTextWithAlign,
    moodBoardTextWithColor,
    moodBoardTextWithFontSize,
    moodBoardTextWithToggledWeight,
  } from "$lib/mood-board/context-actions";
  import { cloneMoodBoardItem } from "$lib/mood-board/item-view";
  import type { MoodBoardItem, MoodBoardTextItem } from "$lib/mood-board/model";

  export let textItem: MoodBoardTextItem;
  export let previewItem: (item: MoodBoardItem) => void;
  export let commitItemEdit: (beforeItem: MoodBoardItem, nextItem: MoodBoardItem) => void;

  let editBeforeItem: MoodBoardTextItem | null = null;

  function cloneItem(value: MoodBoardTextItem): MoodBoardTextItem {
    return cloneMoodBoardItem(value);
  }

  function beginEdit() {
    editBeforeItem ??= cloneItem(textItem);
  }

  function commitEdit(nextItem: MoodBoardTextItem) {
    if (!editBeforeItem) return;
    const before = editBeforeItem;
    editBeforeItem = null;
    commitItemEdit(before, nextItem);
  }

  function updateTextColor(value: string) {
    beginEdit();
    previewItem(moodBoardTextWithColor(textItem, value));
  }

  function commitTextColor(value: string) {
    commitEdit(moodBoardTextWithColor(textItem, value));
  }

  function updateTextFontSize(value: string) {
    beginEdit();
    previewItem(moodBoardTextWithFontSize(textItem, value));
  }

  function commitTextFontSize(value: string) {
    commitEdit(moodBoardTextWithFontSize(textItem, value));
  }

  function toggleTextWeight() {
    commitItemEdit(cloneItem(textItem), moodBoardTextWithToggledWeight(textItem));
  }

  function updateTextAlign(value: "left" | "center" | "right") {
    commitItemEdit(cloneItem(textItem), moodBoardTextWithAlign(textItem, value));
  }
</script>

<span class="separator"></span>
<label class="color-control" style={`--control-color:${textItem.color};`} title="Culoare text">
  <input
    type="color"
    value={textItem.color}
    aria-label="Culoare text"
    onfocus={beginEdit}
    oninput={(event) => updateTextColor(event.currentTarget.value)}
    onchange={(event) => commitTextColor(event.currentTarget.value)}
    onblur={(event) => commitTextColor(event.currentTarget.value)}
  />
</label>
<input
  class="number-control"
  type="number"
  min="10"
  max="120"
  step="1"
  value={textItem.fontSize}
  aria-label="Mărime text"
  onfocus={beginEdit}
  oninput={(event) => updateTextFontSize(event.currentTarget.value)}
  onblur={(event) => commitTextFontSize(event.currentTarget.value)}
/>
<button type="button" class:active={textItem.fontWeight >= 700} title="Bold" onclick={toggleTextWeight}>
  <IconBold size={15} stroke={2} />
</button>
<span class="separator compact"></span>
<button type="button" class:active={textItem.textAlign === "left"} title="Aliniază stânga" onclick={() => updateTextAlign("left")}>
  <IconAlignLeft size={15} stroke={2} />
</button>
<button type="button" class:active={textItem.textAlign === "center"} title="Aliniază centru" onclick={() => updateTextAlign("center")}>
  <IconAlignCenter size={15} stroke={2} />
</button>
<button type="button" class:active={textItem.textAlign === "right"} title="Aliniază dreapta" onclick={() => updateTextAlign("right")}>
  <IconAlignRight size={15} stroke={2} />
</button>
