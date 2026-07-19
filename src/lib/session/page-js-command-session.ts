import type {
  PageJsCommandReceipt,
  PageJsRequestIdentity,
} from "$lib/types";

export function createPageJsRequestIdentity(
  projectRoot: string,
  runtimeSessionId: string,
): PageJsRequestIdentity {
  const expectedProjectRoot = projectRoot.trim();
  const expectedSessionId = runtimeSessionId.trim();
  if (!expectedProjectRoot || !expectedSessionId) {
    throw new Error("Page JS cere ProjectRoot și runtimeSessionId active.");
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
      `${operation} a refuzat receipt-ul Page JS al altei sesiuni runtime.`,
    );
  }
  return receipt.payload;
}
