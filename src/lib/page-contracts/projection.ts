import { scannedCacheKey } from "$lib/project/files";
import {
  invalidateFileBufferDraftSyncCursor,
} from "$lib/session/file-buffer-draft-sync";
import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
import type {
  PageContractAuthorityReceipt,
  PageJsConfig,
  PageJsDraftStageReceipt,
  ProjectWorkspaceMutationReceipt,
} from "$lib/types";

export type PageContractProjectionHost = {
  sourceCache: Record<string, string>;
  activeScannedPath: string | null;
  source: string;
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  projectSessionEpoch: number;
};

export type PageContractSessionLease = {
  projectRoot: string;
  sessionId: string;
  projectSessionEpoch: number;
};

type ProjectedTextSnapshot = { known: boolean; text: string };

export type PageContractProjectionLease = PageContractSessionLease & {
  templatePath: string;
  texts: Record<string, ProjectedTextSnapshot>;
};

export type PageContractPlanProjection = {
  template: { changed: boolean; contents: string };
  stylesheet: { changed: boolean; contents: string };
  stylesheetPath: string;
  pageJsConfig: PageJsConfig;
};

export type PageContractApplyProjectionReceipt = {
  plan: PageContractPlanProjection;
  workspaceMutation: ProjectWorkspaceMutationReceipt | null;
  pageJs: PageJsDraftStageReceipt | null;
  authority: PageContractAuthorityReceipt;
};

export type PageContractProjectionResult = {
  retryRequired: false;
  projectedFiles: string[];
  preservedFiles: string[];
  pageJsConcurrent: boolean;
};

const pageContractLanes = new Map<string, Promise<void>>();

export function capturePageContractSessionLease(
  host: PageContractProjectionHost,
): PageContractSessionLease {
  const projectRoot = host.sessionProjectRoot.trim();
  const sessionId = host.kernelProjectSessionId.trim();
  if (!projectRoot || !sessionId) {
    throw new Error("Contractul paginii cere o sesiune de proiect activă și identificabilă.");
  }
  return { projectRoot, sessionId, projectSessionEpoch: host.projectSessionEpoch };
}

export function pageContractSessionLeaseMatches(
  host: PageContractProjectionHost,
  lease: PageContractSessionLease,
) {
  return host.sessionProjectRoot === lease.projectRoot
    && host.kernelProjectSessionId === lease.sessionId
    && host.projectSessionEpoch === lease.projectSessionEpoch;
}

export async function runInPageContractLane<T>(
  lease: PageContractSessionLease,
  _templatePath: string,
  operation: () => Promise<T>,
): Promise<T> {
  const key = `${lease.projectRoot}\u0000${lease.sessionId}`;
  const previous = pageContractLanes.get(key) ?? Promise.resolve();
  let release!: () => void;
  const current = new Promise<void>((resolve) => { release = resolve; });
  const tail = previous.catch(() => undefined).then(() => current);
  pageContractLanes.set(key, tail);
  await previous.catch(() => undefined);
  try {
    return await operation();
  } finally {
    release();
    if (pageContractLanes.get(key) === tail) pageContractLanes.delete(key);
  }
}

export async function flushPageContractDrafts() {
  await flushWorkspaceMutationInputs("manual");
}

export function capturePageContractProjectionLease(
  host: PageContractProjectionHost,
  sessionLease: PageContractSessionLease,
  templatePath: string,
  relativePaths: string[],
): PageContractProjectionLease {
  if (!pageContractSessionLeaseMatches(host, sessionLease)) {
    throw new Error("Page Contract a devenit stale înainte de capturarea proiecției.");
  }
  return {
    ...sessionLease,
    templatePath,
    texts: Object.fromEntries(
      [...new Set(relativePaths)].map((path) => [path, currentProjectedText(host, path)]),
    ),
  };
}

export function projectPageContractReceipt(
  host: PageContractProjectionHost,
  lease: PageContractProjectionLease,
  templateRelativePath: string,
  fallbackStylesheetPath: string,
  receipt: PageContractApplyProjectionReceipt,
): PageContractProjectionResult {
  requireCurrentPageContractReceipt(host, lease, receipt);
  const candidates = new Map<string, string>();
  if (receipt.plan.template.changed) {
    candidates.set(templateRelativePath, receipt.plan.template.contents);
  }
  if (receipt.plan.stylesheet.changed) {
    candidates.set(receipt.plan.stylesheetPath || fallbackStylesheetPath, receipt.plan.stylesheet.contents);
  }

  const projectedFiles: string[] = [];
  const preservedFiles: string[] = [];
  for (const [relativePath, contents] of candidates) {
    const captured = lease.texts[relativePath];
    if (captured && !sameProjectedText(captured, currentProjectedText(host, relativePath))) {
      preservedFiles.push(relativePath);
      invalidateFileBufferDraftSyncCursor(relativePath);
      continue;
    }
    host.sourceCache = {
      ...host.sourceCache,
      [scannedCacheKey({ relativePath })]: contents,
    };
    if (host.activeScannedPath === relativePath) host.source = contents;
    invalidateFileBufferDraftSyncCursor(relativePath);
    projectedFiles.push(relativePath);
  }

  const pageJsConcurrent = false;
  return {
    retryRequired: false,
    projectedFiles,
    preservedFiles,
    pageJsConcurrent,
  };
}

function requireCurrentPageContractReceipt(
  host: PageContractProjectionHost,
  lease: PageContractProjectionLease,
  receipt: PageContractApplyProjectionReceipt,
) {
  const authority = receipt.authority;
  if (!pageContractSessionLeaseMatches(host, lease)) {
    throw new Error("Page Contract a ignorat receipt-ul unei sesiuni înlocuite.");
  }
  if (authority.projectRoot !== lease.projectRoot || authority.sessionId !== lease.sessionId) {
    throw new Error("Page Contract a primit un receipt Rust din altă sesiune.");
  }
  if (
    authority.schemaVersion !== 2
    || !authority.operationId.trim()
    || !Number.isSafeInteger(authority.revisionBefore)
    || !Number.isSafeInteger(authority.revisionAfter)
    || authority.revisionAfter < authority.revisionBefore
  ) {
    throw new Error("Page Contract a primit un receipt revision/schema invalid.");
  }
  if (authority.status === "noop" && authority.revisionAfter !== authority.revisionBefore) {
    throw new Error("Page Contract noop nu poate avansa revizia.");
  }
  if (authority.status === "staged" && authority.revisionAfter === authority.revisionBefore) {
    throw new Error("Page Contract staged trebuie să avanseze revizia.");
  }
  if (receipt.workspaceMutation) {
    if (
      receipt.workspaceMutation.revisionBefore < authority.revisionBefore
      || receipt.workspaceMutation.revisionAfter > authority.revisionAfter
    ) {
      throw new Error("Page Contract a primit o mutație în afara intervalului autorității.");
    }
  }
}

function currentProjectedText(
  host: PageContractProjectionHost,
  relativePath: string,
): ProjectedTextSnapshot {
  if (host.activeScannedPath === relativePath) return { known: true, text: host.source };
  const key = scannedCacheKey({ relativePath });
  return Object.prototype.hasOwnProperty.call(host.sourceCache, key)
    ? { known: true, text: host.sourceCache[key] }
    : { known: false, text: "" };
}

function sameProjectedText(left: ProjectedTextSnapshot, right: ProjectedTextSnapshot) {
  return left.known === right.known && left.text === right.text;
}
