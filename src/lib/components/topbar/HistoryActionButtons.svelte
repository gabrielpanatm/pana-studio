<script lang="ts">
  import {
    IconArrowBackUp,
    IconArrowForwardUp,
    IconDeviceFloppy,
  } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";
  import {
    legacyTranslator,
    localeRevision,
  } from "$lib/i18n/runtime.svelte";

  $: t = legacyTranslator($localeRevision);
  import { UI_TERM_IDS } from "$lib/i18n/ui-terms";

  export let canUndo = false;
  export let canRedo = false;
  export let inspectorHasPending = false;
  export let saveActiveFile: () => void | Promise<boolean>;
  export let undoAction: () => void | Promise<void>;
  export let redoAction: () => void | Promise<void>;
</script>

<ToolbarButton
  title={inspectorHasPending ? `${t(UI_TERM_IDS.save)} (Ctrl+S)` : t(UI_TERM_IDS.save)}
  pending={inspectorHasPending}
  onclick={saveActiveFile}
>
  <IconDeviceFloppy size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton title={`${t(UI_TERM_IDS.undo)} (Ctrl+Z)`} disabled={!canUndo} onclick={undoAction}>
  <IconArrowBackUp size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton title={`${t(UI_TERM_IDS.redo)} (Ctrl+Shift+Z)`} disabled={!canRedo} onclick={redoAction}>
  <IconArrowForwardUp size={17} stroke={1.8} />
</ToolbarButton>
