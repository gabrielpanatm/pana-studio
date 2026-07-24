<script lang="ts">
  import {
    IconBox,
    IconBlocks,
    IconBrush,
    IconCodeDots,
    IconDatabase,
    IconFileText,
    IconGitBranch,
    IconPhoto,
    IconPalette,
    IconRocket,
    IconSettings,
    IconShieldCheck,
    IconTemplate,
    IconTerminal2,
  } from "@tabler/icons-svelte";
  import type { WorkbenchActivity } from "$lib/types";
  import { UI_TERMS } from "$lib/i18n/ui-terms";

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

  const activities: ActivityEntry[] = [
    { id: "editor", label: UI_TERMS.editor },
    { id: "themes", label: UI_TERMS.themes },
    { id: "templates", label: UI_TERMS.templates },
    { id: "components", label: UI_TERMS.components },
    { id: "blocks", label: UI_TERMS.blocks },
    { id: "design_system", label: UI_TERMS.designSystem },
    { id: "assets", label: UI_TERMS.assets },
    { id: "content", label: UI_TERMS.content },
    { id: "data", label: UI_TERMS.data },
    { id: "versioning", label: UI_TERMS.versionControl },
    { id: "audit", label: UI_TERMS.problemsAudit },
    { id: "publish", label: UI_TERMS.publish },
  ];
</script>

<nav class="activity-rail" aria-label="Activități ale spațiului de lucru">
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
        {:else if activity.id === "themes"}
          <IconPalette size={19} stroke={1.8} />
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
      aria-label="Terminal"
      aria-pressed={terminalOpen}
      aria-keyshortcuts="Control+` Meta+`"
      title="Terminal (Ctrl+`)"
      onclick={() => { void toggleTerminal(); }}
    >
      <IconTerminal2 size={19} stroke={1.8} />
      <span>Terminal</span>
    </button>
    <button
      type="button"
      class:active={settingsActive}
      aria-label={UI_TERMS.settings}
      aria-current={settingsActive ? "page" : undefined}
      title={UI_TERMS.settings}
      onclick={selectSettings}
    >
      <IconSettings size={19} stroke={1.8} />
      <span>{UI_TERMS.settings}</span>
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
    border-right: 1px solid var(--wb-border-subtle, var(--border));
    background: var(--surface-panel);
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
    border: 0;
    border-radius: var(--radius-control);
    color: var(--wb-text-muted);
    background: transparent;
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
    color: var(--wb-text-primary);
    background: var(--control-hover);
  }

  button.active {
    color: var(--brand-strong);
    background: var(--control-selected);
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
  }
</style>
