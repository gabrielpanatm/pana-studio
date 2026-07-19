<script lang="ts">
  import {
    moodBoardFrameWithBackground,
    moodBoardFrameWithTone,
  } from "$lib/mood-board/context-actions";
  import { cloneMoodBoardItem } from "$lib/mood-board/item-view";
  import type { MoodBoardFrameItem, MoodBoardItem } from "$lib/mood-board/model";

  export let frameItem: MoodBoardFrameItem;
  export let previewItem: (item: MoodBoardItem) => void;
  export let commitItemEdit: (beforeItem: MoodBoardItem, nextItem: MoodBoardItem) => void;

  let editBeforeItem: MoodBoardFrameItem | null = null;

  function cloneItem(value: MoodBoardFrameItem): MoodBoardFrameItem {
    return cloneMoodBoardItem(value);
  }

  function beginEdit() {
    editBeforeItem ??= cloneItem(frameItem);
  }

  function commitEdit(nextItem: MoodBoardFrameItem) {
    if (!editBeforeItem) return;
    const before = editBeforeItem;
    editBeforeItem = null;
    commitItemEdit(before, nextItem);
  }

  function updateFrameTone(value: string) {
    beginEdit();
    previewItem(moodBoardFrameWithTone(frameItem, value));
  }

  function commitFrameTone(value: string) {
    commitEdit(moodBoardFrameWithTone(frameItem, value));
  }

  function updateFrameBackground(value: string) {
    beginEdit();
    previewItem(moodBoardFrameWithBackground(frameItem, value));
  }

  function commitFrameBackground(value: string) {
    commitEdit(moodBoardFrameWithBackground(frameItem, value));
  }
</script>

<span class="separator"></span>
<label class="color-control" style={`--control-color:${frameItem.tone};`} title="Culoare accent frame">
  <input
    type="color"
    value={frameItem.tone}
    aria-label="Culoare accent frame"
    onfocus={beginEdit}
    oninput={(event) => updateFrameTone(event.currentTarget.value)}
    onchange={(event) => commitFrameTone(event.currentTarget.value)}
    onblur={(event) => commitFrameTone(event.currentTarget.value)}
  />
</label>
<label class="color-control" style={`--control-color:${frameItem.background};`} title="Fundal frame">
  <input
    type="color"
    value={frameItem.background}
    aria-label="Fundal frame"
    onfocus={beginEdit}
    oninput={(event) => updateFrameBackground(event.currentTarget.value)}
    onchange={(event) => commitFrameBackground(event.currentTarget.value)}
    onblur={(event) => commitFrameBackground(event.currentTarget.value)}
  />
</label>
