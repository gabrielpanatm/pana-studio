export type AssetEditLease = Readonly<{
  contextKey: string;
  baselineValue: string;
}>;

export function captureAssetEditLease(contextKey: string, baselineValue: string): AssetEditLease {
  return Object.freeze({ contextKey, baselineValue });
}

export function assetEditLeaseMatches(lease: AssetEditLease | null, currentContextKey: string) {
  return Boolean(lease && lease.contextKey === currentContextKey);
}

export function cancelledAssetEditValue(lease: AssetEditLease) {
  return lease.baselineValue;
}
