<script lang="ts">
  import { IconClipboard, IconCpu, IconDatabase, IconPlugConnected } from "@tabler/icons-svelte";
  import {
    configureCodexMcp,
    readAiContextStatus,
    readCodexMcpStatus,
  } from "$lib/project/io";
  import type { AiContextStatus, CodexMcpStatus } from "$lib/types";
  import { errorMessage, isRecoveryRequiredError } from "$lib/util";

  let {
    status = null,
    onStatusUpdate = undefined as ((text: string, kind: string) => void) | undefined,
  }: {
    status?: AiContextStatus | null;
    onStatusUpdate?: (text: string, kind: string) => void;
  } = $props();

  let localStatus = $state<AiContextStatus | null>(null);
  let codexStatus = $state<CodexMcpStatus | null>(null);
  let loading = $state(false);
  let codexLoading = $state(false);
  let codexRecoveryBlocked = $state(false);

  $effect(() => {
    if (status) localStatus = status;
  });

  $effect(() => {
    loadStatus();
  });

  async function loadStatus() {
    loading = true;
    try {
      const [aiStatus, codex] = await Promise.all([
        readAiContextStatus(),
        readCodexMcpStatus(),
      ]);
      localStatus = aiStatus;
      codexStatus = codex;
    } catch (error) {
      onStatusUpdate?.(`Nu am putut citi statusul AI/MCP: ${errorMessage(error)}`, "error");
    }
    loading = false;
  }

  async function copyValue(value: string, label: string) {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      onStatusUpdate?.(`${label} copiat.`, "saved");
    } catch {
      onStatusUpdate?.(`Nu am putut copia ${label}.`, "error");
    }
  }

  async function configureCodex() {
    codexLoading = true;
    try {
      codexStatus = await configureCodexMcp();
      codexRecoveryBlocked = false;
      onStatusUpdate?.("Codex CLI a fost configurat pentru Pană Studio MCP.", "saved");
    } catch (error) {
      codexRecoveryBlocked = isRecoveryRequiredError(error);
      onStatusUpdate?.(`Nu am putut configura Codex MCP: ${errorMessage(error)}`, "error");
    }
    codexLoading = false;
  }
</script>

<div class="ai-pane">
  <div class="ai-summary">
    <div class="summary-icon">
      <IconCpu size={18} stroke={1.9} />
    </div>
    <div>
      <h4>Context AI read-only</h4>
      <p>
        Pană Studio publică în RAM contextul canonic pentru agenții CLI. AI-ul
        îl citește prin MCP autentificat și editează direct fișierele sursă.
      </p>
    </div>
  </div>

  <div class="status-row">
    <span class:active={localStatus?.contextExists && localStatus?.serverRunning} class="status-dot"></span>
    <span>{loading ? "Se verifică..." : localStatus?.serverRunning ? "Context și server MCP active" : localStatus?.contextExists ? "Context disponibil, server MCP indisponibil" : "Contextul va apărea după primul snapshot"}</span>
  </div>

  <div class="field-list">
    <div class="field-card">
      <div class="field-label">
        <IconDatabase size={14} stroke={1.8} />
        <span>Descriptor lifecycle</span>
      </div>
      <code>{localStatus?.contextPath ?? "..."}</code>
      <button type="button" title="Copiază path" onclick={() => copyValue(localStatus?.contextPath ?? "", "Path context JSON")}>
        <IconClipboard size={14} stroke={1.9} />
      </button>
    </div>

    <div class="field-card">
      <div class="field-label">
        <IconPlugConnected size={14} stroke={1.8} />
        <span>Discovery</span>
      </div>
      <code>{localStatus?.discoveryPath ?? "..."}</code>
      <button type="button" title="Copiază path" onclick={() => copyValue(localStatus?.discoveryPath ?? "", "Path discovery MCP")}>
        <IconClipboard size={14} stroke={1.9} />
      </button>
    </div>

    <div class="field-card muted">
      <div class="field-label">
        <IconPlugConnected size={14} stroke={1.8} />
        <span>MCP HTTP</span>
      </div>
      <code>{localStatus?.endpoint ?? "http://127.0.0.1:48731/mcp"}</code>
      <button type="button" title="Copiază endpoint" onclick={() => copyValue(localStatus?.endpoint ?? "", "Endpoint MCP")}>
        <IconClipboard size={14} stroke={1.9} />
      </button>
    </div>
  </div>

  <p class="note">
    Datele și fișierele sunt read-only prin MCP; numai lease-ul de coordonare
    modifică stare volatilă în RAM. Fișierele locale sunt doar diagnostic și nu
    păstrează contextul UI complet după închiderea aplicației.
  </p>

  <div class="host-card">
    <div>
      <h4>Codex CLI</h4>
      <p>
        {codexStatus?.configured
          ? "Codex este configurat și autentificat pentru Pană Studio."
          : codexStatus?.configExists && !codexStatus.securePermissions
            ? "Configul Codex este accesibil altor utilizatori; setează permisiunile la 0600 înainte de instalarea tokenului."
          : codexStatus?.authenticated
            ? "Tokenul este instalat, dar configurația endpointului nu este completă."
            : "Adaugă endpointul și tokenul MCP în configul activ Codex."}
      </p>
      <code>{codexStatus?.configPath ?? "~/.codex/config.toml"}</code>
    </div>
    <button
      type="button"
      class="text-button"
      disabled={codexLoading || codexRecoveryBlocked || codexStatus?.configured || (codexStatus?.configExists && !codexStatus.securePermissions)}
      onclick={configureCodex}
    >
      {codexLoading
        ? "Se configurează..."
        : codexRecoveryBlocked
          ? "Recovery necesar"
          : codexStatus?.configExists && !codexStatus.securePermissions
            ? "Necesită 0600"
          : codexStatus?.configured
            ? "Configurat"
            : "Configurează"}
    </button>
  </div>
</div>

<style>
  .ai-pane {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .ai-summary {
    display: grid;
    grid-template-columns: 32px 1fr;
    gap: 10px;
    align-items: flex-start;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .summary-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: var(--surface-4);
    color: var(--brand);
  }

  h4,
  p {
    margin: 0;
  }

  h4 {
    font-size: 13px;
    font-weight: 850;
  }

  p,
  .note {
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 700;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    background: var(--border-4);
  }

  .status-dot.active {
    background: #54d18a;
    box-shadow: 0 0 0 3px rgba(84, 209, 138, 0.14);
  }

  .field-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .field-card {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 6px 8px;
    align-items: center;
    padding: 9px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-3);
  }

  .field-card.muted {
    opacity: 0.82;
  }

  .field-label {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 850;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  code {
    grid-column: 1 / 2;
    min-width: 0;
    overflow: hidden;
    color: var(--text);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button {
    grid-column: 2 / 3;
    grid-row: 1 / 3;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: var(--surface-4);
    color: var(--text-muted);
    cursor: pointer;
  }

  button:hover {
    border-color: var(--border-4);
    color: var(--text);
  }

  .host-card {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 10px;
    align-items: center;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .host-card code {
    display: block;
    margin-top: 6px;
  }

  .text-button {
    grid-column: auto;
    grid-row: auto;
    width: auto;
    min-width: 96px;
    padding: 0 10px;
    font-size: 12px;
    font-weight: 800;
  }

  .text-button:disabled {
    cursor: default;
    opacity: 0.62;
  }
</style>
