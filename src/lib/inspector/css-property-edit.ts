import type { ScssVariable } from "$lib/types";

export type CssPendingValueBaseline = Readonly<{
  hadPendingValue: boolean;
  value: string;
}>;

export function captureCssPendingValueBaseline(
  pendingValues: Readonly<Record<string, string>>,
  property: string,
): CssPendingValueBaseline {
  return {
    hadPendingValue: Object.prototype.hasOwnProperty.call(pendingValues, property),
    value: pendingValues[property] ?? "",
  };
}

export function restoreCssPendingValueBaseline(
  pendingValues: Readonly<Record<string, string>>,
  property: string,
  baseline: CssPendingValueBaseline,
): Record<string, string> {
  const restored = { ...pendingValues };
  if (baseline.hadPendingValue) restored[property] = baseline.value;
  else delete restored[property];
  return restored;
}

export type CssContinuousEditHandlers = Readonly<{
  oninput: (value: string) => void;
  oncommit: (value: string) => void;
  oncancel: () => void;
}>;

/**
 * Contractul unic dintre controalele Inspectorului și autoritatea CSS.
 *
 * - `draft` proiectează optimist valoarea, fără a crea încă o operație în timeline;
 * - `commit` finalizează o interacțiune și o trimite către ProjectWorkspace;
 * - `cancel` abandonează numai draftul proprietății curente;
 * - `continuous` leagă un control cu editare continuă la cele trei faze.
 */
export type CssPropertyEditController = Readonly<{
  draft: (property: string, value: string) => void;
  commit: (property: string, value?: string) => void;
  cancel: (property: string) => void;
  continuous: (property: string) => CssContinuousEditHandlers;
}>;

export type ScssVariableEditController = Readonly<{
  draft: (variable: ScssVariable, value: string) => void;
  commit: (variable: ScssVariable, value?: string) => void;
  cancel: (variable: ScssVariable) => void;
}>;
