import {
  closeTerminalTabState,
  createTerminalTab,
  openTerminalTabState,
  type TerminalTab,
} from "$lib/terminal/runtime";

type TerminalSessionController = {
  destroySession: (tabId: string) => void;
};

export type TerminalTabsHost = {
  terminalPaneOpen: boolean;
  terminalTabs: TerminalTab[];
  activeTerminalTabId: string;
  terminalTabSerial: number;
  terminalController: TerminalSessionController;
};

export function initialTerminalTabs() {
  return [createTerminalTab(1)];
}

export function openTerminalTab(host: TerminalTabsHost) {
  const nextState = openTerminalTabState(host.terminalTabs, host.terminalTabSerial);
  host.terminalTabSerial = nextState.nextSerial;
  host.terminalTabs = nextState.tabs;
  host.activeTerminalTabId = nextState.activeTabId;
  host.terminalPaneOpen = true;
}

export function selectTerminalTab(host: TerminalTabsHost, tabId: string) {
  host.activeTerminalTabId = tabId;
  host.terminalPaneOpen = true;
}

export function closeTerminalTab(host: TerminalTabsHost, tabId: string) {
  host.terminalController.destroySession(tabId);
  const nextState = closeTerminalTabState(
    host.terminalTabs,
    host.activeTerminalTabId,
    host.terminalTabSerial,
    tabId,
  );
  host.terminalTabSerial = nextState.nextSerial;
  host.terminalTabs = nextState.tabs;
  host.activeTerminalTabId = nextState.activeTabId;
}
