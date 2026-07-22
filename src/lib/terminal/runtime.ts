import type { IDisposable, IPty } from "tauri-pty";

export type TerminalTab = {
  id: string;
  title: string;
  description: string;
};

export type TerminalQuickTask = {
  id: string;
  label: string;
  title: string;
  command: string;
  useBundledZola?: boolean;
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
    label: "Verificare",
    title: "Rulează zola check în proiectul curent",
    command: "check",
    useBundledZola: true,
  },
  {
    id: "zola-build",
    label: "Construire",
    title: "Rulează zola build în proiectul curent",
    command: "build",
    useBundledZola: true,
  },
  {
    id: "zola-serve",
    label: "Server",
    title: "Pornește zola serve în terminal",
    command: "serve",
    useBundledZola: true,
  },
];

let terminalRuntimePromise: Promise<TerminalRuntime> | null = null;

export function createTerminalTheme(theme: "dark" | "light") {
  if (theme === "light") {
    return {
      background: "#f4f8f6",
      foreground: "#223029",
      cursor: "#1d7f6a",
      cursorAccent: "#f4f8f6",
      selectionBackground: "rgba(29, 127, 106, 0.16)",
      black: "#d2ddd7",
      brightBlack: "#6b7a73",
    };
  }

  return {
    background: "#121816",
    foreground: "#d7e3dd",
    cursor: "#2faa8c",
    cursorAccent: "#121816",
    selectionBackground: "rgba(47, 170, 140, 0.22)",
    black: "#0f1412",
    brightBlack: "#697670",
  };
}

export function createTerminalTab(index: number): TerminalTab {
  return {
    id: `terminal-shell-${index}`,
    title: `Shell ${index}`,
    description: "Sesiune shell reala, pornita in radacina proiectului curent.",
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
