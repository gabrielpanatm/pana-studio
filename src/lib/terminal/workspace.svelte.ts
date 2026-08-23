import { t } from "$lib/i18n/runtime.svelte";
import {
  zolaBuild,
} from "$lib/project/io/zola";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { TerminalController } from "$lib/terminal/controller";
import {
  closeTerminalTabState,
  createTerminalTab,
  openTerminalTabState,
  terminalQuickTasks,
  type TerminalQuickTask,
} from "$lib/terminal/runtime";
import { errorMessage } from "$lib/util";

export type TerminalWorkspaceCommands = {
  setPaneOpen: (open: boolean) => Promise<boolean>;
  currentProjectPath: () => string;
  runZolaValidation: (reason: "manual") => Promise<boolean>;
  openCurrentProjectInBrowser: () => Promise<void>;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

/** Owns all terminal UI state, sessions and user commands. */
export class TerminalWorkspaceState {
  terminalHost = $state<HTMLDivElement | undefined>(undefined);
  terminalPaneOpen = $state(false);
  terminalTabs = $state([createTerminalTab(1)]);
  activeTerminalTabId = $state("terminal-shell-1");
  terminalTabSerial = $state(1);
  appliedSessionRuntimeVersion = $state(0);
  readonly terminalQuickTasks = terminalQuickTasks;
  readonly terminalController = new TerminalController();

  private readonly commands: TerminalWorkspaceCommands;

  constructor(commands: TerminalWorkspaceCommands) {
    this.commands = commands;
  }

  get currentProjectPath() {
    return this.commands.currentProjectPath();
  }

  get activeTerminalTab() {
    return this.terminalTabs.find((tab) => tab.id === this.activeTerminalTabId)
      ?? this.terminalTabs[0]
      ?? null;
  }

  synchronizePaneOpen(open: boolean) {
    this.terminalPaneOpen = open;
  }

  async togglePane() {
    return await this.commands.setPaneOpen(!this.terminalPaneOpen);
  }

  async closePane() {
    return await this.commands.setPaneOpen(false);
  }

  async openTab() {
    if (!(await this.commands.setPaneOpen(true))) return;
    const next = openTerminalTabState(this.terminalTabs, this.terminalTabSerial);
    this.terminalTabSerial = next.nextSerial;
    this.terminalTabs = next.tabs;
    this.activeTerminalTabId = next.activeTabId;
  }

  async selectTab(tabId: string) {
    if (!(await this.commands.setPaneOpen(true))) return;
    this.activeTerminalTabId = tabId;
  }

  closeTab(tabId: string) {
    this.terminalController.destroySession(tabId);
    const next = closeTerminalTabState(
      this.terminalTabs,
      this.activeTerminalTabId,
      this.terminalTabSerial,
      tabId,
    );
    this.terminalTabSerial = next.nextSerial;
    this.terminalTabs = next.tabs;
    this.activeTerminalTabId = next.activeTabId;
  }

  async runQuickTask(task: TerminalQuickTask) {
    if (!(await this.commands.setPaneOpen(true))) return;
    const projectPath = this.commands.currentProjectPath();
    if (!projectPath) {
      this.commands.setGlobalStatus(t("workbench-terminal-project-required"), "error");
      return;
    }

    try {
      if (task.kind === "embedded-check") {
        await this.commands.runZolaValidation("manual");
      } else if (task.kind === "embedded-build") {
        this.commands.setGlobalStatus(t("workbench-terminal-building"), "saving");
        const log = await zolaBuild();
        this.commands.setGlobalStatus(
          log.split("\n")[0] || t("workbench-terminal-build-complete"),
          "saved",
        );
      } else {
        await this.commands.openCurrentProjectInBrowser();
      }
    } catch (error) {
      this.commands.setGlobalStatus(
        t("workbench-terminal-operation-failed", { error: errorMessage(error) }),
        "error",
      );
    }
  }

  async clearActive() {
    const projectPath = this.commands.currentProjectPath();
    const tab = this.activeTerminalTab;
    if (!projectPath || !tab) return;
    await this.terminalController.ensureSession(tab, projectPath);
    if (!this.terminalController.writeCommand(tab.id, "clear")) {
      this.commands.setGlobalStatus(t("workbench-terminal-not-ready"), "error");
    }
  }

  reset() {
    this.terminalController.destroyAll();
    this.terminalTabs = [createTerminalTab(1)];
    this.activeTerminalTabId = "terminal-shell-1";
    this.terminalTabSerial = 1;
    this.terminalPaneOpen = false;
    this.appliedSessionRuntimeVersion = 0;
  }

  destroy() {
    this.terminalController.destroyAll();
  }
}
