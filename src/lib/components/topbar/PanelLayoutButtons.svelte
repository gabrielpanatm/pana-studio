<script lang="ts">
  import {
    IconLayoutBottombarCollapse,
    IconLayoutBottombarExpand,
    IconLayoutSidebarLeftCollapse,
    IconLayoutSidebarLeftExpand,
    IconLayoutSidebarRightCollapse,
    IconLayoutSidebarRightExpand,
  } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";
  import {
    legacyTranslator,
    localeRevision,
  } from "$lib/i18n/runtime.svelte";

  $: t = legacyTranslator($localeRevision);

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
    title={`${leftPaneCollapsed ? t("workbench-show-left-panel") : t("workbench-collapse-left-panel")} (Ctrl+B)`}
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
  title={`${terminalPaneOpen ? t("workbench-hide-terminal") : t("workbench-show-terminal")} (Ctrl+\`)`}
  active={terminalPaneOpen}
  segmented
  onclick={toggleTerminalPane}
>
  {#if terminalPaneOpen}
    <IconLayoutBottombarCollapse size={16} stroke={1.8} />
  {:else}
    <IconLayoutBottombarExpand size={16} stroke={1.8} />
  {/if}
</ToolbarButton>
{#if showSidebars}
  <ToolbarButton
    title={rightPaneCollapsed ? t("workbench-show-inspector") : t("workbench-collapse-inspector")}
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
