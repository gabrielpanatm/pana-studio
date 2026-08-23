import { flushSync } from "svelte";
import type { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";

const PROBE_PROPERTY = "__PANA_APPLICATION_COMPOSITION_RUNTIME__";

type RuntimeProbeOptions = {
  constructionStartedAt: number;
  constructionEndedAt: number;
  workspaceLayout: WorkspaceLayoutState;
};

type Distribution = {
  sampleCount: number;
  p50Ms: number;
  p95Ms: number;
  maxMs: number;
  samplesMs: number[];
};

type RuntimeSnapshot = {
  schemaVersion: 1;
  capturedAt: string;
  constructionMs: number;
  effectSettleFromConstructionStartMs: number | null;
  effectSettleFromConstructionEndMs: number | null;
  compositionModuleResources: Array<{
    module: string;
    startTimeMs: number;
    durationMs: number;
    responseEndMs: number;
    transferSizeBytes: number;
    decodedBodySizeBytes: number;
  }>;
  reactiveWorkspaceLayout: Distribution | null;
};

type RuntimeProbe = {
  read: () => RuntimeSnapshot;
  runReactiveUpdates: (sampleCount?: number) => Distribution;
};

function rounded(value: number) {
  return Math.round(value * 1_000) / 1_000;
}

function percentile(sorted: number[], ratio: number) {
  if (sorted.length === 0) return 0;
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil(sorted.length * ratio) - 1),
  );
  return sorted[index] ?? 0;
}

function distribution(samples: number[]): Distribution {
  const sorted = [...samples].sort((left, right) => left - right);
  return {
    sampleCount: sorted.length,
    p50Ms: rounded(percentile(sorted, 0.5)),
    p95Ms: rounded(percentile(sorted, 0.95)),
    maxMs: rounded(sorted.at(-1) ?? 0),
    samplesMs: samples.map(rounded),
  };
}

const COMPOSITION_MODULES = [
  "/src/lib/session/workspace-authority-service.ts",
  "/src/lib/project/transition-service.ts",
  "/src/lib/preview/runtime-service.ts",
] as const;

function compositionModuleResources() {
  const resources = performance.getEntriesByType("resource") as PerformanceResourceTiming[];
  return COMPOSITION_MODULES.flatMap((module) => {
    const resource = resources.find((entry) => entry.name.includes(module));
    return resource ? [{
      module,
      startTimeMs: rounded(resource.startTime),
      durationMs: rounded(resource.duration),
      responseEndMs: rounded(resource.responseEnd),
      transferSizeBytes: resource.transferSize,
      decodedBodySizeBytes: resource.decodedBodySize,
    }] : [];
  });
}

export function registerApplicationCompositionRuntimeProbe(options: RuntimeProbeOptions) {
  let settleAt: number | null = null;
  let reactiveWorkspaceLayout: Distribution | null = null;
  let firstAnimationFrame: number | null = null;
  let secondAnimationFrame: number | null = null;

  const snapshot = (): RuntimeSnapshot => ({
    schemaVersion: 1,
    capturedAt: new Date().toISOString(),
    constructionMs: rounded(
      options.constructionEndedAt - options.constructionStartedAt,
    ),
    effectSettleFromConstructionStartMs: settleAt === null
      ? null
      : rounded(settleAt - options.constructionStartedAt),
    effectSettleFromConstructionEndMs: settleAt === null
      ? null
      : rounded(settleAt - options.constructionEndedAt),
    compositionModuleResources: compositionModuleResources(),
    reactiveWorkspaceLayout,
  });

  const probe: RuntimeProbe = {
    read: snapshot,
    runReactiveUpdates(sampleCount = 50) {
      const iterations = Math.max(2, Math.round(sampleCount / 2) * 2);
      const original = options.workspaceLayout.leftPaneCollapsed;
      const samples: number[] = [];
      for (let index = 0; index < iterations; index += 1) {
        const startedAt = performance.now();
        options.workspaceLayout.toggleLeftPane();
        flushSync();
        samples.push(performance.now() - startedAt);
      }
      if (options.workspaceLayout.leftPaneCollapsed !== original) {
        options.workspaceLayout.leftPaneCollapsed = original;
        flushSync();
      }
      reactiveWorkspaceLayout = distribution(samples);
      return reactiveWorkspaceLayout;
    },
  };

  Object.defineProperty(window, PROBE_PROPERTY, {
    configurable: true,
    value: probe,
  });

  $effect(() => {
    firstAnimationFrame = window.requestAnimationFrame(() => {
      firstAnimationFrame = null;
      secondAnimationFrame = window.requestAnimationFrame(() => {
        secondAnimationFrame = null;
        settleAt = performance.now();
      });
    });

    return () => {
      if (firstAnimationFrame !== null) {
        window.cancelAnimationFrame(firstAnimationFrame);
        firstAnimationFrame = null;
      }
      if (secondAnimationFrame !== null) {
        window.cancelAnimationFrame(secondAnimationFrame);
        secondAnimationFrame = null;
      }
    };
  });

  return () => {
    if ((window as unknown as Record<string, unknown>)[PROBE_PROPERTY] === probe) {
      delete (window as unknown as Record<string, unknown>)[PROBE_PROPERTY];
    }
  };
}
