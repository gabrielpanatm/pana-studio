<script lang="ts">
  import {
    IconBox,
    IconBraces,
    IconBrush,
    IconChevronRight,
    IconCommand,
    IconFile,
    IconFileText,
    IconLoader2,
    IconPhoto,
    IconSearch,
    IconShieldCheck,
  } from "@tabler/icons-svelte";
  import { tick } from "svelte";
  import {
    commandCenterQuery,
    searchCommandCenter,
  } from "$lib/workbench/command-center";
  import type {
    CommandCenterAction,
    CommandCenterItem,
    CommandCenterSearchResponse,
  } from "$lib/types";

  let {
    open = false,
    projectRoot = "",
    runtimeSessionId = "",
    close = () => {},
    execute = () => {},
  }: {
    open?: boolean;
    projectRoot?: string;
    runtimeSessionId?: string;
    close?: () => void;
    execute?: (action: CommandCenterAction) => void | Promise<void>;
  } = $props();

  let inputElement = $state<HTMLInputElement | null>(null);
  let inputValue = $state("");
  let response = $state<CommandCenterSearchResponse | null>(null);
  let selectedIndex = $state(0);
  let loading = $state(false);
  let executingId = $state<string | null>(null);
  let errorMessage = $state("");
  let requestSerial = 0;
  let wasOpen = false;
  const parsedQuery = $derived(commandCenterQuery(inputValue));
  const results = $derived(response?.results ?? []);
  const selectedItem = $derived(results[selectedIndex] ?? null);

  $effect(() => {
    if (open && !wasOpen) {
      inputValue = "";
      response = null;
      selectedIndex = 0;
      errorMessage = "";
      void tick().then(() => {
        inputElement?.focus();
        inputElement?.select();
      });
    }
    wasOpen = open;
  });

  $effect(() => {
    if (!open) return;
    const query = parsedQuery.query;
    const scope = parsedQuery.scope;
    const expectedProjectRoot = projectRoot || null;
    const expectedSessionId = runtimeSessionId || null;
    const serial = ++requestSerial;
    loading = true;
    errorMessage = "";
    const timer = window.setTimeout(() => {
      void searchCommandCenter({
        query,
        scope,
        projectRoot: expectedProjectRoot,
        runtimeSessionId: expectedSessionId,
      }).then((nextResponse) => {
        if (serial !== requestSerial || !open) return;
        response = nextResponse;
        selectedIndex = Math.min(selectedIndex, Math.max(0, nextResponse.results.length - 1));
      }).catch((error) => {
        if (serial !== requestSerial || !open) return;
        response = null;
        errorMessage = error instanceof Error ? error.message : String(error);
      }).finally(() => {
        if (serial === requestSerial) loading = false;
      });
    }, 70);
    return () => window.clearTimeout(timer);
  });

  function moveSelection(delta: number) {
    if (results.length === 0) return;
    selectedIndex = (selectedIndex + delta + results.length) % results.length;
    void tick().then(() => {
      document.getElementById(optionId(results[selectedIndex]))?.scrollIntoView({
        block: "nearest",
      });
    });
  }

  function handleInputKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveSelection(1);
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveSelection(-1);
      return;
    }
    if (event.key === "Home" && results.length > 0) {
      event.preventDefault();
      selectedIndex = 0;
      return;
    }
    if (event.key === "End" && results.length > 0) {
      event.preventDefault();
      selectedIndex = results.length - 1;
      return;
    }
    if (event.key === "Enter" && selectedItem) {
      event.preventDefault();
      void choose(selectedItem);
    }
  }

  async function choose(item: CommandCenterItem) {
    if (!item.enabled || executingId) return;
    executingId = item.id;
    errorMessage = "";
    try {
      await execute(item.action);
      close();
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      executingId = null;
    }
  }

  function optionId(item: CommandCenterItem) {
    return "command-center-" + item.id.replace(/[^a-zA-Z0-9_-]/g, "-");
  }
</script>

