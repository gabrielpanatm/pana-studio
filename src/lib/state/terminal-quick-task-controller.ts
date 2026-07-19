import { getZolaBinaryPath } from "$lib/project/io";
import { shellQuote } from "$lib/state/app-helpers";
import type { TerminalQuickTask, TerminalTab } from "$lib/terminal/runtime";
import type { SaveState } from "$lib/types";

type TerminalTaskController = {
  ensureSession: (tab: TerminalTab, cwd: string) => Promise<void>;
  writeCommand: (tabId: string, command: string) => boolean;
};

export type TerminalQuickTaskHost = {
  terminalPaneOpen: boolean;
  activeTerminalTab: TerminalTab | null;
  currentProjectPath: string;
  zolaBinaryPath: string | null;
  terminalController: TerminalTaskController;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

export async function terminalCommandForTask(host: TerminalQuickTaskHost, task: TerminalQuickTask) {
  if (!task.useBundledZola) return task.command;
  if (!host.zolaBinaryPath) {
    host.zolaBinaryPath = await getZolaBinaryPath();
  }
  return `${shellQuote(host.zolaBinaryPath)} ${task.command}`;
}

export async function runTerminalQuickTask(host: TerminalQuickTaskHost, task: TerminalQuickTask) {
  if (!host.terminalPaneOpen) host.terminalPaneOpen = true;
  const tab = host.activeTerminalTab;
  if (!host.currentProjectPath) {
    host.setGlobalStatus("Deschide un proiect înainte de a rula task-uri în terminal.", "error");
    return;
  }
  if (!tab) return;

  await host.terminalController.ensureSession(tab, host.currentProjectPath);
  let command = "";
  try {
    command = await terminalCommandForTask(host, task);
  } catch (error) {
    host.setGlobalStatus(
      `Nu am putut pregăti task-ul terminal: ${error instanceof Error ? error.message : String(error)}`,
      "error",
    );
    return;
  }

  const commandWritten = host.terminalController.writeCommand(tab.id, command);
  if (!commandWritten) {
    host.setGlobalStatus("Terminalul nu este pregătit încă. Încearcă din nou după ce shell-ul pornește.", "error");
    return;
  }
  host.setGlobalStatus(`Terminal: ${task.label}`, "idle");
}

export async function clearActiveTerminal(host: TerminalQuickTaskHost) {
  const tab = host.activeTerminalTab;
  if (!host.currentProjectPath) return;
  if (!tab) return;

  await host.terminalController.ensureSession(tab, host.currentProjectPath);
  const commandWritten = host.terminalController.writeCommand(tab.id, "clear");
  if (!commandWritten) {
    host.setGlobalStatus("Terminalul nu este pregătit încă.", "error");
  }
}
