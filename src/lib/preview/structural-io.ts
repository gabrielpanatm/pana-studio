import type {
  PreviewProjectionIntentInput,
  PreviewProjectionIntentReceipt,
  PreviewStructuralCommandIdentity,
} from "$lib/preview/contracts";
import type {
  PreviewHtmlAttributesExecutionInput,
  PreviewHtmlAttributesExecutionReceipt,
  PreviewHtmlDeleteExecutionInput,
  PreviewHtmlDeleteExecutionReceipt,
  PreviewHtmlDuplicateExecutionInput,
  PreviewHtmlDuplicateExecutionReceipt,
  PreviewHtmlInsertDropExecutionInput,
  PreviewHtmlInsertDropExecutionReceipt,
  PreviewHtmlTagExecutionInput,
  PreviewHtmlTagExecutionReceipt,
  PreviewHtmlTextExecutionInput,
  PreviewHtmlTextExecutionReceipt,
  PreviewSelectionBatchExecutionInput,
  PreviewSelectionBatchExecutionReceipt,
  PreviewTeraDeleteExecutionInput,
  PreviewTeraDeleteExecutionReceipt,
  PreviewTeraInsertDropExecutionInput,
  PreviewTeraInsertDropExecutionReceipt,
} from "$lib/preview/contracts";
import { invoke } from "@tauri-apps/api/core";

export function normalizePreviewProjectionIntent(
  input: PreviewProjectionIntentInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewProjectionIntentReceipt> {
  return invoke<PreviewProjectionIntentReceipt>("normalize_preview_projection_intent", { input, identity });
}

export function executePreviewHtmlInsertDropIntent(
  input: PreviewHtmlInsertDropExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlInsertDropExecutionReceipt> {
  return invoke<PreviewHtmlInsertDropExecutionReceipt>("execute_preview_html_insert_drop_intent", { input, identity });
}

export function executePreviewHtmlAttributesIntent(
  input: PreviewHtmlAttributesExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlAttributesExecutionReceipt> {
  return invoke<PreviewHtmlAttributesExecutionReceipt>("execute_preview_html_attributes_intent", { input, identity });
}

export function executePreviewHtmlTextIntent(
  input: PreviewHtmlTextExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlTextExecutionReceipt> {
  return invoke<PreviewHtmlTextExecutionReceipt>("execute_preview_html_text_intent", { input, identity });
}

export function executePreviewHtmlTagIntent(
  input: PreviewHtmlTagExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlTagExecutionReceipt> {
  return invoke<PreviewHtmlTagExecutionReceipt>("execute_preview_html_tag_intent", { input, identity });
}

export function executePreviewHtmlDuplicateIntent(
  input: PreviewHtmlDuplicateExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlDuplicateExecutionReceipt> {
  return invoke<PreviewHtmlDuplicateExecutionReceipt>("execute_preview_html_duplicate_intent", { input, identity });
}

export function executePreviewHtmlDeleteIntent(
  input: PreviewHtmlDeleteExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlDeleteExecutionReceipt> {
  return invoke<PreviewHtmlDeleteExecutionReceipt>("execute_preview_html_delete_intent", { input, identity });
}

export function executePreviewSelectionBatchIntent(
  input: PreviewSelectionBatchExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewSelectionBatchExecutionReceipt> {
  return invoke<PreviewSelectionBatchExecutionReceipt>("execute_preview_selection_batch_intent", { input, identity });
}

export function executePreviewTeraDeleteIntent(
  input: PreviewTeraDeleteExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewTeraDeleteExecutionReceipt> {
  return invoke<PreviewTeraDeleteExecutionReceipt>("execute_preview_tera_delete_intent", { input, identity });
}

export function executePreviewTeraInsertDropIntent(
  input: PreviewTeraInsertDropExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewTeraInsertDropExecutionReceipt> {
  return invoke<PreviewTeraInsertDropExecutionReceipt>("execute_preview_tera_insert_drop_intent", { input, identity });
}
