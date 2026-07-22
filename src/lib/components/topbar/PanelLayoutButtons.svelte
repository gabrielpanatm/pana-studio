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
  export let showSidebars = true;
  export let toggleLeftPane: () => void;
  export let toggleTerminalPane: () => void;
  export let toggleRightPane: () => void | Promise<void>;
</script>

{#if showSidebars}
  <ToolbarButton
    title={leftPaneCollapsed ? "Arată panoul din stânga (Ctrl+B)" : "Restrânge panoul din stânga (Ctrl+B)"}
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
{/if}
<ToolbarButton
  title={terminalPaneOpen ? "Ascunde terminalul (Ctrl+`)" : "Arată terminalul (Ctrl+`)"}
  active={terminalPaneOpen}
  segmented
  onclick={toggleTerminalPane}
>
  <IconLayoutBottombar size={17} stroke={1.8} />
</ToolbarButton>
{#if showSidebars}
  <ToolbarButton
    title={rightPaneCollapsed ? "Arată Inspectorul" : "Restrânge Inspectorul"}
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
{/if}
