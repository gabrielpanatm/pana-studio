import { zolaBuild } from "$lib/project/io";
import { t } from "$lib/i18n/runtime.svelte";
import type { TerminalQuickTask, TerminalTab } from "$lib/terminal/runtime";
import type { GlobalStatusKind } from "$lib/status/global-status";
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
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

export async function runTerminalQuickTask(host: TerminalQuickTaskHost, task: TerminalQuickTask) {
  if (!host.currentProjectPath) {
    host.setGlobalStatus(t("workbench-terminal-project-required"), "error");
    return;
  }

  try {
    if (task.kind === "embedded-check") {
      await host.runZolaValidation("manual");
      return;
    }
    if (task.kind === "embedded-build") {
      host.setGlobalStatus(t("workbench-terminal-building"), "saving");
      const log = await zolaBuild();
      host.setGlobalStatus(log.split("\n")[0] || t("workbench-terminal-build-complete"), "saved");
      return;
    }
    await host.openCurrentProjectInBrowser();
  } catch (error) {
    host.setGlobalStatus(
      t("workbench-terminal-operation-failed", { error: errorMessage(error) }),
      "error",
    );
  }
}

export async function clearActiveTerminal(host: TerminalQuickTaskHost) {
  const tab = host.activeTerminalTab;
  if (!host.currentProjectPath || !tab) return;

  await host.terminalController.ensureSession(tab, host.currentProjectPath);
  const commandWritten = host.terminalController.writeCommand(tab.id, "clear");
  if (!commandWritten) {
    host.setGlobalStatus(t("workbench-terminal-not-ready"), "error");
  }
}
