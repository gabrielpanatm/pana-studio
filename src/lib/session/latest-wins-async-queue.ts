export type LatestWinsAsyncQueueSnapshot = {
  pendingCount: number;
  inFlight: boolean;
  failureCount: number;
  enqueuedCount: number;
  executedCount: number;
  coalescedCount: number;
};

export type LatestWinsAsyncQueueOptions<Task> = {
  key: (task: Task) => string;
  run: (task: Task, context: { isCurrent: () => boolean }) => Promise<void>;
  merge?: (previous: Task, next: Task) => Task;
  delayMs?: number;
  delayMode?: "debounce" | "throttle";
  onError?: (error: unknown, task: Task) => void;
};

export type LatestWinsAsyncQueue<Task> = {
  enqueue: (task: Task) => void;
  flush: (options?: { throwOnFailure?: boolean; retryFailures?: boolean }) => Promise<void>;
  reset: () => void;
  snapshot: () => LatestWinsAsyncQueueSnapshot;
};

/**
 * A bounded, single-flight lane for interactive work.
 *
 * Repeated work for the same key is replaced before execution. If another
 * update arrives while that key is in flight, only the latest pending value is
 * executed afterwards. Errors are retained until a later successful run or an
 * explicit reset so Save/project transitions can fail closed.
 */
export function createLatestWinsAsyncQueue<Task>(
  options: LatestWinsAsyncQueueOptions<Task>,
): LatestWinsAsyncQueue<Task> {
  type QueuedTask = { task: Task; generation: number; keyRevision: number };
  type FailedTask = QueuedTask & { error: unknown };
  const pending = new Map<string, QueuedTask>();
  const failures = new Map<string, FailedTask>();
  const keyRevisions = new Map<string, number>();
  const delayMs = Math.max(0, Math.round(options.delayMs ?? 0));
  const delayMode = options.delayMode ?? "debounce";
  let timer: ReturnType<typeof setTimeout> | null = null;
  let drainPromise: Promise<void> | null = null;
  let enqueuedCount = 0;
  let executedCount = 0;
  let coalescedCount = 0;
  let generation = 0;
  let activeTask: { key: string; queued: QueuedTask } | null = null;

  function enqueue(task: Task) {
    const key = options.key(task).trim();
    if (!key) throw new Error("LatestWinsAsyncQueue a refuzat o cheie goală.");
    const previous = pending.get(key);
    const mergeBase = previous
      ?? (activeTask?.key === key && activeTask.queued.generation === generation
        ? activeTask.queued
        : undefined);
    const keyRevision = (keyRevisions.get(key) ?? 0) + 1;
    keyRevisions.set(key, keyRevision);
    if (previous !== undefined || (mergeBase !== undefined && options.merge)) coalescedCount += 1;
    pending.set(key, {
      task: mergeBase !== undefined && mergeBase.generation === generation && options.merge
        ? options.merge(mergeBase.task, task)
        : task,
      generation,
      keyRevision,
    });
    enqueuedCount += 1;
    failures.delete(key);
    scheduleDrain();
  }

  function scheduleDrain() {
    if (drainPromise) return;
    if (timer !== null) {
      if (delayMode === "throttle") return;
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = null;
      ensureDrain();
    }, delayMs);
  }

  function ensureDrain() {
    if (drainPromise || pending.size === 0) return;
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    const activeDrain = drain();
    drainPromise = activeDrain;
    void activeDrain.finally(() => {
      if (drainPromise === activeDrain) drainPromise = null;
      if (pending.size > 0) scheduleDrain();
    });
  }

  async function drain() {
    while (pending.size > 0) {
      const next = pending.entries().next().value as [string, QueuedTask] | undefined;
      if (!next) return;
      const [key, queued] = next;
      pending.delete(key);
      activeTask = { key, queued };
      const context = {
        isCurrent: () => queued.generation === generation && keyRevisions.get(key) === queued.keyRevision,
      };
      try {
        await options.run(queued.task, context);
        if (context.isCurrent()) failures.delete(key);
      } catch (error) {
        if (context.isCurrent()) {
          failures.set(key, { ...queued, error });
          options.onError?.(error, queued.task);
        }
      } finally {
        if (activeTask?.queued === queued) activeTask = null;
        if (queued.generation === generation) executedCount += 1;
      }
    }
  }

  async function flush(
    flushOptions: { throwOnFailure?: boolean; retryFailures?: boolean } = {},
  ) {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    if ((flushOptions.retryFailures ?? true) && failures.size > 0) {
      for (const [key, failed] of failures) {
        if (failed.generation === generation && !pending.has(key)) {
          pending.set(key, {
            task: failed.task,
            generation,
            keyRevision: failed.keyRevision,
          });
        }
      }
      failures.clear();
    }
    while (pending.size > 0 || drainPromise) {
      ensureDrain();
      const activeDrain = drainPromise;
      if (activeDrain) await activeDrain;
    }
    if ((flushOptions.throwOnFailure ?? true) && failures.size > 0) {
      const details = Array.from(failures.entries())
        .map(([key, failed]) => `${key}: ${failed.error instanceof Error ? failed.error.message : String(failed.error)}`)
        .join("; ");
      throw new Error(`Sincronizarea lucrărilor interactive a eșuat. ${details}`);
    }
  }

  function reset() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    generation += 1;
    pending.clear();
    failures.clear();
    keyRevisions.clear();
    enqueuedCount = 0;
    executedCount = 0;
    coalescedCount = 0;
  }

  function snapshot(): LatestWinsAsyncQueueSnapshot {
    return {
      pendingCount: pending.size,
      inFlight: drainPromise !== null,
      failureCount: failures.size,
      enqueuedCount,
      executedCount,
      coalescedCount,
    };
  }

  return { enqueue, flush, reset, snapshot };
}
