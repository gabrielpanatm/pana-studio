<script lang="ts">
  import {
    IconBox,
    IconBrush,
    IconCodeDots,
    IconFileText,
    IconGitBranch,
    IconPhoto,
    IconRocket,
    IconSettings,
    IconShieldCheck,
    IconSitemap,
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
    settingsOpen = false,
    selectActivity = () => {},
    toggleTerminal = () => {},
    toggleSettings = () => {},
  }: {
    activeActivity?: WorkbenchActivity;
    disabled?: boolean;
    terminalOpen?: boolean;
    settingsOpen?: boolean;
    selectActivity?: (activity: WorkbenchActivity) => void | Promise<void>;
    toggleTerminal?: () => void | Promise<void>;
    toggleSettings?: () => void;
  } = $props();

  const activities: ActivityEntry[] = [
    { id: "editor", label: UI_TERMS.editor },
    { id: "site", label: UI_TERMS.site },
    { id: "components", label: UI_TERMS.components },
    { id: "design_system", label: UI_TERMS.designSystem },
    { id: "assets", label: UI_TERMS.assets },
    { id: "content", label: UI_TERMS.content },
    { id: "versioning", label: UI_TERMS.versionControl },
    { id: "audit", label: UI_TERMS.problemsAudit },
    { id: "publish", label: UI_TERMS.publish },
  ];
</script>

<nav class="activity-rail" aria-label="Activități ale spațiului de lucru">
  <div class="product-mark" aria-label="Pană Studio">P</div>

  <div class="activity-list">
    {#each activities as activity (activity.id)}
      <button
        type="button"
        class:active={activeActivity === activity.id}
        disabled={disabled}
        aria-label={activity.label}
        aria-current={activeActivity === activity.id ? "page" : undefined}
        title={activity.label}
        onclick={() => { void selectActivity(activity.id); }}
      >
        {#if activity.id === "editor"}
          <IconCodeDots size={19} stroke={1.8} />
        {:else if activity.id === "site"}
          <IconSitemap size={19} stroke={1.8} />
        {:else if activity.id === "components"}
          <IconBox size={19} stroke={1.8} />
        {:else if activity.id === "design_system"}
          <IconBrush size={19} stroke={1.8} />
        {:else if activity.id === "assets"}
          <IconPhoto size={19} stroke={1.8} />
        {:else if activity.id === "content"}
          <IconFileText size={19} stroke={1.8} />
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
      class:active={settingsOpen}
      aria-label={UI_TERMS.settings}
      aria-pressed={settingsOpen}
      title={UI_TERMS.settings}
      onclick={toggleSettings}
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
    background: var(--wb-surface-chrome, var(--surface-2));
  }

  .product-mark {
    display: grid;
    width: 34px;
    height: 34px;
    margin: 1px 0 8px;
    place-items: center;
    border: 1px solid color-mix(in srgb, var(--wb-accent) 45%, var(--wb-border-subtle));
    border-radius: 9px;
    color: #fff;
    font-size: 14px;
    font-weight: 900;
    background: var(--wb-accent);
    box-shadow: 0 5px 16px color-mix(in srgb, var(--wb-accent) 24%, transparent);
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
    border-radius: 8px;
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
    background: var(--wb-control-hover);
  }

  button.active {
    color: var(--wb-accent-strong);
    background: var(--wb-accent-soft);
  }

  button.active::before {
    position: absolute;
    inset: 8px auto 8px -5px;
    width: 3px;
    border-radius: 0 3px 3px 0;
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
