export const EXTERNAL_RECONCILE_BARRIER_EVENT = "pana:external-reconcile-barrier";

export function dispatchExternalReconcileInteractionBarrier() {
  if (typeof document !== "undefined" && document.activeElement instanceof HTMLElement) {
    document.activeElement.blur();
  }
  if (typeof window !== "undefined") {
    window.dispatchEvent(new Event(EXTERNAL_RECONCILE_BARRIER_EVENT));
  }
}

export function listenForExternalReconcileInteractionBarrier(cancel: () => void) {
  if (typeof window === "undefined") return () => {};
  window.addEventListener(EXTERNAL_RECONCILE_BARRIER_EVENT, cancel);
  return () => window.removeEventListener(EXTERNAL_RECONCILE_BARRIER_EVENT, cancel);
}
