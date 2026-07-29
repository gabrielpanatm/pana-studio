import { formatSourceEditLocation } from "$lib/source-graph/location";
import { projectRelativeZolaPath } from "$lib/project/files";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  EditorSourceReference,
  SelectionSnapshot,
  SourceNodeKind,
  SourceRange,
} from "$lib/types";

export type WorkbenchSourceRole = "html" | "css" | "js";

export type WorkbenchSourceStatus = {
  role: WorkbenchSourceRole;
  label: string;
  value: string;
  file: string;
  location: string;
  selector: string | null;
  openable: boolean;
  definition: EditorSourceReference | null;
  composition: EditorSourceReference | null;
};

function sourceRangeDisplay(file: string, range: SourceRange | null | undefined) {
  const relativeFile = projectRelativeZolaPath(file);
  return range
    ? formatSourceEditLocation({
        file: relativeFile,
        line: range.line,
        column: range.column,
      })
    : relativeFile;
}

function sourceRangeLocation(file: string, range: SourceRange | null | undefined) {
  return range
    ? formatSourceEditLocation({
        file,
        line: range.line,
        column: range.column,
      })
    : file;
}

function stylesheetLabel(file: string) {
  const normalized = file.toLowerCase();
  if (normalized.endsWith(".scss")) return "SCSS";
  if (normalized.endsWith(".sass")) return "SASS";
  return "CSS";
}

export function workbenchSourceStatusFromSelection(
  selection: SelectionSnapshot | null,
): WorkbenchSourceStatus | null {
  if (!selection || selection.resolution === "cleared") return null;

  const { focus, provenance } = selection.projections.status;
  if (focus.kind === "cssRule" || focus.kind === "cssProperty") {
    const file = focus.file.trim();
    if (!file) return null;
    return {
      role: "css",
      label: stylesheetLabel(file),
      value: sourceRangeDisplay(file, focus.range),
      file,
      location: sourceRangeLocation(file, focus.range),
      selector: focus.selector,
      openable: Boolean(focus.selector),
      definition: provenance?.definition ?? null,
      composition: provenance?.composition ?? null,
    };
  }

  if (focus.kind === "jsBehavior") {
    const file = focus.file.trim();
    if (!file) return null;
    return {
      role: "js",
      label: "JS",
      value: projectRelativeZolaPath(file),
      file,
      location: file,
      selector: null,
      openable: true,
      definition: provenance?.definition ?? null,
      composition: provenance?.composition ?? null,
    };
  }

  const definition = provenance?.definition ?? null;
  const composition = provenance?.composition ?? null;
  const source = definition ?? composition;
  if (!source) return null;
  return {
    role: "html",
    label: "HTML",
    value: editorSourceReferenceDisplay(source),
    file: source.file,
    location: editorSourceReferenceLocation(source),
    selector: null,
    openable: source.canOpenInCode,
    definition,
    composition,
  };
}

export function editorSourceReferenceDisplay(reference: EditorSourceReference) {
  const file = projectRelativeZolaPath(reference.file);
  return reference.range
    ? formatSourceEditLocation({
        file,
        line: reference.range.line,
        column: reference.range.column,
      })
    : file;
}

export function editorSourceReferenceLocation(reference: EditorSourceReference) {
  return reference.range
    ? `${reference.file}:${reference.range.line}:${reference.range.column}`
    : reference.file;
}

export function teraSourceKindLabel(kind: SourceNodeKind) {
  if (kind === "template") return t("source-provenance-kind-template");
  if (kind === "partial") return t("source-provenance-kind-partial");
  if (kind === "extends") return t("source-provenance-kind-layout");
  if (kind === "block") return t("source-provenance-kind-block");
  if (kind === "include") return t("source-provenance-kind-include");
  if (kind === "import") return t("source-provenance-kind-import");
  if (kind === "macro") return t("source-provenance-kind-macro");
  if (kind === "for") return t("source-provenance-kind-loop");
  if (kind === "if") return t("source-provenance-kind-condition");
  if (kind === "set") return t("source-provenance-kind-local");
  if (kind === "teraVariable") return t("source-provenance-kind-variable");
  if (kind === "teraComment") return t("source-provenance-kind-comment");
  if (kind === "raw") return t("source-provenance-kind-raw");
  if (kind === "tera") return "Tera";
  return kind;
}
