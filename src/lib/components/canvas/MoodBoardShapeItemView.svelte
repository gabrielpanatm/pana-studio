<script lang="ts">
  import type { MoodBoardShapeItem } from "$lib/mood-board/model";

  export let item: MoodBoardShapeItem;
</script>

<div class="shape-preview" class:ellipse={item.shape === "ellipse"} class:diamond={item.shape === "diamond"}></div>

<style>
  .shape-preview {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    border: var(--shape-stroke-width) solid var(--shape-stroke);
    border-radius: 7px;
    background: var(--shape-fill);
  }

  .shape-preview.ellipse {
    border-radius: 999px;
  }

  .shape-preview.diamond {
    width: 68%;
    height: 68%;
    place-self: center;
    transform: rotate(45deg);
  }
</style>
