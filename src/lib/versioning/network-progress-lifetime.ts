export type VersionNetworkProgressLike = Readonly<{
  operationId: string;
  status: string;
}>;

export type VersionNetworkTimerScheduler = Readonly<{
  schedule: (callback: () => void, delayMs: number) => unknown;
  cancel: (handle: unknown) => void;
}>;

const TERMINAL_STATUSES = new Set(["completed", "failed", "cancelled"]);

/** Pure terminal-event lifetime with injectable timers for deterministic tests. */
export class VersionNetworkProgressLifetime<T extends VersionNetworkProgressLike> {
  private current: T | null = null;
  private terminalHandle: unknown = null;
  private readonly scheduler: VersionNetworkTimerScheduler;
  private readonly terminalDelayMs: number;

  constructor(
    scheduler: VersionNetworkTimerScheduler,
    terminalDelayMs = 2_500,
  ) {
    this.scheduler = scheduler;
    this.terminalDelayMs = terminalDelayMs;
  }

  receive(payload: T, publish: (value: T | null) => void) {
    this.cancelTerminal();
    this.current = payload;
    publish(payload);
    if (!TERMINAL_STATUSES.has(payload.status)) return;
    this.terminalHandle = this.scheduler.schedule(() => {
      this.terminalHandle = null;
      if (this.current?.operationId !== payload.operationId) return;
      this.current = null;
      publish(null);
    }, this.terminalDelayMs);
  }

  clear(publish: (value: T | null) => void) {
    this.cancelTerminal();
    this.current = null;
    publish(null);
  }

  private cancelTerminal() {
    if (this.terminalHandle !== null) this.scheduler.cancel(this.terminalHandle);
    this.terminalHandle = null;
  }
}