{#if open}
  <div
    class="command-center-backdrop"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) close();
    }}
  >
    <div
      class="command-center"
      role="dialog"
      aria-modal="true"
      aria-labelledby="command-center-title"
    >
      <h2 id="command-center-title">Command Center</h2>
      <div class="search-field">
        <span class="search-icon" aria-hidden="true">
          <IconSearch size={19} stroke={1.8} />
        </span>
        <span class="scope-chip">{parsedQuery.scopeLabel}</span>
        <input
          bind:this={inputElement}
          bind:value={inputValue}
          role="combobox"
          aria-autocomplete="list"
          aria-controls="command-center-results"
          aria-expanded="true"
          aria-activedescendant={selectedItem ? optionId(selectedItem) : undefined}
          aria-label="Caută comenzi, fișiere și simboluri"
          placeholder="Caută comenzi, fișiere și simboluri…"
          autocomplete="off"
          spellcheck="false"
          onkeydown={handleInputKeydown}
        />
        {#if loading}
          <IconLoader2 class="loading-icon" size={17} stroke={1.8} aria-label="Se caută" />
        {:else}
          <kbd>Esc</kbd>
        {/if}
      </div>

      <div
        id="command-center-results"
        class="results"
        role="listbox"
        aria-label="Rezultate Command Center"
      >
        {#if errorMessage}
          <div class="result-state error" role="alert">
            <IconShieldCheck size={18} stroke={1.8} />
            <span>{errorMessage}</span>
          </div>
        {:else if !loading && results.length === 0}
          <div class="result-state">
            <IconSearch size={18} stroke={1.8} />
            <span>Niciun rezultat pentru această căutare.</span>
          </div>
        {:else}
          {#each results as item, index (item.id)}
            <button
              id={optionId(item)}
              type="button"
              class:selected={selectedIndex === index}
              class:disabled={!item.enabled}
              role="option"
              aria-selected={selectedIndex === index ? "true" : "false"}
              disabled={!item.enabled || executingId !== null}
              title={item.disabledReason ?? item.subtitle}
              tabindex="-1"
              onmouseenter={() => { selectedIndex = index; }}
              onclick={() => { void choose(item); }}
            >
              <span class="result-icon" aria-hidden="true">
                {#if executingId === item.id}
                  <IconLoader2 class="loading-icon" size={17} stroke={1.8} />
                {:else if item.kind === "command"}
                  <IconCommand size={17} stroke={1.8} />
                {:else if item.kind === "activity"}
                  <IconChevronRight size={17} stroke={1.8} />
                {:else if item.kind === "page"}
                  <IconFileText size={17} stroke={1.8} />
                {:else if item.kind === "component"}
                  <IconBox size={17} stroke={1.8} />
                {:else if item.kind === "style"}
                  <IconBrush size={17} stroke={1.8} />
                {:else if item.kind === "asset"}
                  <IconPhoto size={17} stroke={1.8} />
                {:else if item.kind === "symbol"}
                  <IconBraces size={17} stroke={1.8} />
                {:else if item.kind === "diagnostic"}
                  <IconShieldCheck size={17} stroke={1.8} />
                {:else}
                  <IconFile size={17} stroke={1.8} />
                {/if}
              </span>
              <span class="result-copy">
                <strong>{item.title}</strong>
                <small>{item.disabledReason ?? item.subtitle}</small>
              </span>
              {#if item.shortcut}
                <kbd>{item.shortcut}</kbd>
              {:else}
                <span class="result-kind">{item.kind.replace("_", " ")}</span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>

      <footer>
        <span><kbd>↑↓</kbd> navigare</span>
        <span><kbd>Enter</kbd> deschide</span>
        <span><kbd>&gt;</kbd> comenzi</span>
        <span><kbd>#</kbd> fișiere</span>
        <span><kbd>@</kbd> simboluri</span>
        {#if response?.truncated}
          <span class="match-count">{response.totalMatches} rezultate</span>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .command-center-backdrop {
    position: fixed;
    z-index: 10000;
    inset: 0;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: min(11vh, 108px) 16px 16px;
    background: rgba(6, 10, 8, 0.46);
    backdrop-filter: blur(3px);
  }

  .command-center {
    width: min(720px, calc(100vw - 32px));
    max-height: min(680px, calc(100vh - 120px));
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--wb-accent) 26%, var(--wb-border-subtle));
    border-radius: 13px;
    color: var(--wb-text-primary);
    background: var(--wb-surface-document);
    box-shadow: 0 28px 90px rgba(0, 0, 0, 0.38);
  }

  h2 {
    position: fixed;
    overflow: hidden;
    width: 1px;
    height: 1px;
    margin: 0;
    clip: rect(0 0 0 0);
  }

  .search-field {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 56px;
    padding: 0 14px;
    border-bottom: 1px solid var(--wb-border-subtle);
    background: var(--wb-surface-chrome);
  }

  .search-icon {
    display: inline-flex;
    flex: 0 0 auto;
    color: var(--wb-accent-strong);
  }

  .scope-chip {
    flex: 0 0 auto;
    padding: 3px 7px;
    border: 1px solid color-mix(in srgb, var(--wb-accent) 32%, var(--wb-border-subtle));
    border-radius: 5px;
    color: var(--wb-accent-strong);
    font-size: 12px;
    font-weight: 750;
    background: var(--wb-accent-soft);
  }

  input {
    min-width: 0;
    flex: 1;
    padding: 0;
    border: 0;
    outline: 0;
    color: var(--wb-text-primary);
    font-size: 16px;
    background: transparent;
  }

  input::placeholder {
    color: var(--wb-text-muted);
  }

  kbd,
  .result-kind {
    display: inline-flex;
    align-items: center;
    min-height: 21px;
    padding: 1px 6px;
    border: 1px solid var(--wb-border-subtle);
    border-radius: 5px;
    color: var(--wb-text-muted);
    font-family: inherit;
    font-size: 12px;
    line-height: 1;
    background: var(--wb-surface-chrome);
    box-shadow: inset 0 -1px 0 color-mix(in srgb, var(--wb-border-subtle) 75%, transparent);
  }

  .results {
    min-height: 180px;
    max-height: min(540px, calc(100vh - 250px));
    padding: 6px;
    overflow-y: auto;
    background: var(--wb-surface-document);
  }

  .results button {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 48px;
    padding: 5px 9px 5px 6px;
    border: 1px solid transparent;
    border-radius: 8px;
    color: var(--wb-text-primary);
    text-align: left;
    background: transparent;
  }

  .results button:hover:not(:disabled),
  .results button.selected {
    border-color: color-mix(in srgb, var(--wb-accent) 24%, transparent);
    background: var(--wb-control-hover);
  }

  .results button.selected {
    box-shadow: inset 3px 0 0 var(--wb-accent);
  }

  .results button.disabled {
    opacity: 0.48;
  }

  .result-icon {
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    border: 1px solid var(--wb-border-subtle);
    border-radius: 7px;
    color: var(--wb-accent-strong);
    background: var(--wb-surface-chrome);
  }

  .result-copy {
    display: grid;
    gap: 3px;
    min-width: 0;
  }

  .result-copy strong,
  .result-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .result-copy strong {
    font-size: 12px;
    font-weight: 750;
  }

  .result-copy small {
    color: var(--wb-text-muted);
    font-size: 12px;
  }

  .result-kind {
    text-transform: capitalize;
    box-shadow: none;
  }

  .result-state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 180px;
    color: var(--wb-text-muted);
    font-size: 12px;
  }

  .result-state.error {
    color: #dc5f5f;
  }

  footer {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 34px;
    padding: 0 12px;
    border-top: 1px solid var(--wb-border-subtle);
    color: var(--wb-text-muted);
    font-size: 12px;
    background: var(--wb-surface-chrome);
  }

  footer span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .match-count {
    margin-left: auto;
  }

  :global(.loading-icon) {
    animation: command-center-spin 700ms linear infinite;
  }

  @keyframes command-center-spin {
    to { transform: rotate(360deg); }
  }

  @media (max-width: 640px) {
    .command-center-backdrop {
      padding: 8px;
    }

    .command-center {
      width: 100%;
      max-height: calc(100vh - 16px);
    }

    footer span:nth-child(n + 3) {
      display: none;
    }
  }
</style>
