import { createDiskState, type DiskState } from "$lib/session/disk-state";

/** Owns the accepted on-disk baseline projected by the active ProjectSession. */
export class AcceptedDiskState {
  snapshot = $state<DiskState>(createDiskState());

  reset() {
    this.snapshot = createDiskState();
  }
}
