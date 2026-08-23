/** Owns the frontend quiescence and mutation lease around Rust undo/redo. */
export class HistoryOperationState {
  quiesceActive = $state(false);
  leaseActive = $state(false);

  reset() {
    this.quiesceActive = false;
    this.leaseActive = false;
  }
}
