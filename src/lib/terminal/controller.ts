import type { FitAddon } from "@xterm/addon-fit";
import type { Terminal } from "@xterm/xterm";
import type { IDisposable } from "tauri-pty";
import {
  createTerminalEnvironment,
  createTerminalShellArgs,
  terminalShellLauncher,
} from "$lib/terminal/environment";
import {
  appendTerminalChunk,
  createTerminalTheme,
  disposeTerminalSession,
  loadTerminalRuntime,
  safeTerminalSize,
  type TerminalSession,
  type TerminalTab,
} from "$lib/terminal/runtime";

export class TerminalController {
  private sessions = new Map<string, TerminalSession>();
  private view: Terminal | null = null;
  private fitAddon: FitAddon | null = null;
  private resizeObserver: ResizeObserver | null = null;
  private inputSubscription: IDisposable | null = null;
  private renderedTabId: string | null = null;
  private renderedHost: HTMLDivElement | null = null;

  appendOutput(tabId: string, chunk: string): void {
    if (!chunk) return;
    const session = this.sessions.get(tabId);
    if (!session) return;
    appendTerminalChunk(session, chunk);
    if (this.renderedTabId === tabId && this.view) {
      this.view.write(chunk);
    }
  }

  writeToSession(tabId: string, data: string): boolean {
    const session = this.sessions.get(tabId);
    if (!session?.pty) return false;
    session.pty.write(data);
    return true;
  }

  writeCommand(tabId: string, command: string): boolean {
    const normalizedCommand = command.trim();
    if (!normalizedCommand) return false;
    return this.writeToSession(tabId, `${normalizedCommand}\n`);
  }

  destroyRenderer(): void {
    if (this.renderedHost) {
      this.renderedHost.onclick = null;
      this.renderedHost.onmousedown = null;
    }
    this.inputSubscription?.dispose();
    this.inputSubscription = null;
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
    this.view?.dispose();
    this.view = null;
    this.fitAddon = null;
    this.renderedTabId = null;
    this.renderedHost = null;
  }

  destroySession(tabId: string): void {
    const session = this.sessions.get(tabId);
    if (!session) return;
    disposeTerminalSession(session);
    this.sessions.delete(tabId);
  }

  destroyAll(): void {
    this.destroyRenderer();
    for (const tabId of this.sessions.keys()) {
      this.destroySession(tabId);
    }
  }

  async ensureSession(tab: TerminalTab, cwd: string): Promise<void> {
    const existing = this.sessions.get(tab.id);
    if (existing?.pty) return;
    if (existing) this.destroySession(tab.id);

    const session: TerminalSession = {
      pty: null,
      buffer: "",
      decoder: new TextDecoder(),
      dataSubscription: null,
      exitSubscription: null,
    };
    this.sessions.set(tab.id, session);

    try {
      const { spawn } = await loadTerminalRuntime();
      const pty = spawn(terminalShellLauncher, createTerminalShellArgs(), {
        cols: 120,
        rows: 32,
        cwd,
        env: createTerminalEnvironment(),
      });
      session.pty = pty;
      this.appendOutput(tab.id, `Pană Studio terminal — cwd: ${cwd}\r\n`);
      session.dataSubscription = pty.onData((data) => {
        try {
          const bytes = data instanceof Uint8Array ? data : new Uint8Array(data as ArrayLike<number>);
          const chunk = session.decoder.decode(bytes, { stream: true });
          this.appendOutput(tab.id, chunk);
        } catch (error) {
          this.appendOutput(
            tab.id,
            `\r\n[pty decode error: ${error instanceof Error ? error.message : String(error)}]\r\n`,
          );
        }
      });
      session.exitSubscription = pty.onExit((event) => {
        const details = event.signal ? `, signal ${event.signal}` : "";
        this.appendOutput(tab.id, `\r\n[process exited: ${event.exitCode}${details}]\r\n`);
      });
    } catch (error) {
      this.appendOutput(
        tab.id,
        `Nu am putut porni sesiunea shell: ${error instanceof Error ? error.message : String(error)}\r\n`,
      );
      this.sessions.delete(tab.id);
    }
  }

  async render(options: {
    paneOpen: boolean;
    tab: TerminalTab | null;
    host: HTMLDivElement | undefined;
    theme: "dark" | "light";
    cwd: string;
  }): Promise<void> {
    const { paneOpen, tab, host, theme, cwd } = options;

    if (!paneOpen || !host || !tab || !cwd) {
      this.destroyRenderer();
      return;
    }

    await this.ensureSession(tab, cwd);
    const session = this.sessions.get(tab.id);
    if (!session) return;

    if (this.renderedTabId === tab.id && this.view && this.renderedHost === host) {
      const current = this.view;
      current.options.theme = createTerminalTheme(theme);
      window.requestAnimationFrame(() => {
        this.fitAddon?.fit();
        if (session.pty) {
          const size = safeTerminalSize(current.cols, current.rows);
          session.pty.resize(size.cols, size.rows);
        }
        current.focus();
        current.textarea?.focus();
      });
      return;
    }

    this.destroyRenderer();

    const { FitAddon, Terminal, WebLinksAddon } = await loadTerminalRuntime();
    const term = new Terminal({
      allowTransparency: true,
      convertEol: false,
      cursorBlink: true,
      cursorStyle: "block",
      fontFamily: '"JetBrains Mono", "SFMono-Regular", Consolas, monospace',
      fontSize: 13,
      lineHeight: 1.3,
      scrollback: 4000,
      theme: createTerminalTheme(theme),
    });
    const fitAddon = new FitAddon();

    term.open(host);
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());
    this.view = term;
    this.fitAddon = fitAddon;
    this.renderedTabId = tab.id;
    this.renderedHost = host;

    if (session.buffer.length) term.write(session.buffer);

    this.inputSubscription = term.onData((data) => {
      session.pty?.write(data);
    });
    host.onmousedown = () => { term.focus(); term.textarea?.focus(); };
    host.onclick = () => { term.focus(); term.textarea?.focus(); };
    this.resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
      if (session.pty) {
        const size = safeTerminalSize(term.cols, term.rows);
        session.pty.resize(size.cols, size.rows);
      }
    });
    this.resizeObserver.observe(host);

    window.requestAnimationFrame(() => {
      fitAddon.fit();
      if (session.pty) {
        const size = safeTerminalSize(term.cols, term.rows);
        session.pty.resize(size.cols, size.rows);
      }
      term.focus();
      term.textarea?.focus();
      window.setTimeout(() => { term.focus(); term.textarea?.focus(); }, 50);
    });
  }
}
