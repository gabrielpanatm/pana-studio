import type { IDisposable, IPty } from "tauri-pty";
import type { MessageId } from "$lib/i18n/generated/catalog";

export type TerminalTab = {
  id: string;
  index: number;
};

export type TerminalQuickTask = {
  id: string;
  labelId: MessageId;
  titleId: MessageId;
  kind: "embedded-check" | "embedded-build" | "internal-preview";
};

export type TerminalSession = {
  pty: IPty | null;
  buffer: string;
  decoder: TextDecoder;
  dataSubscription: IDisposable | null;
  exitSubscription: IDisposable | null;
};

export type TerminalRuntime = {
  FitAddon: typeof import("@xterm/addon-fit").FitAddon;
  Terminal: typeof import("@xterm/xterm").Terminal;
  WebLinksAddon: typeof import("@xterm/addon-web-links").WebLinksAddon;
  spawn: typeof import("tauri-pty").spawn;
};

export type TerminalTabsState = {
  tabs: TerminalTab[];
  activeTabId: string;
  nextSerial: number;
};

export const terminalBufferLimit = 160000;
export const defaultTerminalPaneHeight = 240;

export const terminalQuickTasks: TerminalQuickTask[] = [
  {
    id: "zola-check",
    labelId: "workbench-terminal-task-check",
    titleId: "workbench-terminal-task-check-title",
    kind: "embedded-check",
  },
  {
    id: "zola-build",
    labelId: "workbench-terminal-task-build",
    titleId: "workbench-terminal-task-build-title",
    kind: "embedded-build",
  },
  {
    id: "source-browser",
    labelId: "workbench-terminal-task-preview",
    titleId: "workbench-terminal-task-preview-title",
    kind: "internal-preview",
  },
];

let terminalRuntimePromise: Promise<TerminalRuntime> | null = null;

function normalizedTerminalAccent(accent: string) {
  return /^#[0-9a-f]{6}$/i.test(accent) ? accent.toLowerCase() : "#1d7f6a";
}

function terminalAccentWithAlpha(accent: string, alpha: number) {
  const normalized = normalizedTerminalAccent(accent);
  const red = Number.parseInt(normalized.slice(1, 3), 16);
  const green = Number.parseInt(normalized.slice(3, 5), 16);
  const blue = Number.parseInt(normalized.slice(5, 7), 16);
  return `rgba(${red}, ${green}, ${blue}, ${alpha})`;
}

export function createTerminalTheme(theme: "dark" | "light", accent = "#1d7f6a") {
  const normalizedAccent = normalizedTerminalAccent(accent);
  if (theme === "light") {
    return {
      background: "#f4f8f6",
      foreground: "#223029",
      cursor: normalizedAccent,
      cursorAccent: "#f4f8f6",
      selectionBackground: terminalAccentWithAlpha(normalizedAccent, 0.16),
      black: "#d2ddd7",
      brightBlack: "#6b7a73",
    };
  }

  return {
    background: "#121816",
    foreground: "#d7e3dd",
    cursor: normalizedAccent,
    cursorAccent: "#121816",
    selectionBackground: terminalAccentWithAlpha(normalizedAccent, 0.22),
    black: "#0f1412",
    brightBlack: "#697670",
  };
}

export function createTerminalTab(index: number): TerminalTab {
  return {
    id: `terminal-shell-${index}`,
    index,
  };
}

export async function loadTerminalRuntime(): Promise<TerminalRuntime> {
  if (!terminalRuntimePromise) {
    terminalRuntimePromise = Promise.all([
      import("@xterm/addon-fit"),
      import("@xterm/addon-web-links"),
      import("@xterm/xterm"),
      import("tauri-pty"),
    ]).then(([fit, webLinks, xterm, pty]) => ({
      FitAddon: fit.FitAddon,
      Terminal: xterm.Terminal,
      WebLinksAddon: webLinks.WebLinksAddon,
      spawn: pty.spawn,
    }));
  }

  return terminalRuntimePromise;
}

export function trimTerminalBuffer(buffer: string) {
  if (buffer.length <= terminalBufferLimit) {
    return buffer;
  }

  return buffer.slice(buffer.length - terminalBufferLimit);
}

export function safeTerminalSize(cols: number, rows: number) {
  return {
    cols: Math.max(20, Number.isFinite(cols) ? Math.floor(cols) : 0),
    rows: Math.max(6, Number.isFinite(rows) ? Math.floor(rows) : 0),
  };
}

export function appendTerminalChunk(session: TerminalSession, chunk: string) {
  if (!chunk) {
    return session.buffer;
  }

  session.buffer = trimTerminalBuffer(`${session.buffer}${chunk}`);
  return session.buffer;
}

export function disposeTerminalSession(session: TerminalSession) {
  session.dataSubscription?.dispose();
  session.exitSubscription?.dispose();

  try {
    session.pty?.kill();
  } catch {
    // ignore PTY shutdown errors during tab cleanup
  }
}

export function openTerminalTabState(tabs: TerminalTab[], currentSerial: number): TerminalTabsState {
  const nextSerial = currentSerial + 1;
  const nextTab = createTerminalTab(nextSerial);

  return {
    tabs: [...tabs, nextTab],
    activeTabId: nextTab.id,
    nextSerial,
  };
}

export function closeTerminalTabState(
  tabs: TerminalTab[],
  activeTabId: string,
  currentSerial: number,
  closedTabId: string,
): TerminalTabsState {
  const remainingTabs = tabs.filter((tab) => tab.id !== closedTabId);

  if (!remainingTabs.length) {
    const nextSerial = currentSerial + 1;
    const fallbackTab = createTerminalTab(nextSerial);

    return {
      tabs: [fallbackTab],
      activeTabId: fallbackTab.id,
      nextSerial,
    };
  }

  return {
    tabs: remainingTabs,
    activeTabId:
      activeTabId === closedTabId
        ? remainingTabs[Math.max(0, remainingTabs.length - 1)]?.id ?? remainingTabs[0].id
        : activeTabId,
    nextSerial: currentSerial,
  };
}
