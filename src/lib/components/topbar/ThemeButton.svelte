<script lang="ts">
  import { IconMoonStars, IconSun } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";

  type UiTheme = "dark" | "light";

  export let uiTheme: UiTheme = "dark";
  export let toggleUiTheme: () => void;
</script>

<ToolbarButton title={uiTheme === "dark" ? "Interfață luminoasă" : "Interfață întunecată"} onclick={toggleUiTheme}>
  {#if uiTheme === "dark"}
    <IconSun size={17} stroke={1.8} />
  {:else}
    <IconMoonStars size={17} stroke={1.8} />
  {/if}
</ToolbarButton>
