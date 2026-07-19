import {
  projectRelativeZolaPath,
  scannedCacheKey,
} from "$lib/project/files";
import { readProjectFile } from "$lib/project/io";
import { stageKernelPlannedSourceDraft } from "$lib/session/kernel-planned-draft";
import type { HtmlPendingArea, PageSection, SaveState } from "$lib/types";

export type HtmlMutationControllerHost = {
  sourceCache: Record<string, string>;
  activeScannedPath: string | null;
  source: string;
  pageSections: PageSection[];
  setPageSections?: (sections: PageSection[]) => void;
  setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

export async function stageKernelPlannedTemplateDraft(
  host: HtmlMutationControllerHost,
  tpl: { file: string; line: number },
  plannedSource: string,
  options: { pendingArea?: HtmlPendingArea; status?: string; isCurrent?: () => boolean } = {},
) {
  const relativePath = projectRelativeZolaPath(tpl.file);
  const cacheKey = scannedCacheKey({ relativePath });
  const source = host.sourceCache[cacheKey] ?? await readProjectFile(relativePath);
  if (options.isCurrent && !options.isCurrent()) return null;
  const updatedSource = plannedSource;
  if (options.isCurrent && !options.isCurrent()) return null;
  const base = source;
  stageKernelPlannedSourceDraft(host, relativePath, base, source, updatedSource, {
    detail: options.pendingArea ?? "template",
    label: "Kernel planned template draft",
    operation: "kernel.planned_template_draft",
  });
  host.setPageSections?.(host.pageSections);
  if (host.activeScannedPath === relativePath) host.source = updatedSource;
  if (options.pendingArea) host.setHtmlPending(options.pendingArea, true);
  if (options.status) host.setGlobalStatus(options.status, "unsaved");
  return updatedSource;
}
