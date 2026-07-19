<script lang="ts">
  import {
    IconArrowBackUp,
    IconArrowForwardUp,
    IconClock,
    IconDeviceFloppy,
  } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";

  export let canUndo = false;
  export let canRedo = false;
  export let inspectorHasPending = false;
  export let historyPanelOpen = false;
  export let saveActiveFile: () => void | Promise<boolean>;
  export let undoAction: () => void | Promise<void>;
  export let redoAction: () => void | Promise<void>;
  export let toggleHistoryPanel: () => void;
</script>

<ToolbarButton
  title={inspectorHasPending ? "Salvează modificări (Ctrl+S)" : "Save"}
  pending={inspectorHasPending}
  onclick={saveActiveFile}
>
  <IconDeviceFloppy size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton title="Undo (Ctrl+Z)" disabled={!canUndo} onclick={undoAction}>
  <IconArrowBackUp size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton title="Redo (Ctrl+Shift+Z)" disabled={!canRedo} onclick={redoAction}>
  <IconArrowForwardUp size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton title="History snapshots" active={historyPanelOpen} onclick={toggleHistoryPanel}>
  <IconClock size={17} stroke={1.8} />
</ToolbarButton>
