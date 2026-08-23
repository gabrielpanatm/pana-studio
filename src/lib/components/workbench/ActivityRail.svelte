<script lang="ts">
  import {
    IconBox,
    IconBlocks,
    IconBrush,
    IconCodeDots,
    IconDatabase,
    IconFileText,
    IconForms,
    IconGitBranch,
    IconPhoto,
    IconRocket,
    IconSettings,
    IconShieldCheck,
    IconTags,
    IconTemplate,
    IconTerminal2,
  } from "@tabler/icons-svelte";
  import type { WorkbenchActivity } from "$lib/workbench/contracts";
  import { t } from "$lib/i18n/runtime.svelte";
  import { UI_TERM_IDS } from "$lib/i18n/ui-terms";

  type ActivityEntry = {
    id: WorkbenchActivity;
    label: string;
  };

  let {
    activeActivity = "editor",
    disabled = false,
    terminalOpen = false,
    settingsActive = false,
    selectActivity = () => {},
    toggleTerminal = () => {},
    selectSettings = () => {},
  }: {
    activeActivity?: WorkbenchActivity;
    disabled?: boolean;
    terminalOpen?: boolean;
    settingsActive?: boolean;
    selectActivity?: (activity: WorkbenchActivity) => void | Promise<void>;
    toggleTerminal?: () => void | Promise<void>;
    selectSettings?: () => void;
  } = $props();

  const activities = $derived<ActivityEntry[]>([
    { id: "editor", label: t(UI_TERM_IDS.editor) },
    { id: "templates", label: t(UI_TERM_IDS.templates) },
    { id: "components", label: t(UI_TERM_IDS.components) },
    { id: "blocks", label: t(UI_TERM_IDS.blocks) },
    { id: "design_system", label: t(UI_TERM_IDS.designSystem) },
    { id: "assets", label: t(UI_TERM_IDS.assets) },
    { id: "content", label: t(UI_TERM_IDS.content) },
    { id: "content_models", label: t(UI_TERM_IDS.contentModels) },
    { id: "taxonomies", label: t(UI_TERM_IDS.taxonomies) },
    { id: "data", label: t(UI_TERM_IDS.data) },
    { id: "versioning", label: t(UI_TERM_IDS.versionControl) },
    { id: "audit", label: t(UI_TERM_IDS.problemsAudit) },
    { id: "publish", label: t(UI_TERM_IDS.publish) },
  ]);
</script>

<nav class="activity-rail" aria-label={t("workbench-activities-label")}>
  <div class="activity-list">
    {#each activities as activity (activity.id)}
      <button
        type="button"
        class:active={!settingsActive && activeActivity === activity.id}
        disabled={disabled}
        aria-label={activity.label}
        aria-current={!settingsActive && activeActivity === activity.id ? "page" : undefined}
        title={activity.label}
        onclick={() => { void selectActivity(activity.id); }}
      >
        {#if activity.id === "editor"}
          <IconCodeDots size={19} stroke={1.8} />
        {:else if activity.id === "templates"}
          <IconTemplate size={19} stroke={1.8} />
        {:else if activity.id === "components"}
          <IconBox size={19} stroke={1.8} />
        {:else if activity.id === "blocks"}
          <IconBlocks size={19} stroke={1.8} />
        {:else if activity.id === "design_system"}
          <IconBrush size={19} stroke={1.8} />
        {:else if activity.id === "assets"}
          <IconPhoto size={19} stroke={1.8} />
        {:else if activity.id === "content"}
          <IconFileText size={19} stroke={1.8} />
        {:else if activity.id === "content_models"}
          <IconForms size={19} stroke={1.8} />
        {:else if activity.id === "taxonomies"}
          <IconTags size={19} stroke={1.8} />
        {:else if activity.id === "data"}
          <IconDatabase size={19} stroke={1.8} />
        {:else if activity.id === "versioning"}
          <IconGitBranch size={19} stroke={1.8} />
        {:else if activity.id === "audit"}
          <IconShieldCheck size={19} stroke={1.8} />
        {:else}
          <IconRocket size={19} stroke={1.8} />
        {/if}
        <span>{activity.label}</span>
      </button>
    {/each}
  </div>

  <div class="rail-utilities">
    <button
      type="button"
      class:active={terminalOpen}
      disabled={disabled}
      aria-label={t("workbench-terminal")}
      aria-pressed={terminalOpen}
      aria-keyshortcuts="Control+` Meta+`"
      title={`${t("workbench-terminal")} (Ctrl+\`)`}
      onclick={() => { void toggleTerminal(); }}
    >
      <IconTerminal2 size={19} stroke={1.8} />
      <span>{t("workbench-terminal")}</span>
    </button>
    <button
      type="button"
      class:active={settingsActive}
      aria-label={t(UI_TERM_IDS.settings)}
      aria-current={settingsActive ? "page" : undefined}
      title={t(UI_TERM_IDS.settings)}
      onclick={selectSettings}
    >
      <IconSettings size={19} stroke={1.8} />
      <span>{t(UI_TERM_IDS.settings)}</span>
    </button>
  </div>
</nav>

<style>
  .activity-rail {
    display: flex;
    align-items: center;
    flex-direction: column;
    width: var(--wb-activity-rail-width, 52px);
    min-width: var(--wb-activity-rail-width, 52px);
    min-height: 0;
    padding: 6px 5px;
    border-left: 1px solid var(--skeuo-edge-highlight);
    border-right: 1px solid var(--wb-border-subtle, var(--border));
    background: var(--material-panel);
    box-shadow: 1px 0 2px var(--skeuo-shade-soft);
  }

  .activity-list,
  .rail-utilities {
    display: grid;
    gap: 3px;
    width: 100%;
  }

  .activity-list {
    align-content: start;
    flex: 1;
    min-height: 0;
  }

  button {
    position: relative;
    display: grid;
    width: 40px;
    height: 40px;
    margin: 0 auto;
    place-items: center;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    color: var(--wb-text-muted);
    background: transparent;
    box-shadow: none;
  }

  button > span {
    position: fixed;
    overflow: hidden;
    width: 1px;
    height: 1px;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }

  button:hover:not(:disabled) {
    border-color: var(--border-subtle);
    color: var(--wb-text-primary);
    background: var(--material-control-hover);
    box-shadow: var(--shadow-control);
  }

  button.active {
    border-color: color-mix(in srgb, var(--brand) 38%, var(--border-subtle));
    color: var(--brand-strong);
    background: var(--material-control-selected);
    box-shadow: var(--shadow-pressed);
  }

  button.active::before {
    position: absolute;
    inset: 7px auto 7px -5px;
    width: 2px;
    border-radius: 0 2px 2px 0;
    background: var(--wb-accent);
    content: "";
  }

  button:focus-visible {
    outline: 2px solid var(--wb-focus-ring);
    outline-offset: -2px;
  }

  button:disabled {
    opacity: 0.36;
  }

  .rail-utilities {
    padding-top: 5px;
    border-top: 1px solid var(--wb-border-subtle);
    box-shadow: inset 0 1px 0 var(--skeuo-edge-highlight);
  }
</style>
