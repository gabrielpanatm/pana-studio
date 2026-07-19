import type { SaveState } from "$lib/types";

export type StatusControllerHost = {
  saveState: SaveState;
  saveStatus: string;
  statusDismissTimer: number | null;
};

export function clearStatusDismissTimer(host: StatusControllerHost) {
  if (host.statusDismissTimer === null) return;
  window.clearTimeout(host.statusDismissTimer);
  host.statusDismissTimer = null;
}

export function setGlobalStatus(host: StatusControllerHost, text: string, kind: SaveState) {
  clearStatusDismissTimer(host);
  host.saveState = kind;
  host.saveStatus = text;
  if (kind === "saved") {
    host.statusDismissTimer = window.setTimeout(() => {
      host.saveState = "idle";
      host.statusDismissTimer = null;
    }, 4000);
  }
}
