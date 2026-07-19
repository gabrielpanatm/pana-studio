import { invoke } from "@tauri-apps/api/core";

let resetQueued = false;
let lastResetAt = 0;

export const nativeZoomListenerOptions: AddEventListenerOptions = { capture: true, passive: false };

export function resetNativeWebviewZoom() {
  const now = Date.now();
  if (resetQueued || now - lastResetAt < 80) return;
  resetQueued = true;
  lastResetAt = now;

  requestAnimationFrame(() => {
    resetQueued = false;
    void invoke("reset_main_webview_zoom").catch((error) => {
      console.warn("[Pană Studio] Nu am putut reseta zoom-ul nativ WebView.", error);
    });
  });
}

export function preventNativeZoomWheel(event: WheelEvent) {
  if (!event.ctrlKey && !event.metaKey) return;
  event.preventDefault();
  resetNativeWebviewZoom();
}

export function preventNativeGestureZoom(event: Event) {
  event.preventDefault();
  resetNativeWebviewZoom();
}

export function resetNativeZoomIfVisualViewportChanged(visualViewport = window.visualViewport) {
  const scale = visualViewport?.scale ?? 1;
  if (Math.abs(scale - 1) > 0.001) resetNativeWebviewZoom();
}
