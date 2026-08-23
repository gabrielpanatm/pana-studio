import type {
  PageJsCommandReceipt,
  PageJsRequestIdentity,
} from "$lib/js/contracts";
import { t } from "$lib/i18n/runtime.svelte";

export function createPageJsRequestIdentity(
  projectRoot: string,
  runtimeSessionId: string,
): PageJsRequestIdentity {
  const expectedProjectRoot = projectRoot.trim();
  const expectedSessionId = runtimeSessionId.trim();
  if (!expectedProjectRoot || !expectedSessionId) {
    throw new Error(t("page-js-command-session-missing"));
  }
  return { expectedProjectRoot, expectedSessionId };
}

export function isPageJsRequestIdentityCurrent(
  identity: PageJsRequestIdentity,
  projectRoot: string,
  runtimeSessionId: string,
): boolean {
  return identity.expectedProjectRoot === projectRoot
    && identity.expectedSessionId === runtimeSessionId;
}

export function pageJsCommandPayload<T>(
  receipt: PageJsCommandReceipt<T>,
  identity: PageJsRequestIdentity,
  operation: string,
): T {
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(
      t("page-js-command-receipt-session-mismatch", { operation }),
    );
  }
  return receipt.payload;
}
