<script lang="ts">
  import {
    IconArrowBackUp,
    IconArrowForwardUp,
    IconDeviceFloppy,
  } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";
  import { UI_TERMS } from "$lib/i18n/ui-terms";

  export let canUndo = false;
  export let canRedo = false;
  export let inspectorHasPending = false;
  export let saveActiveFile: () => void | Promise<boolean>;
  export let undoAction: () => void | Promise<void>;
  export let redoAction: () => void | Promise<void>;
</script>

<ToolbarButton
  title={inspectorHasPending ? `${UI_TERMS.save} modificările (Ctrl+S)` : UI_TERMS.save}
  pending={inspectorHasPending}
  onclick={saveActiveFile}
>
  <IconDeviceFloppy size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton title={`${UI_TERMS.undo} (Ctrl+Z)`} disabled={!canUndo} onclick={undoAction}>
  <IconArrowBackUp size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton title={`${UI_TERMS.redo} (Ctrl+Shift+Z)`} disabled={!canRedo} onclick={redoAction}>
  <IconArrowForwardUp size={17} stroke={1.8} />
</ToolbarButton>
