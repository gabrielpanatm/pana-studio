import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AiCoordinationSnapshot } from "$lib/ai/contracts";

export const AI_COORDINATION_CHANGED_EVENT = "pana-ai-coordination-changed";

function isAiCoordinationSnapshot(value: unknown): value is AiCoordinationSnapshot {
  if (!value || typeof value !== "object") return false;
  const snapshot = value as Partial<AiCoordinationSnapshot>;
  if (
    snapshot.schemaVersion !== 2
    || !Number.isSafeInteger(snapshot.coordinationRevision)
    || (snapshot.coordinationRevision ?? -1) < 0
    || !snapshot.authority
    || typeof snapshot.authority !== "object"
    || !Array.isArray(snapshot.clients)
  ) return false;
  const authority = snapshot.authority as { state?: unknown };
  return typeof authority.state === "string";
}

export function subscribeAiCoordinationChanges(
  listener: (snapshot: AiCoordinationSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(AI_COORDINATION_CHANGED_EVENT, (event) => {
    if (!isAiCoordinationSnapshot(event.payload)) {
      console.error("[Pană Studio] AiCoordination event invalid", event.payload);
      return;
    }
    listener(event.payload);
  });
}
