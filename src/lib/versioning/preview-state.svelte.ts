import type { VersionPreviewReceipt } from "$lib/versioning/contracts";

/** Owns the selected read-only historical preview generation. */
export class VersionPreviewState {
  active = $state<VersionPreviewReceipt | null>(null);

  reset() {
    this.active = null;
  }
}
