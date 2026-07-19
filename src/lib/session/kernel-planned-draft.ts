import { scannedCacheKey } from "$lib/project/files";
import { queueFileBufferDraftTextTransitionForPath } from "$lib/session/file-buffer-draft-sync";

export type KernelPlannedDraftHost = {
  sourceCache: Record<string, string>;
};

export type KernelPlannedDraftOptions = {
  detail: string;
  label: string;
  operation: string;
};

export function stageKernelPlannedSourceDraft(
  host: KernelPlannedDraftHost,
  relativePath: string,
  base: string,
  previous: string,
  plannedSource: string,
  options: KernelPlannedDraftOptions,
) {
  queueFileBufferDraftTextTransitionForPath(relativePath, previous, plannedSource, options.operation);
  host.sourceCache = {
    ...host.sourceCache,
    [scannedCacheKey({ relativePath })]: plannedSource,
  };
  return plannedSource;
}
