import type { ContextMenuItem } from "$lib/context-menu/store.svelte";
import type { EditorRuntime } from "$lib/editor-runtime/runtime";
import {
  captureEditorHtmlTarget,
  captureEditorTeraTarget,
  type EditorHtmlTarget,
  type EditorSurface,
  type EditorTeraTarget,
} from "$lib/editor-runtime/commands";

export function htmlElementContextMenuItems(
  runtime: EditorRuntime,
  target: EditorHtmlTarget,
  surface: EditorSurface,
  options: { selectLabel?: string } = {},
): ContextMenuItem[] {
  const capturedTarget = captureEditorHtmlTarget(target);
  const canMutate = runtime.canDispatch({ type: "delete-html", surface, target: capturedTarget });
  return [
    {
      id: `${surface}-select-html`,
      label: options.selectLabel ?? "Selecteaza element",
      disabled: !capturedTarget.selector,
      action: async () => {
        await runtime.dispatch({ type: "select-html", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-open-html-code`,
      label: "Deschide in cod",
      disabled: !capturedTarget.selector,
      action: async () => {
        await runtime.dispatch({ type: "open-html-code", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-duplicate-html`,
      label: "Duplica element",
      disabled: !canMutate.allowed,
      separatorBefore: true,
      action: async () => {
        await runtime.dispatch({ type: "duplicate-html", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-delete-html`,
      label: "Sterge element",
      tone: "danger",
      shortcut: "Del",
      disabled: !canMutate.allowed,
      action: async () => {
        await runtime.dispatch({ type: "delete-html", surface, target: capturedTarget });
      },
    },
  ];
}

export function teraContextMenuItems(
  runtime: EditorRuntime,
  target: EditorTeraTarget,
  surface: EditorSurface,
): ContextMenuItem[] {
  const capturedTarget = captureEditorTeraTarget(target);
  return [
    {
      id: `${surface}-select-tera`,
      label: "Selecteaza sursa Tera",
      action: async () => {
        await runtime.dispatch({ type: "select-tera", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-edit-tera-html`,
      label: "Editeaza HTML vizual",
      disabled: capturedTarget.canSelectHtml === false || !capturedTarget.selector,
      action: async () => {
        await runtime.dispatch({ type: "edit-tera-html", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-open-tera-code`,
      label: "Deschide sursa",
      action: async () => {
        await runtime.dispatch({ type: "open-tera-code", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-delete-tera`,
      label: "Sterge nodul Tera",
      tone: "danger",
      separatorBefore: true,
      action: async () => {
        await runtime.dispatch({ type: "delete-tera", surface, target: capturedTarget });
      },
    },
  ];
}
