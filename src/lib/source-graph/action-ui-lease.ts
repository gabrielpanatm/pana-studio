export type SiteActionUiSessionHost = {
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  projectSessionEpoch: number;
};

export type SiteActionUiLease = {
  callId: number;
  projectRoot: string;
  sessionId: string;
  projectSessionEpoch: number;
};

export function captureSiteActionUiLease(
  host: SiteActionUiSessionHost,
  callId: number,
): SiteActionUiLease {
  return Object.freeze({
    callId,
    projectRoot: host.sessionProjectRoot,
    sessionId: host.kernelProjectSessionId,
    projectSessionEpoch: host.projectSessionEpoch,
  });
}

export function siteActionUiLeaseMatches(
  host: SiteActionUiSessionHost,
  lease: SiteActionUiLease,
  currentCallId: number,
) {
  return currentCallId === lease.callId
    && host.sessionProjectRoot === lease.projectRoot
    && host.kernelProjectSessionId === lease.sessionId
    && host.projectSessionEpoch === lease.projectSessionEpoch;
}
