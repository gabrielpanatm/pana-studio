import { clearPageJsDraft, stagePageJsDraft } from "$lib/js/io";
import { createLatestWinsAsyncQueue } from "$lib/session/latest-wins-async-queue";
import { normalizePageJsTemplatePath } from "$lib/js/page-path";
import type {
  PageJsDraftSessionIdentity,
  PageJsDraftStageInput,
  PageJsDraftStageReceipt,
} from "$lib/js/contracts";
import { t } from "$lib/i18n/runtime.svelte";

const PAGE_JS_DRAFT_SYNC_DELAY_MS = 180;

export type PageJsDraftSyncIdentity = PageJsDraftSessionIdentity & {
  generation: number;
};

export type PageJsDraftSyncTask =
  | {
      kind: "stage";
      templatePath: string;
      identity: PageJsDraftSyncIdentity;
      input: PageJsDraftStageInput;
    }
  | {
      kind: "clear";
      templatePath: string;
      identity: PageJsDraftSyncIdentity;
    };

export type PageJsDraftSyncTransport = {
  stage: (
    input: PageJsDraftStageInput,
    identity: PageJsDraftSessionIdentity,
  ) => Promise<PageJsDraftStageReceipt>;
  clear: (
    templatePath: string,
    identity: PageJsDraftSessionIdentity,
  ) => Promise<PageJsDraftStageReceipt>;
};

type PageJsDraftSyncIdentityGuard = (identity: PageJsDraftSyncIdentity) => boolean;

function transportIdentity(identity: PageJsDraftSyncIdentity): PageJsDraftSessionIdentity {
  return {
    expectedProjectRoot: identity.expectedProjectRoot,
    expectedSessionId: identity.expectedSessionId,
  };
}

function requireValidTaskIdentity(identity: PageJsDraftSyncIdentity) {
  if (
    !identity.expectedProjectRoot.trim()
    || !identity.expectedSessionId.trim()
    || !Number.isSafeInteger(identity.generation)
    || identity.generation < 1
  ) {
    throw new Error(t("page-js-draft-identity-invalid"));
  }
}

function requireMatchingReceipt(
  receipt: PageJsDraftStageReceipt,
  identity: PageJsDraftSyncIdentity,
) {
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(
      t("page-js-draft-receipt-session-mismatch", {
        expectedRoot: identity.expectedProjectRoot,
        expectedSession: identity.expectedSessionId,
        actualRoot: receipt.projectRoot,
        actualSession: receipt.runtimeSessionId,
      }),
    );
  }
}

export function createPageJsDraftSyncQueue(
  transport: PageJsDraftSyncTransport,
  delayMs = PAGE_JS_DRAFT_SYNC_DELAY_MS,
  isIdentityCurrent: PageJsDraftSyncIdentityGuard = () => true,
) {
  return createLatestWinsAsyncQueue<PageJsDraftSyncTask>({
    key: (task) => (
      `${task.identity.expectedProjectRoot}\u0000${task.identity.expectedSessionId}\u0000${task.identity.generation}\u0000${normalizePageJsTemplatePath(task.templatePath)}`
    ),
    delayMs,
    run: async (task, context) => {
      requireValidTaskIdentity(task.identity);
      if (!context.isCurrent() || !isIdentityCurrent(task.identity)) return;
      const templatePath = normalizePageJsTemplatePath(task.templatePath);
      if (!templatePath) return;
      const identity = transportIdentity(task.identity);
      const receipt = task.kind === "stage"
        ? await transport.stage({ ...task.input, templatePath }, identity)
        : await transport.clear(templatePath, identity);
      if (!context.isCurrent() || !isIdentityCurrent(task.identity)) {
        return;
      }
      requireMatchingReceipt(receipt, task.identity);
    },
    onError: (error, task) => {
      console.warn("[Pană Studio] Page JS kernel draft sync failed", task.templatePath, error);
    },
  });
}

let activeIdentity: PageJsDraftSyncIdentity | null = null;
let identityGeneration = 0;

function isActiveIdentity(identity: PageJsDraftSyncIdentity) {
  return activeIdentity !== null
    && activeIdentity.generation === identity.generation
    && activeIdentity.expectedProjectRoot === identity.expectedProjectRoot
    && activeIdentity.expectedSessionId === identity.expectedSessionId;
}

const pageJsDraftSync = createPageJsDraftSyncQueue({
  stage: (input, identity) => stagePageJsDraft(input, identity),
  clear: (templatePath, identity) => clearPageJsDraft(templatePath, null, identity),
}, PAGE_JS_DRAFT_SYNC_DELAY_MS, isActiveIdentity);

export function hasPendingPageJsDraftSync() {
  const snapshot = pageJsDraftSync.snapshot();
  return snapshot.pendingCount > 0
    || snapshot.inFlight
    || snapshot.failureCount > 0;
}

export async function flushPageJsDraftSync(options: { throwOnFailure?: boolean } = {}) {
  await pageJsDraftSync.flush(options);
}

export function resetPageJsDraftSyncState() {
  identityGeneration += 1;
  pageJsDraftSync.reset();
  activeIdentity = null;
}

export function setPageJsDraftSyncSession(
  projectRoot: string | null | undefined,
  runtimeSessionId: string | null | undefined,
) {
  const nextProjectRoot = projectRoot?.trim() ?? "";
  const nextRuntimeSessionId = runtimeSessionId?.trim() ?? "";
  if (!nextProjectRoot || !nextRuntimeSessionId) {
    resetPageJsDraftSyncState();
    return;
  }
  if (
    activeIdentity?.expectedProjectRoot === nextProjectRoot
    && activeIdentity.expectedSessionId === nextRuntimeSessionId
  ) return;
  identityGeneration += 1;
  pageJsDraftSync.reset();
  activeIdentity = {
    expectedProjectRoot: nextProjectRoot,
    expectedSessionId: nextRuntimeSessionId,
    generation: identityGeneration,
  };
}
