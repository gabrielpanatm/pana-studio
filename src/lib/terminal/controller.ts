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
import {
  deriveTerminalScrollProxyGeometry,
  terminalLineFromProxyScroll,
} from "$lib/terminal/scroll-proxy";
import { t } from "$lib/i18n/runtime.svelte";

export class TerminalController {
  private sessions = new Map<string, TerminalSession>();
  private view: Terminal | null = null;
  private fitAddon: FitAddon | null = null;
  private resizeObserver: ResizeObserver | null = null;
  private inputSubscription: IDisposable | null = null;
  private renderedTabId: string | null = null;
  private renderedHost: HTMLDivElement | null = null;
  private scrollProxy: HTMLDivElement | null = null;
  private scrollProxyContent: HTMLDivElement | null = null;
  private scrollSubscription: IDisposable | null = null;
  private writeParsedSubscription: IDisposable | null = null;
  private terminalResizeSubscription: IDisposable | null = null;
  private scrollProxySyncFrame: number | null = null;
  private scrollProxyInputFrame: number | null = null;
  private scrollProxyReleaseFrame: number | null = null;
  private suppressScrollProxyInput = false;

  private readonly handleScrollProxyInput = () => {
    if (this.suppressScrollProxyInput || this.scrollProxyInputFrame !== null) return;
    this.scrollProxyInputFrame = window.requestAnimationFrame(() => {
      this.scrollProxyInputFrame = null;
      const terminal = this.view;
      const proxy = this.scrollProxy;
      if (!terminal || !proxy) return;

      const maxScrollTop = Math.max(0, proxy.scrollHeight - proxy.clientHeight);
      const targetLine = terminalLineFromProxyScroll(
        proxy.scrollTop,
        maxScrollTop,
        terminal.buffer.active.baseY,
      );
      if (targetLine !== terminal.buffer.active.viewportY) {
        terminal.scrollToLine(targetLine);
      }
    });
  };

  private scheduleScrollProxySync(): void {
    if (this.scrollProxySyncFrame !== null) return;
    this.scrollProxySyncFrame = window.requestAnimationFrame(() => {
      this.scrollProxySyncFrame = null;
      this.syncScrollProxy();
    });
  }

  private syncScrollProxy(): void {
    const terminal = this.view;
    const proxy = this.scrollProxy;
    const content = this.scrollProxyContent;
    if (!terminal || !proxy || !content) return;

    const geometry = deriveTerminalScrollProxyGeometry({
      viewportHeightPx: proxy.clientHeight,
      rows: terminal.rows,
      baseY: terminal.buffer.active.baseY,
      viewportY: terminal.buffer.active.viewportY,
    });

    this.suppressScrollProxyInput = true;
    content.style.height = `${geometry.contentHeightPx}px`;
    const maxScrollTop = Math.max(0, proxy.scrollHeight - proxy.clientHeight);
    const nextScrollTop = geometry.maxLine > 0
      ? terminal.buffer.active.viewportY / geometry.maxLine * maxScrollTop
      : 0;
    if (Math.abs(proxy.scrollTop - nextScrollTop) > 0.5) {
      proxy.scrollTop = nextScrollTop;
    }

    if (this.scrollProxyReleaseFrame !== null) {
      window.cancelAnimationFrame(this.scrollProxyReleaseFrame);
    }
    this.scrollProxyReleaseFrame = window.requestAnimationFrame(() => {
      this.scrollProxyReleaseFrame = null;
      this.suppressScrollProxyInput = false;
    });
  }

  private attachScrollProxy(host: HTMLDivElement, terminal: Terminal): void {
    const proxy = host.ownerDocument.createElement("div");
    proxy.className = "terminal-scroll-proxy";
    proxy.setAttribute("aria-hidden", "true");
    const content = host.ownerDocument.createElement("div");
    content.className = "terminal-scroll-proxy-content";
    proxy.appendChild(content);
    host.appendChild(proxy);

    this.scrollProxy = proxy;
    this.scrollProxyContent = content;
    proxy.addEventListener("scroll", this.handleScrollProxyInput, { passive: true });
    this.scrollSubscription = terminal.onScroll(() => this.scheduleScrollProxySync());
    this.writeParsedSubscription = terminal.onWriteParsed(() => this.scheduleScrollProxySync());
    this.terminalResizeSubscription = terminal.onResize(() => this.scheduleScrollProxySync());
    this.scheduleScrollProxySync();
  }

