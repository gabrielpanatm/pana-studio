<script lang="ts">
  import { IconGitBranch } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";

  export let versionsPanelOpen = false;
  export let disabled = false;
  export let toggleVersionsPanel: () => void;
</script>

<ToolbarButton
  title="Versiuni Git"
  active={versionsPanelOpen}
  {disabled}
  onclick={toggleVersionsPanel}
>
  <IconGitBranch size={17} stroke={1.8} />
</ToolbarButton>
