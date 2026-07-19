<script lang="ts">
  import { IconSettings } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";

  export let settingsPanelOpen = false;
  export let toggleSettingsPanel: () => void;
</script>

<ToolbarButton title="Setări proiect" active={settingsPanelOpen} onclick={toggleSettingsPanel}>
  <IconSettings size={17} stroke={1.8} />
</ToolbarButton>
