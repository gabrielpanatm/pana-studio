export type EditFlushReason = "save" | "history" | "snapshot" | "template-switch" | "unmount" | "manual";
export type EditFlushHandler = (reason: EditFlushReason) => void | Promise<void>;

const handlers = new Map<string, EditFlushHandler>();

export function registerEditFlushHandler(id: string, handler: EditFlushHandler): () => void {
  handlers.set(id, handler);
  return () => {
    if (handlers.get(id) === handler) handlers.delete(id);
  };
}

export async function flushRegisteredEditDrafts(reason: EditFlushReason) {
  const pendingHandlers = Array.from(handlers.values());
  for (const handler of pendingHandlers) {
    await handler(reason);
  }
}
