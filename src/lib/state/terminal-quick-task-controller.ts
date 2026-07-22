import { zolaBuild } from "$lib/project/io";
import type { TerminalQuickTask, TerminalTab } from "$lib/terminal/runtime";
import type { SaveState } from "$lib/types";
import { errorMessage } from "$lib/util";

type TerminalTaskController = {
  ensureSession: (tab: TerminalTab, cwd: string) => Promise<void>;
  writeCommand: (tabId: string, command: string) => boolean;
};

export type TerminalQuickTaskHost = {
  activeTerminalTab: TerminalTab | null;
  currentProjectPath: string;
  terminalController: TerminalTaskController;
  runZolaValidation: (reason: "manual") => Promise<boolean>;
  openCurrentProjectInBrowser: (route?: string | null) => Promise<void>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

export async function runTerminalQuickTask(host: TerminalQuickTaskHost, task: TerminalQuickTask) {
  if (!host.currentProjectPath) {
    host.setGlobalStatus("Deschide un proiect înainte de a rula o operație Zola.", "error");
    return;
  }

  try {
    if (task.kind === "embedded-check") {
      await host.runZolaValidation("manual");
      return;
    }
    if (task.kind === "embedded-build") {
      host.setGlobalStatus("Se construiește proiectul cu motorul Zola embedded...", "saving");
      const log = await zolaBuild();
      host.setGlobalStatus(log.split("\n")[0] || "Build Zola embedded finalizat.", "saved");
      return;
    }
    await host.openCurrentProjectInBrowser();
  } catch (error) {
    host.setGlobalStatus(`Operația Zola embedded a eșuat: ${errorMessage(error)}`, "error");
  }
}

export async function clearActiveTerminal(host: TerminalQuickTaskHost) {
  const tab = host.activeTerminalTab;
  if (!host.currentProjectPath || !tab) return;

  await host.terminalController.ensureSession(tab, host.currentProjectPath);
  const commandWritten = host.terminalController.writeCommand(tab.id, "clear");
  if (!commandWritten) {
    host.setGlobalStatus("Terminalul nu este pregătit încă.", "error");
  }
}
