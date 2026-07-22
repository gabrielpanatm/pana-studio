import { formatSourceEditLocation } from "$lib/source-graph/location";
import type { EditableAttributes, HtmlPendingArea, SaveState, SelectionInfo } from "$lib/types";

export type HtmlDraftControllerHost = {
  selectedElement: SelectionInfo | null;
  attributeValues: EditableAttributes;
  textContentValue: string;
  textEditOriginalKey: string | null;
  textEditOriginalText: string | null;
  attributeStatus: string;
  textStatus: string;
  setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

export function htmlTextSelectionKey(selection: SelectionInfo) {
  const sourceLocation = selection.sourceLocation
    ? formatSourceEditLocation(selection.sourceLocation)
    : "";
  return `${selection.sourceId ?? sourceLocation}::${selection.domPath}`;
}

/** Keeps the inspector draft; AppState owns the acknowledged speculative Canvas projection. */
export function updateAttributeValue(
  host: HtmlDraftControllerHost,
  property: string,
  value: string,
) {
  host.attributeValues = { ...host.attributeValues, [property]: value };
  host.setHtmlPending("attributes", true);
  host.setGlobalStatus(
    `Atribut modificat: ${property} — se aplică prin ProjectWorkspace`,
    "unsaved",
  );
  host.attributeStatus = "Atributul este în ciornă; Canvas primește o proiecție temporară confirmată, iar sesiunea proiectului rămâne autoritatea.";
}

export function updateTextContentValue(host: HtmlDraftControllerHost, value: string) {
  host.textContentValue = value;
  if (!host.selectedElement || host.selectedElement.hasChildElements) {
    host.textStatus = "Editarea textului este disponibilă doar pentru elemente simple.";
    return;
  }
  const key = htmlTextSelectionKey(host.selectedElement);
  if (host.textEditOriginalKey !== key) {
    host.textEditOriginalKey = key;
    host.textEditOriginalText = host.selectedElement.rawText ?? "";
  }
  host.setHtmlPending("text", true);
  host.setGlobalStatus("Text modificat — se aplică prin sesiunea proiectului", "unsaved");
  host.textStatus = "Textul este în ciornă; proiecția live nu înlocuiește confirmarea sesiunii proiectului.";
}

export function removeAttribute(host: HtmlDraftControllerHost, name: string) {
  const { [name]: _removed, ...rest } = host.attributeValues;
  host.attributeValues = rest;
  host.setHtmlPending("attributes", true);
  host.setGlobalStatus(
    `Atribut eliminat: ${name} — se aplică prin ProjectWorkspace`,
    "unsaved",
  );
  host.attributeStatus = "Eliminarea este în ciornă; Canvas o proiectează temporar, iar sesiunea proiectului o confirmă canonic.";
}
