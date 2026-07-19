<script lang="ts">
  import {
    IconLayoutBottombar,
    IconLayoutSidebarLeftCollapse,
    IconLayoutSidebarLeftExpand,
    IconLayoutSidebarRightCollapse,
    IconLayoutSidebarRightExpand,
  } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";

  export let leftPaneCollapsed = false;
  export let rightPaneCollapsed = false;
  export let terminalPaneOpen = false;
  export let toggleLeftPane: () => void;
  export let toggleTerminalPane: () => void;
  export let toggleRightPane: () => void | Promise<void>;
</script>

<ToolbarButton
  title={leftPaneCollapsed ? "Arata panoul din stanga" : "Restrange panoul din stanga"}
  active={!leftPaneCollapsed}
  segmented
  onclick={toggleLeftPane}
>
  {#if leftPaneCollapsed}
    <IconLayoutSidebarLeftExpand size={16} stroke={1.8} />
  {:else}
    <IconLayoutSidebarLeftCollapse size={16} stroke={1.8} />
  {/if}
</ToolbarButton>
<ToolbarButton
  title={terminalPaneOpen ? "Ascunde terminalul" : "Arata terminalul"}
  active={terminalPaneOpen}
  segmented
  onclick={toggleTerminalPane}
>
  <IconLayoutBottombar size={17} stroke={1.8} />
</ToolbarButton>
<ToolbarButton
  title={rightPaneCollapsed ? "Arata panoul din dreapta" : "Restrange panoul din dreapta"}
  active={!rightPaneCollapsed}
  segmented
  onclick={toggleRightPane}
>
  {#if rightPaneCollapsed}
    <IconLayoutSidebarRightExpand size={16} stroke={1.8} />
  {:else}
    <IconLayoutSidebarRightCollapse size={16} stroke={1.8} />
  {/if}
</ToolbarButton>
