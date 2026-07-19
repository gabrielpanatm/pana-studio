<script lang="ts">
  import { IconActivity, IconAlertTriangle, IconCircleCheck, IconRefresh } from "@tabler/icons-svelte";
  import { readKernelObservabilityLog } from "$lib/project/io";
  import type {
    KernelLogLevel,
    KernelObservabilityLogEvent,
    KernelObservabilityLogSnapshot,
    KernelObservabilityLogSourceFilter,
  } from "$lib/types";
  import {
    formatKernelLogTime,
    kernelLogAttributeEntries,
    kernelLogLevelLabel,
    kernelLogPathSummary,
    kernelLogSourceLabel,
    kernelLogTargetLabel,
    observabilityHealthDetail,
    observabilityHealthLabel,
    observabilitySummary,
  } from "$lib/kernel/observability-log-control";

  let {
    projectKey = "",
    refreshToken = 0,
    onStatusUpdate = undefined as ((text: string, kind: "restored" | "saving" | "error") => void) | undefined,
  }: {
    projectKey?: string;
    refreshToken?: number;
    onStatusUpdate?: (text: string, kind: "restored" | "saving" | "error") => void;
  } = $props();

  let snapshot = $state<KernelObservabilityLogSnapshot | null>(null);
  let loading = $state(false);
  let loadError = $state("");
  let activeProjectKey = $state("");
  let activeRefreshToken = $state<number | null>(null);
  let recoveryOnly = $state(true);
  let includeArchives = $state(false);
  let selectedLevels = $state<KernelLogLevel[]>(["info", "warn", "error"]);
  let sourceFilter = $state<KernelObservabilityLogSourceFilter>("all");
  let eventLimit = $state(80);
  const levels: KernelLogLevel[] = ["info", "warn", "error"];
  const summary = $derived(observabilitySummary(snapshot));

  $effect(() => {
    if (!projectKey) return;
    const projectChanged = projectKey !== activeProjectKey;
    const tokenChanged = refreshToken !== activeRefreshToken;
    if (!projectChanged && !tokenChanged) return;
    activeProjectKey = projectKey;
    activeRefreshToken = refreshToken;
    if (projectChanged) snapshot = null;
    void refresh();
  });

  async function refresh() {
    loading = true;
    loadError = "";
    try {
      snapshot = await readKernelObservabilityLog(
        eventLimit,
        recoveryOnly,
        includeArchives,
        selectedLevels,
        sourceFilter,
      );
    } catch (error) {
      loadError = error instanceof Error ? error.message : String(error);
      onStatusUpdate?.(`Observability Log nu a putut fi citit: ${loadError}`, "error");
    } finally {
      loading = false;
    }
  }

  function toggleLevel(level: KernelLogLevel) {
    selectedLevels = selectedLevels.includes(level)
      ? selectedLevels.filter((candidate) => candidate !== level)
      : [...selectedLevels, level];
    void refresh();
  }

  function attributes(event: KernelObservabilityLogEvent) {
    return kernelLogAttributeEntries(event);
  }
</script>

<section class="observability" aria-labelledby="observability-title">
  <header>
    <div class={`summary ${summary.tone}`}>
      {#if summary.tone === "clean"}<IconCircleCheck size={17} stroke={1.9} />{:else}<IconAlertTriangle size={17} stroke={1.9} />{/if}
      <div>
        <strong id="observability-title">{summary.label}</strong>
        <span>{loading ? "Se citește logul operațional..." : summary.detail}</span>
      </div>
    </div>
    <button type="button" disabled={loading} onclick={() => void refresh()} title="Recitește">
      <IconRefresh size={15} stroke={1.9} />
    </button>
  </header>

  <div class="filters">
    <label><input type="checkbox" bind:checked={recoveryOnly} onchange={() => void refresh()} /> doar recovery</label>
    <label><input type="checkbox" bind:checked={includeArchives} onchange={() => void refresh()} /> include arhive</label>
    {#each levels as level}
      <label><input type="checkbox" checked={selectedLevels.includes(level)} onchange={() => toggleLevel(level)} /> {kernelLogLevelLabel(level)}</label>
    {/each}
    <select bind:value={sourceFilter} onchange={() => void refresh()}>
      <option value="all">toate sursele</option>
      <option value="active">log activ</option>
      <option value="archives" disabled={!includeArchives}>arhive</option>
    </select>
    <select bind:value={eventLimit} onchange={() => void refresh()}>
      <option value={40}>40</option><option value={80}>80</option><option value={120}>120</option><option value={200}>200</option>
    </select>
  </div>

  {#if loadError}<p class="error" role="alert">{loadError}</p>{/if}

  {#if snapshot}
    <div class={`health ${snapshot.health.status}`}>
      <IconActivity size={16} stroke={1.8} />
      <strong>{observabilityHealthLabel(snapshot.health)}</strong>
      <span>{observabilityHealthDetail(snapshot.health)}</span>
    </div>
    <p class="path" title={snapshot.logPath}>{kernelLogPathSummary(snapshot)}</p>

    <div class="events">
      {#each snapshot.events as event (event.id)}
        <article class={event.level}>
          <header>
            <strong>{kernelLogLevelLabel(event.level)} · {event.owner} · {event.eventName}</strong>
            <time>{formatKernelLogTime(event.timestampMs)}</time>
          </header>
          <p>{event.message}</p>
          <small>{kernelLogTargetLabel(event)} · {kernelLogSourceLabel(event)}</small>
          {#if event.diagnostic}<p class="diagnostic">{event.diagnostic}</p>{/if}
          {#if attributes(event).length}
            <dl>
              {#each attributes(event) as [key, value]}
                <div><dt>{key}</dt><dd>{value}</dd></div>
              {/each}
            </dl>
          {/if}
        </article>
      {/each}
    </div>
  {/if}
</section>

<style>
  .observability { display: grid; gap: 10px; padding: 12px; border: 1px solid var(--border); border-radius: 9px; background: var(--surface-3); }
  header,
  .summary,
  .health { display: flex; align-items: center; }
  .observability > header { justify-content: space-between; gap: 10px; }
  .summary { gap: 8px; }
  .summary div { display: grid; gap: 3px; }
  .summary strong { font-size: 13px; }
  .summary span,
  .health span,
  .path,
  article p,
  article small { color: var(--text-muted); font-size: 11px; }
  button { width: 36px; height: 34px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface); color: var(--text); }
  .filters { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; color: var(--text-muted); font-size: 11px; }
  .health { gap: 7px; padding: 8px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface); }
  .health span { margin-left: auto; }
  .path { margin: 0; }
  .events { display: grid; gap: 7px; }
  article { display: grid; gap: 5px; padding: 9px; border: 1px solid var(--border); border-left-width: 3px; border-radius: 7px; background: var(--surface); }
  article.warn { border-left-color: #f59e0b; }
  article.error { border-left-color: #ef4444; }
  article.info { border-left-color: var(--brand-strong); }
  article header { justify-content: space-between; gap: 12px; }
  article strong { font-size: 11px; }
  article time { color: var(--text-muted); font-size: 10px; }
  article p { margin: 0; line-height: 1.4; }
  .diagnostic { color: #d97706; }
  dl { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 5px; margin: 0; }
  dl div { min-width: 0; padding: 5px; background: var(--surface-3); }
  dt { color: var(--text-muted); font-size: 9px; text-transform: uppercase; }
  dd { margin: 2px 0 0; overflow: hidden; font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
  .error { margin: 0; color: #ef4444; font-size: 11px; }
</style>