  private destroyScrollProxy(): void {
    this.scrollSubscription?.dispose();
    this.scrollSubscription = null;
    this.writeParsedSubscription?.dispose();
    this.writeParsedSubscription = null;
    this.terminalResizeSubscription?.dispose();
    this.terminalResizeSubscription = null;
    if (this.scrollProxySyncFrame !== null) {
      window.cancelAnimationFrame(this.scrollProxySyncFrame);
      this.scrollProxySyncFrame = null;
    }
    if (this.scrollProxyInputFrame !== null) {
      window.cancelAnimationFrame(this.scrollProxyInputFrame);
      this.scrollProxyInputFrame = null;
    }
    if (this.scrollProxyReleaseFrame !== null) {
      window.cancelAnimationFrame(this.scrollProxyReleaseFrame);
      this.scrollProxyReleaseFrame = null;
    }
    this.scrollProxy?.removeEventListener("scroll", this.handleScrollProxyInput);
    this.scrollProxy?.remove();
    this.scrollProxy = null;
    this.scrollProxyContent = null;
    this.suppressScrollProxyInput = false;
  }

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
    this.destroyScrollProxy();
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
      this.appendOutput(tab.id, `${t("terminal-session-started", { cwd })}\r\n`);
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
        `${t("terminal-shell-start-failed", {
          message: error instanceof Error ? error.message : String(error),
        })}\r\n`,
      );
      this.sessions.delete(tab.id);
    }
  }

  async render(options: {
    paneOpen: boolean;
    tab: TerminalTab | null;
    host: HTMLDivElement | undefined;
    theme: "dark" | "light";
    accent: string;
    cwd: string;
  }): Promise<void> {
    const { paneOpen, tab, host, theme, accent, cwd } = options;

    if (!paneOpen || !host || !tab || !cwd) {
      this.destroyRenderer();
      return;
    }

    await this.ensureSession(tab, cwd);
    const session = this.sessions.get(tab.id);
    if (!session) return;

    if (this.renderedTabId === tab.id && this.view && this.renderedHost === host) {
      const current = this.view;
      current.options.theme = createTerminalTheme(theme, accent);
      window.requestAnimationFrame(() => {
        this.fitAddon?.fit();
        if (session.pty) {
          const size = safeTerminalSize(current.cols, current.rows);
          session.pty.resize(size.cols, size.rows);
        }
          current.focus();
          current.textarea?.focus();
          this.scheduleScrollProxySync();
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
      fontFamily: "var(--font-mono)",
      fontSize: 13,
      lineHeight: 1.3,
      scrollback: 4000,
      smoothScrollDuration: window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? 0
        : 100,
      theme: createTerminalTheme(theme, accent),
    });
    const fitAddon = new FitAddon();

    term.open(host);
    term.element?.setAttribute("data-pana-wheel-smoothing", "native");
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());
    this.view = term;
    this.fitAddon = fitAddon;
    this.renderedTabId = tab.id;
    this.renderedHost = host;
    this.attachScrollProxy(host, term);

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
      this.scheduleScrollProxySync();
    });
    this.resizeObserver.observe(host);

    window.requestAnimationFrame(() => {
      fitAddon.fit();
      if (session.pty) {
        const size = safeTerminalSize(term.cols, term.rows);
        session.pty.resize(size.cols, size.rows);
      }
      this.scheduleScrollProxySync();
      term.focus();
      term.textarea?.focus();
      window.setTimeout(() => { term.focus(); term.textarea?.focus(); }, 50);
    });
  }
}
