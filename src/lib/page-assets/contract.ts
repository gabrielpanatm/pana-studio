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
import { applyPageAssetContract } from "$lib/project/io";
import type { SaveState, SourceEditLocation } from "$lib/types";
import { pageScssRelativePath } from "$lib/page-assets/paths";

export type PageAssetContractHost = PageContractProjectionHost & {
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

export async function reconcilePageAssetContracts(
  host: PageAssetContractHost,
  tpl: SourceEditLocation,
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
    const receipt = await applyPageAssetContract({
      expectedProjectRoot: sessionLease.projectRoot,
      expectedSessionId: sessionLease.sessionId,
      templatePath,
    });
    projectPageContractReceipt(host, projectionLease, templateRelativePath, fallbackScssPath, receipt);
    if (receipt.plan.template.changed) {
      host.setGlobalStatus("Contractul CSS a fost actualizat în sesiunea ProjectWorkspace.", "unsaved");
    }
    return receipt;
  });
}

function requireCurrentSession(
  host: PageAssetContractHost,
  lease: ReturnType<typeof capturePageContractSessionLease>,
) {
  if (!pageContractSessionLeaseMatches(host, lease)) {
    throw new Error("Page Asset contract a fost anulat deoarece ProjectSession s-a schimbat.");
  }
}
