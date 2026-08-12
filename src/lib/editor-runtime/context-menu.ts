import type { ContextMenuItem } from "$lib/context-menu/store.svelte";
import type { EditorRuntime } from "$lib/editor-runtime/runtime";
import {
  captureEditorHtmlTarget,
  captureEditorTeraTarget,
  type EditorHtmlTarget,
  type EditorSurface,
  type EditorTeraTarget,
} from "$lib/editor-runtime/commands";
import { t } from "$lib/i18n/runtime.svelte";

export function htmlElementContextMenuItems(
  runtime: EditorRuntime,
  target: EditorHtmlTarget,
  surface: EditorSurface,
  options: {
    selectLabel?: string;
    canSelect?: boolean;
    canOpenInCode?: boolean;
    canDuplicate?: boolean;
    canDelete?: boolean;
  } = {},
): ContextMenuItem[] {
  const capturedTarget = captureEditorHtmlTarget(target);
  const canMutate = runtime.canDispatch({ type: "delete-html", surface, target: capturedTarget });
  return [
    {
      id: `${surface}-select-html`,
      label: options.selectLabel ?? t("context-menu-select-element"),
      disabled: !capturedTarget.sourceId || options.canSelect === false,
      action: async () => {
        await runtime.dispatch({ type: "select-html", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-open-html-code`,
      label: t("context-menu-open-code"),
      disabled: !capturedTarget.sourceId || options.canOpenInCode === false,
      action: async () => {
        await runtime.dispatch({ type: "open-html-code", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-duplicate-html`,
      label: t("context-menu-duplicate-element"),
      disabled: !canMutate.allowed || options.canDuplicate === false,
      separatorBefore: true,
      action: async () => {
        await runtime.dispatch({ type: "duplicate-html", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-delete-html`,
      label: t("context-menu-delete-element"),
      tone: "danger",
      shortcut: "Del",
      disabled: !canMutate.allowed || options.canDelete === false,
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
  options: {
    canSelect?: boolean;
    canEnterBoundary?: boolean;
    canOpenInCode?: boolean;
    canDelete?: boolean;
  } = {},
): ContextMenuItem[] {
  const capturedTarget = captureEditorTeraTarget(target);
  return [
    {
      id: `${surface}-select-tera`,
      label: t("context-menu-select-tera-source"),
      disabled: options.canSelect === false,
      action: async () => {
        await runtime.dispatch({ type: "select-tera", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-enter-tera-boundary`,
      label: t("context-menu-edit-html-visually"),
      disabled: options.canEnterBoundary === false
        || capturedTarget.canEnterBoundary !== true
        || !capturedTarget.editorNodeId,
      action: async () => {
        await runtime.dispatch({ type: "enter-tera-boundary", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-open-tera-code`,
      label: t("context-menu-open-source"),
      disabled: options.canOpenInCode === false,
      action: async () => {
        await runtime.dispatch({ type: "open-tera-code", surface, target: capturedTarget });
      },
    },
    {
      id: `${surface}-delete-tera`,
      label: t("context-menu-delete-tera-node"),
      tone: "danger",
      separatorBefore: true,
      disabled: options.canDelete === false,
      action: async () => {
        await runtime.dispatch({ type: "delete-tera", surface, target: capturedTarget });
      },
    },
  ];
}
