export type EditFlushReason = "save" | "history" | "snapshot" | "template-switch" | "unmount" | "manual";
export type EditFlushHandler = (reason: EditFlushReason) => void | Promise<void>;
export type EditFlushPending = () => boolean;

type EditFlushRegistration = Readonly<{
  handler: EditFlushHandler;
  pending: EditFlushPending;
}>;

const handlers = new Map<string, EditFlushRegistration>();

function registrationIsPending(registration: EditFlushRegistration) {
  try {
    return registration.pending();
  } catch {
    // A broken optimization hint must never allow an editor draft to be skipped.
    return true;
  }
}

export function registerEditFlushHandler(
  id: string,
  handler: EditFlushHandler,
  pending: EditFlushPending = () => true,
): () => void {
  const registration = { handler, pending };
  handlers.set(id, registration);
  return () => {
    if (handlers.get(id) === registration) handlers.delete(id);
  };
}

export function hasPendingRegisteredEditDrafts() {
  return Array.from(handlers.values()).some(registrationIsPending);
}

export async function flushRegisteredEditDrafts(reason: EditFlushReason) {
  const pendingHandlers = Array.from(handlers.values()).filter(registrationIsPending);
  for (const { handler } of pendingHandlers) {
    await handler(reason);
  }
}
