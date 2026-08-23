<script lang="ts">
  import { untrack } from "svelte";
  import {
    advanceStableHtmlInspectorProjection,
    type StableHtmlInspectorProjection,
  } from "$lib/inspector/html-projection-stability";
  import type {
    EditableAttributes,
    InspectorHtmlPhysicalFacts,
  } from "$lib/canvas/contracts";
  import type {
    InspectorSelectionSummarySnapshot,
    SelectionSnapshot,
  } from "$lib/editor/contracts";

  let {
    summary = null,
    selection = null,
    physicalFacts = null,
    attributeValues,
    textContentValue = "",
    classEditorValue = "",
    imageSourceValue = "",
    pendingTag = null,
    attributeStatus = "",
    textStatus = "",
    classStatus = "",
    imageStatus = "",
    tagStatus = "",
    canEditHtml = false,
    isActivePreviewHtmlSource,
    stableProjection = $bindable<StableHtmlInspectorProjection | null>(null),
    pending = $bindable(false),
  }: {
    summary?: InspectorSelectionSummarySnapshot | null;
    selection?: SelectionSnapshot | null;
    physicalFacts?: InspectorHtmlPhysicalFacts | null;
    attributeValues: EditableAttributes;
    textContentValue?: string;
    classEditorValue?: string;
    imageSourceValue?: string;
    pendingTag?: string | null;
    attributeStatus?: string;
    textStatus?: string;
    classStatus?: string;
    imageStatus?: string;
    tagStatus?: string;
    canEditHtml?: boolean;
    isActivePreviewHtmlSource: boolean;
    stableProjection?: StableHtmlInspectorProjection | null;
    pending?: boolean;
  } = $props();

  $effect.pre(() => {
    const transition = advanceStableHtmlInspectorProjection(
      untrack(() => stableProjection),
      {
        summary,
        selection,
        physicalFacts,
        attributeValues,
        textContentValue,
        classEditorValue,
        imageSourceValue,
        pendingTag,
        attributeStatus,
        textStatus,
        classStatus,
        imageStatus,
        tagStatus,
        canEditHtml,
        isActivePreviewHtmlSource,
      },
    );
    stableProjection = transition.projection;
    pending = transition.pending;
  });
</script>
