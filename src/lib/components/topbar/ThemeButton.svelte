<script lang="ts">
  import { IconMoonStars, IconSun } from "@tabler/icons-svelte";
  import ToolbarButton from "$lib/components/topbar/ToolbarButton.svelte";
  import {
    legacyTranslator,
    localeRevision,
  } from "$lib/i18n/runtime.svelte";

  $: t = legacyTranslator($localeRevision);

  type UiTheme = "dark" | "light";

  export let uiTheme: UiTheme = "dark";
  export let toggleUiTheme: () => void;
</script>

<ToolbarButton title={uiTheme === "dark" ? t("workbench-switch-light") : t("workbench-switch-dark")} onclick={toggleUiTheme}>
  {#if uiTheme === "dark"}
    <IconSun size={17} stroke={1.8} />
  {:else}
    <IconMoonStars size={17} stroke={1.8} />
  {/if}
</ToolbarButton>
