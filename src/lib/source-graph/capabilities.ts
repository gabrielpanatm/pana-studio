import type { MessageId } from "$lib/i18n/generated/catalog";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  SourceCapabilities,
  SourceCapabilityReason,
} from "$lib/source-graph/contracts";

const reasonMessageIds: Record<SourceCapabilityReason, MessageId> = {
  structuredConfig: "source-capability-reason-structured-config",
  structuredDataNode: "source-capability-reason-structured-data-node",
  styleFile: "source-capability-reason-style-file",
  teraTemplateFile: "source-capability-reason-tera-template-file",
  teraExtends: "source-capability-reason-tera-extends",
  teraBlock: "source-capability-reason-tera-block",
  teraInclude: "source-capability-reason-tera-include",
  teraComponentDefinition: "source-capability-reason-tera-component-definition",
  teraFor: "source-capability-reason-tera-for",
  teraIf: "source-capability-reason-tera-if",
  teraElif: "source-capability-reason-tera-elif",
  teraElse: "source-capability-reason-tera-else",
  teraSet: "source-capability-reason-tera-set",
  teraSetGlobal: "source-capability-reason-tera-set-global",
  teraFilter: "source-capability-reason-tera-filter",
  teraBreak: "source-capability-reason-tera-break",
  teraContinue: "source-capability-reason-tera-continue",
  teraSuper: "source-capability-reason-tera-super",
  teraVariable: "source-capability-reason-tera-variable",
  teraComponentCall: "source-capability-reason-tera-component-call",
  teraFunctionCall: "source-capability-reason-tera-function-call",
  legacyTeraSyntax: "source-capability-reason-legacy-tera-syntax",
  nativeBlockMarker: "source-capability-reason-native-block-marker",
  teraComment: "source-capability-reason-tera-comment",
  teraRaw: "source-capability-reason-tera-raw",
  teraSyntax: "source-capability-reason-tera-syntax",
  htmlInTeraLoop: "source-capability-reason-html-in-tera-loop",
  htmlInTeraCondition: "source-capability-reason-html-in-tera-condition",
  htmlInTeraComponent: "source-capability-reason-html-in-tera-component",
  htmlInTeraLocalScope: "source-capability-reason-html-in-tera-local-scope",
  htmlInTeraRaw: "source-capability-reason-html-in-tera-raw",
  markdownPage: "source-capability-reason-markdown-page",
  staticJavaScript: "source-capability-reason-static-javascript",
  staticAsset: "source-capability-reason-static-asset",
  dataOutputReadOnly: "source-capability-reason-data-output-read-only",
  dataThemeReadOnly: "source-capability-reason-data-theme-read-only",
  dataFormatVisualUnsupported: "source-capability-reason-data-format-visual-unsupported",
  markdownRenderedBoundary: "source-capability-reason-markdown-rendered-boundary",
  markdownSourceUnresolved: "source-capability-reason-markdown-source-unresolved",
};

export function sourceCapabilityReason(
  capabilities: Pick<SourceCapabilities, "reasonCode">,
  fallback: MessageId = "source-capability-reason-code-only",
): string {
  const messageId = capabilities.reasonCode
    ? reasonMessageIds[capabilities.reasonCode]
    : fallback;
  return t(messageId);
}
