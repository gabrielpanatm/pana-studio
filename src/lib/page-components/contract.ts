import {
  capturePageContractProjectionLease,
  capturePageContractSessionLease,
  flushPageContractDrafts,
  pageContractSessionLeaseMatches,
  projectPageContractReceipt,
  runInPageContractLane,
  type PageContractProjectionHost,
} from "$lib/page-contracts/projection";
import {
  projectRelativeZolaPath,
  zolaRelativePath,
} from "$lib/project/files";
import { pageScssRelativePath } from "$lib/page-assets/paths";
import { applyPageComponentContract } from "$lib/project/io";
import type { SaveState, SourceEditLocation } from "$lib/types";

export type PageComponentContractHost = PageContractProjectionHost & {
  postPreviewMessage: (payload: Record<string, unknown>) => void;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

const styleElementId = "pana-component-contract-preview-css";

function syncComponentPreview(
  host: PageComponentContractHost,
  previewCss: string,
) {
  host.postPreviewMessage({
    type: "set-live-style-css",
    id: styleElementId,
    css: previewCss,
    refreshSelection: false,
  });
}

export async function reconcilePageComponentContracts(
  host: PageComponentContractHost,
  tpl: SourceEditLocation,
  options: { ensureComponentId?: string | null } = {},
) {
  const templateRelativePath = projectRelativeZolaPath(tpl.file);
  const templatePath = zolaRelativePath(tpl.file);
  const fallbackScssPath = projectRelativeZolaPath(pageScssRelativePath(templatePath));
  const sessionLease = capturePageContractSessionLease(host);

  return runInPageContractLane(sessionLease, templatePath, async () => {
    requireCurrentSession(host, sessionLease);
    await flushPageContractDrafts();
    requireCurrentSession(host, sessionLease);
    const projectionLease = capturePageContractProjectionLease(
      host, sessionLease, templatePath, [templateRelativePath, fallbackScssPath],
    );
    const receipt = await applyPageComponentContract({
      expectedProjectRoot: sessionLease.projectRoot,
      expectedSessionId: sessionLease.sessionId,
      templatePath,
      ensureComponentId: options.ensureComponentId ?? null,
      cachebustAssets: false,
    });
    projectPageContractReceipt(host, projectionLease, templateRelativePath, fallbackScssPath, receipt);
    syncComponentPreview(host, receipt.plan.previewCss);
    if (options.ensureComponentId) {
      host.setGlobalStatus("Componentă adăugată în sesiunea proiectului.", "unsaved");
    }
    return receipt;
  });
}

function requireCurrentSession(
  host: PageComponentContractHost,
  lease: ReturnType<typeof capturePageContractSessionLease>,
) {
  if (!pageContractSessionLeaseMatches(host, lease)) {
    throw new Error("Page Component contract a fost anulat deoarece ProjectSession s-a schimbat.");
  }
}
