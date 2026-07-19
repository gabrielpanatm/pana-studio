/**
 * A committed result may settle the submitted draft when the editor still
 * contains either that submission or the exact baseline captured before a
 * programmatic change. Any third value is a concurrent user edit and must be
 * preserved as pending.
 */
export function committedDraftCanSettle(
  currentDraft: string,
  submittedDraft: string,
  baselineDraft: string,
) {
  return currentDraft === submittedDraft || currentDraft === baselineDraft;
}
