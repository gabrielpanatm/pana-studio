import { ReactiveEffectsLifecycle } from "$lib/lifecycle/reactive-effects.svelte";
import type { TerminalWorkspaceState } from "$lib/terminal/workspace.svelte";

const TERMINAL_SESSION_VERSION = 6;

export type TerminalAppearanceSource = {
  accent: string;
  theme: "dark" | "light";
};

/** Owns terminal rendering and terminal runtime-version invalidation. */
export class TerminalLifecycle {
  private readonly effects: ReactiveEffectsLifecycle;

  constructor(
    terminal: TerminalWorkspaceState,
    appearance: TerminalAppearanceSource,
  ) {
    this.effects = new ReactiveEffectsLifecycle([
      () => {
        void terminal.terminalController.render({
          paneOpen: terminal.terminalPaneOpen,
          tab: terminal.activeTerminalTab,
          host: terminal.terminalHost,
          theme: appearance.theme,
          accent: appearance.accent,
          cwd: terminal.currentProjectPath,
        });
      },
      () => {
        if (terminal.appliedSessionRuntimeVersion === TERMINAL_SESSION_VERSION) return;
        terminal.terminalController.destroyAll();
        terminal.appliedSessionRuntimeVersion = TERMINAL_SESSION_VERSION;
        if (terminal.terminalPaneOpen && terminal.terminalHost && terminal.activeTerminalTab) {
          void terminal.terminalController.render({
            paneOpen: terminal.terminalPaneOpen,
            tab: terminal.activeTerminalTab,
            host: terminal.terminalHost,
            theme: appearance.theme,
            accent: appearance.accent,
            cwd: terminal.currentProjectPath,
          });
        }
      },
    ]);
  }

  start() {
    return this.effects.start();
  }

  stop() {
    return this.effects.stop();
  }
}
