<script lang="ts">
  import {
    IconCode, IconFingerprint, IconAlignLeft, IconLink, IconPhoto,
    IconAdjustments, IconAccessible, IconDatabase, IconForms, IconTag,
    IconVideo, IconVolume2, IconBrowser, IconPointer, IconPlus, IconX,
  } from "@tabler/icons-svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import InspectorSection from "./InspectorSection.svelte";
  import InspectorEmptyState from "./InspectorEmptyState.svelte";
  import AssetPicker from "./controls/AssetPicker.svelte";
  import {
    htmlAccessibilityAttributeNames,
    htmlAttributeDefinition,
    htmlAttributePreviewMode,
    htmlAttributeValueError,
    htmlAttributesForElement,
    htmlGlobalAttributeNames,
    htmlTagCapability,
    htmlTagTransitionOptions,
  } from "$lib/html/editor-schema";
  import {
    projectAssetOriginLabel,
    projectAssetPublicUrl,
  } from "$lib/project/assets";
  import {
    createZolaImageIntent,
    resolveZolaImageSource,
    zolaImageSourceFailureMessage,
  } from "$lib/html/zola-image";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type {
    EditableAttributes,
    InspectorHtmlPhysicalFacts,
    InspectorSelectionSummarySnapshot,
    ProjectFile,
    ProjectZolaImageIntent,
    SelectionSnapshot,
    ZolaImageFormat,
    ZolaImageOperation,
  } from "$lib/types";
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";

  let {
    selectionSummary = null,
    selectionSnapshot = null,
    physicalFacts = null,
    canEditHtml = false,
    attributeValues,
    textContentValue = "",
    imageSourceValue = "",
    classEditorValue = "",
    pendingTag = null,
    tagStatus = "",
    attributeStatus = "",
    textStatus = "",
    classStatus = "",
    imageStatus = "",
    scannedAssets = [],
    isActivePreviewHtmlSource,
    updateAttributeValue,
    removeAttribute,
    applyAttributesToHtml,
    updateTextContentValue,
    applyTextContentToHtml,
    setClassEditorValue,
    applyClassesToHtml,
    generateClassForSelectedHtml,
    generateDataAnimForSelectedHtml,
    setImageSourceValue,
    applyZolaImageProcessingToHtml,
    cancelHtmlAttributeDraft,
    changeElementTag,
  }: {
    selectionSummary?: InspectorSelectionSummarySnapshot | null;
    selectionSnapshot?: SelectionSnapshot | null;
    physicalFacts?: InspectorHtmlPhysicalFacts | null;
    canEditHtml?: boolean;
    attributeValues: EditableAttributes;
    textContentValue?: string;
    imageSourceValue?: string;
    classEditorValue?: string;
    pendingTag?: string | null;
    tagStatus?: string;
    attributeStatus?: string;
    textStatus?: string;
    classStatus?: string;
    imageStatus?: string;
    scannedAssets?: ProjectFile[];
    isActivePreviewHtmlSource: boolean;
    updateAttributeValue: (name: string, value: string) => void;
    removeAttribute: (name: string) => void;
    applyAttributesToHtml: (attributes?: EditableAttributes) => void | Promise<EditorActionOutcome>;
    updateTextContentValue: (value: string, composing?: boolean) => void;
    applyTextContentToHtml: () => void | Promise<EditorActionOutcome>;
    setClassEditorValue: (value: string) => void;
    applyClassesToHtml: () => void | Promise<EditorActionOutcome>;
    generateClassForSelectedHtml: () => void | Promise<EditorActionOutcome>;
    generateDataAnimForSelectedHtml: () => void | Promise<EditorActionOutcome>;
    setImageSourceValue: (value: string) => void;
    applyZolaImageProcessingToHtml: (intent: ProjectZolaImageIntent) => void | Promise<EditorActionOutcome>;
    cancelHtmlAttributeDraft: (expectedContextKey?: string) => void;
    changeElementTag: (tag: string) => void;
  } = $props();

  // Internal add-flow state
  let addingClass = $state(false);
  let newClassName = $state("");
  let addingData = $state(false);
  let newDataKey = $state("");
  let newDataValue = $state("");
  let addingAria = $state(false);
  let newAriaKey = $state("");
  let newAriaValue = $state("");
  let textCompositionActive = false;
  let fieldStatusName = $state("");
  let fieldStatusText = $state("");
  let fieldStatusKind = $state<"info" | "success" | "error">("info");
  let fieldCommitSerial = 0;
  let zolaWidthText = $state("1200");
  let zolaHeightText = $state("800");
  let zolaOperation = $state<ZolaImageOperation>("fit_width");
  let zolaFormat = $state<ZolaImageFormat>("webp");
  let zolaQualityText = $state("82");
  let zolaSourceDraft = $state("");
  let zolaImagePending = $state(false);
  let zolaDraftSelectionKey = "";

  const canEdit = $derived(canEditHtml);
  const hasElementSelection = $derived(
    selectionSummary?.state === "resolved"
      && (
        selectionSummary.subjectKind === "htmlElement"
        || selectionSummary.subjectKind === "runtimeElement"
      ),
  );
  const tag = $derived(pendingTag ?? selectionSummary?.tag ?? "");
  const tagCapability = $derived(htmlTagCapability(tag));
  const tagOptions = $derived(htmlTagTransitionOptions(tag));
  const canChangeTag = $derived(canEdit && tagOptions.length > 1);
  const assetContextKey = $derived([
    selectionSnapshot?.runtimeSessionId ?? "",
    selectionSnapshot?.selectionRevision ?? "",
    selectionSnapshot?.anchor?.sourceNodeId ?? "",
    selectionSnapshot?.anchor?.renderInstanceId ?? "",
    selectionSnapshot?.provenance?.definition?.file
      ?? selectionSnapshot?.provenance?.composition?.file
      ?? "",
    selectionSnapshot?.provenance?.definition?.range?.line
      ?? selectionSnapshot?.provenance?.composition?.range?.line
      ?? "",
    selectionSnapshot?.provenance?.definition?.range?.column
      ?? selectionSnapshot?.provenance?.composition?.range?.column
      ?? "",
    tag,
  ].join("::"));
  const zolaImage = $derived(physicalFacts?.zolaImage ?? null);
  const zolaImageEnabled = $derived(Boolean(zolaImage));

  $effect(() => {
    const presentation = zolaImage;
    const nextKey = `${assetContextKey}::${JSON.stringify(presentation)}`;
    if (nextKey === zolaDraftSelectionKey) return;
    zolaDraftSelectionKey = nextKey;
    zolaWidthText = String(presentation?.width ?? positiveInteger(getAttr("width")) ?? 1200);
    zolaHeightText = String(presentation?.height ?? positiveInteger(getAttr("height")) ?? 800);
    zolaOperation = presentation?.operation ?? "fit_width";
    zolaFormat = presentation?.format ?? "webp";
    zolaQualityText = String(presentation?.quality ?? 82);
    zolaSourceDraft = presentation?.sourceUrl ?? imageSourceValue ?? getAttr("src");
  });

  // ── Attribute helpers ────────────────────────────────────────────────────

  function getAttr(name: string): string {
    return attributeValues[name] ?? "";
  }

  // HTML boolean attributes are true by presence, including the valid empty form.
  function getBool(name: string): boolean {
    return Object.prototype.hasOwnProperty.call(attributeValues, name);
  }

  function setAttr(name: string, value: string) {
    updateAttributeValue(name, value);
  }

  function setFieldStatus(name: string, text: string, kind: "info" | "success" | "error") {
    fieldStatusName = name;
    fieldStatusText = text;
    fieldStatusKind = kind;
  }

  function canEditAttribute(name: string) {
    return canEdit && htmlAttributeDefinition(name)?.sourceEditable !== false;
  }

  async function applyAttributeMutation(name: string, nextAttributes: EditableAttributes) {
    const serial = ++fieldCommitSerial;
    const previewMode = htmlAttributePreviewMode(name, tag);
    setFieldStatus(name, t("inspector-applying"), "info");
    try {
      const result = await applyAttributesToHtml(nextAttributes);
      if (serial !== fieldCommitSerial) return;
      if (!result) {
        setFieldStatus(name, attributeStatus || t("inspector-operation-submitted"), "info");
        return;
      }
      if (result.status === "failed" || result.status === "blocked") {
        setFieldStatus(name, result.reason || attributeStatus || t("inspector-attribute-failed"), "error");
        return;
      }
      const projectionNote = previewMode === "sourceOnly"
        ? ` ${t("inspector-source-only-note")}`
        : previewMode === "inert"
          ? ` ${t("inspector-inert-note")}`
          : "";
      setFieldStatus(
        name,
        (result.reason || attributeStatus || (result.status === "noop" ? t("inspector-no-canonical-difference") : t("inspector-attribute-applied"))) + projectionNote,
        "success",
      );
    } catch (error) {
      if (serial !== fieldCommitSerial) return;
      setFieldStatus(name, error instanceof Error ? error.message : String(error), "error");
    }
  }

  function commitAttribute(name: string, value: string) {
    const definition = htmlAttributeDefinition(name);
    if (definition?.sourceEditable === false) {
      setFieldStatus(name, definition.reason || t("inspector-schema-blocked", { name }), "error");
      return;
    }
    const validationError = htmlAttributeValueError(name, value);
    if (validationError) {
      setFieldStatus(name, validationError, "error");
      return;
    }
    const nextAttributes = { ...attributeValues, [name]: value };
    if (!Object.prototype.hasOwnProperty.call(attributeValues, name) || attributeValues[name] !== value) {
      updateAttributeValue(name, value);
    }
    void applyAttributeMutation(name, nextAttributes);
  }

  function commitAttributeRemoval(name: string) {
    const { [name]: _removed, ...nextAttributes } = attributeValues;
    removeAttribute(name);
    void applyAttributeMutation(name, nextAttributes);
  }

  function toggleBool(name: string, enabled: boolean) {
    if (enabled) commitAttribute(name, "");
    else commitAttributeRemoval(name);
  }

  function commitField(name: string, value = getAttr(name)) {
    const definition = htmlAttributeDefinition(name);
    const isPresent = Object.prototype.hasOwnProperty.call(attributeValues, name);
    if (value === "" && definition?.emptyPolicy === "remove") {
      if (!isPresent) return;
      commitAttributeRemoval(name);
      return;
    }
    if (value === "" && !isPresent) return;
    commitAttribute(name, value);
  }

  function positiveInteger(value: string): number | null {
    const parsed = Number(value.trim());
    return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
  }

  function currentZolaSourceUrl() {
    return (zolaSourceDraft || zolaImage?.sourceUrl || imageSourceValue || getAttr("src")).trim();
  }

  async function commitZolaImage(enabled: boolean) {
    if (zolaImagePending) return;
    let intent: ProjectZolaImageIntent;
    try {
      if (!enabled) {
        intent = createZolaImageIntent({ enabled: false });
      } else {
        const source = resolveZolaImageSource(currentZolaSourceUrl(), scannedAssets);
        const width = positiveInteger(zolaWidthText);
        const height = positiveInteger(zolaHeightText);
        const quality = positiveInteger(zolaQualityText);
        if (!width || width > 16384) throw new Error(t("inspector-width-range"));
        if (zolaOperation !== "fit_width" && (!height || height > 16384)) {
          throw new Error(t("inspector-height-range"));
        }
        if (!quality || quality > 100) throw new Error(t("inspector-quality-range"));
        intent = createZolaImageIntent({
          enabled: true,
          source,
          width,
          height: zolaOperation === "fit_width" ? null : height,
          operation: zolaOperation,
          format: zolaFormat,
          quality,
        });
      }
    } catch (error) {
      setFieldStatus("zola-image", error instanceof Error ? error.message : String(error), "error");
      return;
    }

    zolaImagePending = true;
    setFieldStatus("zola-image", enabled ? t("inspector-zola-writing") : t("inspector-zola-restoring"), "info");
    try {
      const result = await applyZolaImageProcessingToHtml(intent);
      if (result && (result.status === "failed" || result.status === "blocked")) {
        setFieldStatus("zola-image", result.reason || t("inspector-zola-blocked"), "error");
      } else {
        setFieldStatus(
          "zola-image",
          enabled ? t("inspector-zola-enabled") : t("inspector-zola-disabled"),
          "success",
        );
      }
    } catch (error) {
      setFieldStatus("zola-image", error instanceof Error ? error.message : String(error), "error");
    } finally {
      zolaImagePending = false;
    }
  }

  function commitTagDraft(nextValue: string) {
    const nextTag = nextValue.trim();
    if (!nextTag || nextTag === tag) return;
    changeElementTag(nextTag);
  }

  // ── Class management ─────────────────────────────────────────────────────

  function addClass() {
    const cls = newClassName.trim();
    if (!cls) return;
    const current = classEditorValue.trim();
    setClassEditorValue(current ? `${current} ${cls}` : cls);
    applyClassesToHtml();
    newClassName = "";
    addingClass = false;
  }

  function removeClass(cls: string) {
    const updated = classEditorValue.split(/\s+/).filter((c) => c !== cls).join(" ");
    setClassEditorValue(updated);
    applyClassesToHtml();
  }

  // ── Data attributes ──────────────────────────────────────────────────────

  const dataAttrs = $derived(
    Object.entries(attributeValues).filter(([k]) => k.startsWith("data-") && !k.startsWith("data-pana-"))
  );

  function confirmAddData() {
    let key = newDataKey.trim();
    if (!key) return;
    if (!key.startsWith("data-")) key = `data-${key}`;
    if (key === "data-" || key.startsWith("data-pana-")) return;
    commitAttribute(key, newDataValue);
    newDataKey = "";
    newDataValue = "";
    addingData = false;
  }

  // ── Aria ─────────────────────────────────────────────────────────────────

  function confirmAddAria() {
    let key = newAriaKey.trim();
    if (!key) return;
    if (!key.startsWith("aria-")) key = `aria-${key}`;
    if (key === "aria-") return;
    commitAttribute(key, newAriaValue);
    newAriaKey = "";
    newAriaValue = "";
    addingAria = false;
  }

  // ── Section hasValues ────────────────────────────────────────────────────

  const GLOBAL_ATTRS = new Set(htmlGlobalAttributeNames());
  const A11Y_FIXED = htmlAccessibilityAttributeNames();
  const elementSpecificAttributes = $derived(htmlAttributesForElement(tag));
  const targetOptions = $derived([
    { value: "", label: t("inspector-same-tab") },
    "_blank",
    "_self",
    "_parent",
    "_top",
  ]);
  const loadingOptions = $derived([{ value: "", label: t("inspector-default-eager") }, "lazy", "eager"]);
  const iframeLoadingOptions = $derived([{ value: "", label: t("inspector-default-eager") }, "lazy", "eager"]);
  const decodingOptions = $derived([{ value: "", label: t("inspector-default-auto") }, "async", "sync", "auto"]);
  const priorityOptions = ["auto", "high", "low"];
  const zolaOperationOptions = $derived([
    { value: "fit_width", label: t("inspector-zola-fit-width") },
    { value: "fit", label: t("inspector-zola-fit") },
    { value: "fill", label: t("inspector-zola-fill") },
  ]);
  const zolaFormatOptions = ["webp", "avif", "auto", "jpg", "png"];
  const buttonTypeOptions = $derived([{ value: "", label: t("inspector-default-submit") }, "button", "submit", "reset"]);
  const inputTypeOptions = ["text", "email", "password", "number", "tel", "url", "search", "date", "time", "datetime-local", "month", "week", "checkbox", "radio", "file", "color", "range", "hidden", "submit", "reset", "button"];
  const methodOptions = ["get", "post", "dialog"];
  const enctypeOptions = $derived([
    { value: "", label: t("inspector-default-urlencoded") },
    "multipart/form-data",
    "text/plain",
  ]);
  const preloadOptions = ["metadata", "auto", "none"];
  const dirOptions = $derived([
    { value: "", label: t("inspector-default-inherited") },
    "auto",
    "ltr",
    "rtl",
  ]);
  const contentEditableOptions = $derived([
    { value: "", label: t("inspector-default-inherited") },
    "true",
    "false",
    "plaintext-only",
  ]);
  const draggableOptions = $derived([
    { value: "", label: t("inspector-default-auto") },
    "true",
    "false",
  ]);
  const ariaBooleanOptions = $derived([
    { value: "", label: t("inspector-unset") },
    "true",
    "false",
  ]);

  const SPECIALIZED_ATTRIBUTES: Record<string, string[]> = {
    a: ["href", "target", "rel", "download", "hreflang"],
    img: ["src", "alt", "width", "height", "loading", "decoding", "fetchpriority"],
    button: ["type", "disabled", "name", "autofocus"],
    input: ["type", "name", "placeholder", "autocomplete", "min", "max", "required", "disabled", "readonly", "checked", "multiple"],
    textarea: ["name", "placeholder", "rows", "cols", "required", "disabled", "readonly"],
    select: ["name", "required", "disabled", "multiple"],
    form: ["action", "method", "enctype", "novalidate"],
    label: ["for"],
    video: ["src", "poster", "preload", "controls", "autoplay", "muted", "loop", "playsinline"],
    audio: ["src", "preload", "controls", "autoplay", "muted", "loop"],
    iframe: ["src", "loading", "sandbox"],
  };

  const additionalElementAttributes = $derived(
    elementSpecificAttributes.filter((name) => !(SPECIALIZED_ATTRIBUTES[tag] ?? []).includes(name)),
  );
  const sourceOnlySpecializedAttributes = $derived(
    (SPECIALIZED_ATTRIBUTES[tag] ?? []).filter((name) => htmlAttributePreviewMode(name, tag) !== "live"),
  );

  function schemaSelectOptions(name: string) {
    const definition = htmlAttributeDefinition(name);
    return [
      { value: "", label: definition?.implicitValue ? `implicit (${definition.implicitValue})` : t("inspector-unset") },
      ...(definition?.values ?? []),
    ];
  }

  function isBooleanPresence(name: string) {
    return htmlAttributeDefinition(name)?.semantic === "booleanPresence";
  }

  function hasSchemaSelect(name: string) {
    return Boolean(htmlAttributeDefinition(name)?.values?.length);
  }

  function attributeModeLabel(name: string) {
    const mode = htmlAttributePreviewMode(name, tag);
    if (mode === "sourceOnly") return t("inspector-mode-source-only");
    if (mode === "inert") return t("inspector-mode-inert");
    if (mode === "blocked") return t("inspector-mode-blocked");
    return "live";
  }

  const hasIdentity = $derived(Boolean(selectionSummary?.elementId || selectionSummary?.classes.length));
  const hasContent  = $derived(Boolean(textContentValue));
  const hasElementSpecific = $derived(elementSpecificAttributes.some(k => k in attributeValues));
  const hasGlobalAttrs = $derived([...GLOBAL_ATTRS].some(k => k in attributeValues));
  const hasA11y = $derived(A11Y_FIXED.some(k => k in attributeValues) || Object.keys(attributeValues).some(k => k.startsWith("aria-")));
  const hasData = $derived(dataAttrs.length > 0);
  const hasGeneratedClass = $derived(Boolean(selectionSummary?.classes.some((cls) => /^ps-[a-z0-9-]+-[a-z0-9]{6,}$/i.test(cls))));
  const hasDataAnimAttr = $derived(Boolean(attributeValues["data-anim"]?.trim()));

  // ── Asset helpers ────────────────────────────────────────────────────────

  const ext = (a: ProjectFile) => a.relativePath.split(".").pop()?.toLowerCase() ?? "";
  const imageAssets = $derived(scannedAssets.filter(a => a.kind === "IMAGE" || ["svg","webp","avif","png","jpg","jpeg","gif"].includes(ext(a))));
  const processableImageAssets = $derived(imageAssets.filter(a => ["webp","avif","png","jpg","jpeg"].includes(ext(a))));
  const zolaSourceResolution = $derived(resolveZolaImageSource(currentZolaSourceUrl(), scannedAssets));
  const videoAssets = $derived(scannedAssets.filter(a => ["mp4","webm","ogv","mov"].includes(ext(a))));
  const audioAssets = $derived(scannedAssets.filter(a => ["mp3","wav","ogg","m4a"].includes(ext(a))));

</script>

{#snippet fieldFeedback(names: string[], fallback = "")}
  {#if names.includes(fieldStatusName) && fieldStatusText}
    <p class:error={fieldStatusKind === "error"} class:success={fieldStatusKind === "success"} class="hf-field-status">{fieldStatusText}</p>
  {:else if fallback}
    <p class="hf-field-status">{fallback}</p>
  {/if}
{/snippet}

{#if !hasElementSelection}
  <InspectorEmptyState kind="html" title="HTML" description={t("inspector-select-element")} />
{:else}

<!-- ── ELEMENT ──────────────────────────────────────────────────────────── -->
<InspectorSection title={t("inspector-section-element")} hasValues={true}>
  {#snippet icon()}<IconCode size={13} stroke={1.7} />{/snippet}

  <div class="hf-row">
    <span class="hf-label">{t("inspector-element-tag")}</span>
    <SelectControl value={tag} options={tagOptions} disabled={!canChangeTag} ariaLabel={t("inspector-element-html-tag")} onchange={commitTagDraft} />
  </div>
  {#if tagCapability?.previewMode === "inert"}
    <p class="hf-capability-note">{tagCapability.reason ?? t("inspector-tag-inert")}</p>
  {:else if canEdit && !canChangeTag}
    <p class="hf-capability-note">{t("inspector-tag-no-transition")}</p>
  {/if}
  {@render fieldFeedback([], tagStatus)}
  <div class="hf-row">
    <span class="hf-label">{t("inspector-element-dimensions-short")}</span>
    <span class="hf-dims">{physicalFacts?.rect.width ?? "—"} × {physicalFacts?.rect.height ?? "—"}</span>
  </div>
  {#if !selectionSnapshot?.provenance?.definition && !selectionSnapshot?.provenance?.composition && !isActivePreviewHtmlSource}
    <p class="hf-warning">{t("inspector-no-source")}</p>
  {/if}
</InspectorSection>

<!-- ── IDENTITY ─────────────────────────────────────────────────────────── -->
<InspectorSection title={t("inspector-section-identity")} hasValues={hasIdentity}>
  {#snippet icon()}<IconFingerprint size={13} stroke={1.7} />{/snippet}

  <div class="hf-row">
    <span class="hf-label mono">#</span>
    <input
      class="hf-input"
      type="text"
      placeholder="id"
      value={getAttr("id")}
      disabled={!canEdit}
      oninput={(e) => setAttr("id", e.currentTarget.value)}
      onblur={() => commitField("id")}
    />
  </div>

  <div class="hf-subheader">
    <span class="hf-sublabel">{t("inspector-classes")}</span>
    {#if canEdit}
      <button type="button" class="hf-add-btn" aria-label={t("inspector-add-class")} onclick={() => { addingClass = true; }}>
        <IconPlus size={13} stroke={1.9} />
      </button>
    {/if}
  </div>

  {#if canEdit && !hasGeneratedClass}
    <button type="button" class="hf-ghost-add" onclick={() => { void generateClassForSelectedHtml(); }}>
      {t("inspector-generate-class")}
    </button>
  {/if}

  <div class="chip-list">
    {#if selectionSummary?.classes.length}
      {#each selectionSummary.classes as cls}
        <span class="cls-chip">
          <span class="cls-chip-name">{cls}</span>
          {#if canEdit}
            <button type="button" class="cls-chip-del" aria-label={t("inspector-remove-class", { name: cls })} onclick={() => removeClass(cls)}>
              <IconX size={12} stroke={2} />
            </button>
          {/if}
        </span>
      {/each}
    {:else}
      <span class="hint-inline">{t("inspector-no-classes")}</span>
    {/if}
    {#if addingClass}
      <div class="hf-add-row">
        <input
          class="hf-input"
          type="text"
          placeholder={t("inspector-class-placeholder")}
          bind:value={newClassName}
          onkeydown={(e) => { if (e.key === "Enter") addClass(); if (e.key === "Escape") { addingClass = false; newClassName = ""; } }}
        />
        <button type="button" class="hf-ok-btn" onclick={addClass}>{t("common-confirm")}</button>
      </div>
    {/if}
  </div>
</InspectorSection>
{@render fieldFeedback(["id"], canEdit ? "" : classStatus)}

<!-- ── CONTENT ──────────────────────────────────────────────────────────── -->
<InspectorSection title={t("inspector-section-content")} hasValues={hasContent}>
  {#snippet icon()}<IconAlignLeft size={13} stroke={1.7} />{/snippet}

  {#if physicalFacts?.hasChildElements}
    <p class="hint-inline">{t("inspector-child-elements", { count: physicalFacts.childElementCount })}</p>
  {:else}
    <textarea
      class="hf-textarea"
      rows="3"
      disabled={!canEdit}
      value={textContentValue}
      oncompositionstart={() => { textCompositionActive = true; }}
      oncompositionend={(e) => {
        textCompositionActive = false;
        updateTextContentValue(e.currentTarget.value, false);
      }}
      oninput={(e) => updateTextContentValue(
        e.currentTarget.value,
        textCompositionActive,
      )}
      onblur={() => applyTextContentToHtml()}
    ></textarea>
  {/if}
</InspectorSection>
{@render fieldFeedback([], canEdit ? "" : textStatus)}

<!-- ── ELEMENT SPECIFIC ─────────────────────────────────────────────────── -->

{#if tag === "a"}
  <InspectorSection title={t("inspector-section-link")} hasValues={hasElementSpecific}>
    {#snippet icon()}<IconLink size={13} stroke={1.7} />{/snippet}
    <div class="hf-row">
      <span class="hf-label">href</span>
      <input class="hf-input" type="text" placeholder="/" value={getAttr("href")} disabled={!canEdit}
        oninput={(e) => setAttr("href", e.currentTarget.value)} onblur={() => commitField("href")} />
    </div>
    <div class="hf-row">
      <span class="hf-label">target</span>
      <SelectControl value={getAttr("target")} options={targetOptions} disabled={!canEditAttribute("target")} ariaLabel={t("inspector-link-target")} onchange={(value) => commitField("target", value)} />
    </div>
    <div class="hf-row">
      <span class="hf-label">rel</span>
      <input class="hf-input" type="text" placeholder="noopener noreferrer" value={getAttr("rel")} disabled={!canEdit}
        oninput={(e) => setAttr("rel", e.currentTarget.value)} onblur={() => commitField("rel")} />
    </div>
    <div class="hf-row">
      <span class="hf-label">hreflang</span>
      <input class="hf-input" type="text" placeholder="ro" value={getAttr("hreflang")} disabled={!canEdit}
        oninput={(e) => setAttr("hreflang", e.currentTarget.value)} onblur={() => commitField("hreflang")} />
    </div>
    <div class="hf-row">
      <span class="hf-label">download</span>
      <button type="button" class="hf-toggle" class:active={getBool("download")} disabled={!canEdit}
        onclick={() => toggleBool("download", !getBool("download"))}>{getBool("download") ? "on" : "off"}</button>
    </div>
    {#if getBool("download")}
      <div class="hf-row">
        <span class="hf-label">filename</span>
        <input class="hf-input" type="text" placeholder={t("inspector-default-value")} value={getAttr("download")} disabled={!canEdit}
          oninput={(e) => setAttr("download", e.currentTarget.value)} onblur={() => commitField("download")} />
      </div>
    {/if}
  </InspectorSection>

{:else if tag === "img"}
  <InspectorSection title={t("inspector-section-image")} hasValues={hasElementSpecific || zolaImageEnabled}>
    {#snippet icon()}<IconPhoto size={13} stroke={1.7} />{/snippet}
    <div class="hf-sublabel-row"><span class="hf-sublabel">src</span></div>
    <AssetPicker
      value={zolaImageEnabled ? zolaSourceDraft : (imageSourceValue || getAttr("src"))}
      assets={zolaImageEnabled ? processableImageAssets : imageAssets}
      assetUrl={projectAssetPublicUrl}
      assetMeta={projectAssetOriginLabel}
      contextKey={assetContextKey}
      disabled={!canEditAttribute("src")}
      oninput={(v) => {
        if (zolaImageEnabled) zolaSourceDraft = v;
        else { setImageSourceValue(v); setAttr("src", v); }
      }}
      oncommit={(v) => {
        if (zolaImageEnabled) {
          zolaSourceDraft = v;
          void commitZolaImage(true);
        } else {
          setImageSourceValue(v);
          commitField("src", v);
        }
      }}
      oncancel={(_baseline, context) => cancelHtmlAttributeDraft(context)}
      commitOnInputMs={450}
    />
    <div class="hf-row">
      <span class="hf-label">alt</span>
      <input class="hf-input" type="text" placeholder={t("inspector-image-description-placeholder")} value={getAttr("alt")} disabled={!canEdit}
        oninput={(e) => setAttr("alt", e.currentTarget.value)} onblur={() => commitField("alt")} />
    </div>
    <div class="zola-image-box">
      <div class="hf-row">
        <span class="hf-label">{t("inspector-zola-process")}</span>
        <button
          type="button"
          class="hf-toggle"
          class:active={zolaImageEnabled}
          disabled={!canEdit || zolaImagePending || (!zolaImageEnabled && !zolaSourceResolution.eligible)}
          onclick={() => { void commitZolaImage(!zolaImageEnabled); }}
        >{zolaImageEnabled ? "on" : "off"}</button>
      </div>
      {#if !zolaImageEnabled && !zolaSourceResolution.eligible}
        <p class="hf-zola-note error">{zolaImageSourceFailureMessage(zolaSourceResolution.code)}</p>
      {:else if !zolaImageEnabled}
        <p class="hf-zola-note">{t("inspector-zola-build-note")}</p>
      {/if}
      {#if zolaImageEnabled}
        <div class="hf-row">
          <span class="hf-label">{t("inspector-zola-operation")}</span>
          <SelectControl
            value={zolaOperation}
            options={zolaOperationOptions}
            disabled={!canEdit || zolaImagePending}
            ariaLabel={t("inspector-zola-operation-label")}
            onchange={(value) => {
              zolaOperation = value as ZolaImageOperation;
              void commitZolaImage(true);
            }}
          />
        </div>
        <div class="hf-row-2">
          <div class="hf-row">
            <span class="hf-label">w</span>
            <input class="hf-input" type="number" min="1" max="16384" value={zolaWidthText}
              disabled={!canEdit || zolaImagePending}
              oninput={(event) => { zolaWidthText = event.currentTarget.value; }}
              onblur={() => { void commitZolaImage(true); }} />
          </div>
          {#if zolaOperation !== "fit_width"}
            <div class="hf-row">
              <span class="hf-label">h</span>
              <input class="hf-input" type="number" min="1" max="16384" value={zolaHeightText}
                disabled={!canEdit || zolaImagePending}
                oninput={(event) => { zolaHeightText = event.currentTarget.value; }}
                onblur={() => { void commitZolaImage(true); }} />
            </div>
          {/if}
        </div>
        <div class="hf-row">
          <span class="hf-label">format</span>
          <SelectControl
            value={zolaFormat}
            options={zolaFormatOptions}
            disabled={!canEdit || zolaImagePending}
            ariaLabel={t("inspector-zola-format-label")}
            onchange={(value) => {
              zolaFormat = value as ZolaImageFormat;
              void commitZolaImage(true);
            }}
          />
        </div>
        {#if zolaFormat !== "png"}
          <div class="hf-row">
            <span class="hf-label">{t("inspector-zola-quality")}</span>
            <input class="hf-input" type="number" min="1" max="100" value={zolaQualityText}
              disabled={!canEdit || zolaImagePending}
              oninput={(event) => { zolaQualityText = event.currentTarget.value; }}
              onblur={() => { void commitZolaImage(true); }} />
          </div>
        {/if}
        <p class="hf-zola-note">{t("inspector-zola-managed-note")}</p>
      {:else}
        <div class="hf-row-2">
          <div class="hf-row">
            <span class="hf-label">w</span>
            <input class="hf-input" type="text" placeholder="800" value={getAttr("width")} disabled={!canEdit}
              oninput={(e) => setAttr("width", e.currentTarget.value)} onblur={() => commitField("width")} />
          </div>
          <div class="hf-row">
            <span class="hf-label">h</span>
            <input class="hf-input" type="text" placeholder="600" value={getAttr("height")} disabled={!canEdit}
              oninput={(e) => setAttr("height", e.currentTarget.value)} onblur={() => commitField("height")} />
          </div>
        </div>
      {/if}
    </div>
    {@render fieldFeedback(["zola-image"], canEdit ? "" : imageStatus)}
    <div class="hf-row">
      <span class="hf-label">loading</span>
      <SelectControl value={getAttr("loading")} options={loadingOptions} disabled={!canEditAttribute("loading")} ariaLabel={t("inspector-image-loading")} onchange={(value) => commitField("loading", value)} />
    </div>
    <div class="hf-row">
      <span class="hf-label">decoding</span>
      <SelectControl value={getAttr("decoding")} options={decodingOptions} disabled={!canEditAttribute("decoding")} ariaLabel={t("inspector-image-decoding")} onchange={(value) => commitField("decoding", value)} />
    </div>
    <div class="hf-row">
      <span class="hf-label">priority</span>
      <SelectControl value={getAttr("fetchpriority") || "auto"} options={priorityOptions} disabled={!canEdit} ariaLabel={t("inspector-image-priority")} onchange={(value) => commitAttribute("fetchpriority", value)} />
    </div>
  </InspectorSection>

{:else if tag === "button"}
  <InspectorSection title={t("inspector-section-button")} hasValues={hasElementSpecific}>
    {#snippet icon()}<IconPointer size={13} stroke={1.7} />{/snippet}
    <div class="hf-row">
      <span class="hf-label">type</span>
      <SelectControl value={getAttr("type")} options={buttonTypeOptions} disabled={!canEditAttribute("type")} ariaLabel={t("inspector-button-type")} onchange={(value) => commitField("type", value)} />
    </div>
    <div class="hf-row">
      <span class="hf-label">name</span>
      <input class="hf-input" type="text" value={getAttr("name")} disabled={!canEdit}
        oninput={(e) => setAttr("name", e.currentTarget.value)} onblur={() => commitField("name")} />
    </div>
    <div class="hf-bools">
      {#each ["disabled", "autofocus"] as b}
        <button type="button" class="hf-bool-chip" class:active={getBool(b)} disabled={!canEdit}
          onclick={() => toggleBool(b, !getBool(b))}>{b}</button>
      {/each}
    </div>
  </InspectorSection>

{:else if tag === "input"}
  <InspectorSection title={t("inspector-section-input")} hasValues={hasElementSpecific}>
    {#snippet icon()}<IconForms size={13} stroke={1.7} />{/snippet}
    <div class="hf-row">
      <span class="hf-label">type</span>
      <SelectControl value={getAttr("type") || "text"} options={inputTypeOptions} disabled={!canEdit} ariaLabel={t("inspector-input-type")} onchange={(value) => commitAttribute("type", value)} />
    </div>
    <div class="hf-row">
      <span class="hf-label">name</span>
      <input class="hf-input" type="text" value={getAttr("name")} disabled={!canEdit}
        oninput={(e) => setAttr("name", e.currentTarget.value)} onblur={() => commitField("name")} />
    </div>
    <div class="hf-row">
      <span class="hf-label">placeholder</span>
      <input class="hf-input" type="text" value={getAttr("placeholder")} disabled={!canEdit}
        oninput={(e) => setAttr("placeholder", e.currentTarget.value)} onblur={() => commitField("placeholder")} />
    </div>
    <div class="hf-row">
      <span class="hf-label">autocomplete</span>
      <input class="hf-input" type="text" placeholder="off" value={getAttr("autocomplete")} disabled={!canEdit}
        oninput={(e) => setAttr("autocomplete", e.currentTarget.value)} onblur={() => commitField("autocomplete")} />
    </div>
    <div class="hf-row-2">
      <div class="hf-row">
        <span class="hf-label">min</span>
        <input class="hf-input" type="text" value={getAttr("min")} disabled={!canEdit}
          oninput={(e) => setAttr("min", e.currentTarget.value)} onblur={() => commitField("min")} />
      </div>
      <div class="hf-row">
        <span class="hf-label">max</span>
        <input class="hf-input" type="text" value={getAttr("max")} disabled={!canEdit}
          oninput={(e) => setAttr("max", e.currentTarget.value)} onblur={() => commitField("max")} />
      </div>
    </div>
    <div class="hf-bools">
      {#each ["required","disabled","readonly","checked","multiple"] as b}
        <button type="button" class="hf-bool-chip" class:active={getBool(b)} disabled={!canEdit}
          onclick={() => toggleBool(b, !getBool(b))}>{b}</button>
      {/each}
    </div>
  </InspectorSection>

{:else if tag === "textarea"}
  <InspectorSection title={t("inspector-section-textarea")} hasValues={hasElementSpecific}>
    {#snippet icon()}<IconForms size={13} stroke={1.7} />{/snippet}
    <div class="hf-row">
      <span class="hf-label">name</span>
      <input class="hf-input" type="text" value={getAttr("name")} disabled={!canEdit}
        oninput={(e) => setAttr("name", e.currentTarget.value)} onblur={() => commitField("name")} />
    </div>
    <div class="hf-row">
      <span class="hf-label">placeholder</span>
      <input class="hf-input" type="text" value={getAttr("placeholder")} disabled={!canEdit}
        oninput={(e) => setAttr("placeholder", e.currentTarget.value)} onblur={() => commitField("placeholder")} />
    </div>
    <div class="hf-row-2">
      <div class="hf-row">
        <span class="hf-label">rows</span>
        <input class="hf-input" type="text" placeholder="4" value={getAttr("rows")} disabled={!canEdit}
          oninput={(e) => setAttr("rows", e.currentTarget.value)} onblur={() => commitField("rows")} />
      </div>
      <div class="hf-row">
        <span class="hf-label">cols</span>
        <input class="hf-input" type="text" value={getAttr("cols")} disabled={!canEdit}
          oninput={(e) => setAttr("cols", e.currentTarget.value)} onblur={() => commitField("cols")} />
      </div>
    </div>
    <div class="hf-bools">
      {#each ["required","disabled","readonly"] as b}
        <button type="button" class="hf-bool-chip" class:active={getBool(b)} disabled={!canEdit}
          onclick={() => toggleBool(b, !getBool(b))}>{b}</button>
      {/each}
    </div>
  </InspectorSection>

{:else if tag === "select"}
  <InspectorSection title={t("inspector-section-select")} hasValues={hasElementSpecific}>
    {#snippet icon()}<IconForms size={13} stroke={1.7} />{/snippet}
    <div class="hf-row">
      <span class="hf-label">name</span>
      <input class="hf-input" type="text" value={getAttr("name")} disabled={!canEdit}
        oninput={(e) => setAttr("name", e.currentTarget.value)} onblur={() => commitField("name")} />
    </div>
    <div class="hf-bools">
      {#each ["required","disabled","multiple"] as b}
        <button type="button" class="hf-bool-chip" class:active={getBool(b)} disabled={!canEdit}
          onclick={() => toggleBool(b, !getBool(b))}>{b}</button>
      {/each}
    </div>
  </InspectorSection>

{:else if tag === "form"}
  <InspectorSection title={t("inspector-section-form")} hasValues={hasElementSpecific}>
    {#snippet icon()}<IconForms size={13} stroke={1.7} />{/snippet}
    <div class="hf-row">
      <span class="hf-label">action</span>
      <input class="hf-input" type="text" placeholder="/contact" value={getAttr("action")} disabled={!canEdit}
        oninput={(e) => setAttr("action", e.currentTarget.value)} onblur={() => commitField("action")} />
    </div>
    <div class="hf-row">
      <span class="hf-label">method</span>
      <SelectControl value={getAttr("method") || "get"} options={methodOptions} disabled={!canEdit} ariaLabel={t("inspector-form-method")} onchange={(value) => commitAttribute("method", value)} />
    </div>
    <div class="hf-row">
      <span class="hf-label">enctype</span>
      <SelectControl value={getAttr("enctype")} options={enctypeOptions} disabled={!canEditAttribute("enctype")} ariaLabel={t("inspector-form-enctype")} onchange={(value) => commitField("enctype", value)} />
    </div>
    <div class="hf-row">
      <span class="hf-label">novalidate</span>
      <button type="button" class="hf-toggle" class:active={getBool("novalidate")} disabled={!canEdit}
        onclick={() => toggleBool("novalidate", !getBool("novalidate"))}>{getBool("novalidate") ? "on" : "off"}</button>
    </div>
  </InspectorSection>

{:else if tag === "label"}
  <InspectorSection title={t("inspector-section-label")} hasValues={"for" in attributeValues}>
    {#snippet icon()}<IconTag size={13} stroke={1.7} />{/snippet}
    <div class="hf-row">
      <span class="hf-label">for</span>
      <input class="hf-input" type="text" placeholder="input-id" value={getAttr("for")} disabled={!canEdit}
        oninput={(e) => setAttr("for", e.currentTarget.value)} onblur={() => commitField("for")} />
    </div>
  </InspectorSection>

{:else if tag === "video"}
  <InspectorSection title={t("inspector-section-video")} hasValues={hasElementSpecific}>
    {#snippet icon()}<IconVideo size={13} stroke={1.7} />{/snippet}
    <div class="hf-sublabel-row"><span class="hf-sublabel">src</span></div>
    <AssetPicker value={getAttr("src")} assets={videoAssets} assetUrl={projectAssetPublicUrl} assetMeta={projectAssetOriginLabel}
      contextKey={assetContextKey} disabled={!canEditAttribute("src")}
      oninput={(v) => setAttr("src", v)} oncommit={(v) => commitField("src", v)}
      oncancel={(_baseline, context) => cancelHtmlAttributeDraft(context)} commitOnInputMs={450} />
    <div class="hf-row">
      <span class="hf-label">poster</span>
      <input class="hf-input" type="text" value={getAttr("poster")} disabled={!canEdit}
        oninput={(e) => setAttr("poster", e.currentTarget.value)} onblur={() => commitField("poster")} />
    </div>
    <div class="hf-row">
      <span class="hf-label">preload</span>
      <SelectControl value={getAttr("preload") || "metadata"} options={preloadOptions} disabled={!canEdit} ariaLabel={t("inspector-video-preload")} onchange={(value) => commitAttribute("preload", value)} />
    </div>
    <div class="hf-bools">
      {#each ["controls","autoplay","muted","loop","playsinline"] as b}
        <button type="button" class="hf-bool-chip" class:active={getBool(b)} disabled={!canEdit}
          onclick={() => toggleBool(b, !getBool(b))}>{b}</button>
      {/each}
    </div>
  </InspectorSection>

{:else if tag === "audio"}
  <InspectorSection title={t("inspector-section-audio")} hasValues={hasElementSpecific}>
    {#snippet icon()}<IconVolume2 size={13} stroke={1.7} />{/snippet}
    <div class="hf-sublabel-row"><span class="hf-sublabel">src</span></div>
    <AssetPicker value={getAttr("src")} assets={audioAssets} assetUrl={projectAssetPublicUrl} assetMeta={projectAssetOriginLabel}
      contextKey={assetContextKey} disabled={!canEditAttribute("src")}
      oninput={(v) => setAttr("src", v)} oncommit={(v) => commitField("src", v)}
      oncancel={(_baseline, context) => cancelHtmlAttributeDraft(context)} commitOnInputMs={450} />
    <div class="hf-row">
      <span class="hf-label">preload</span>
      <SelectControl value={getAttr("preload") || "metadata"} options={preloadOptions} disabled={!canEdit} ariaLabel={t("inspector-audio-preload")} onchange={(value) => commitAttribute("preload", value)} />
    </div>
    <div class="hf-bools">
      {#each ["controls","autoplay","muted","loop"] as b}
        <button type="button" class="hf-bool-chip" class:active={getBool(b)} disabled={!canEdit}
          onclick={() => toggleBool(b, !getBool(b))}>{b}</button>
      {/each}
    </div>
  </InspectorSection>

{:else if tag === "iframe"}
  <InspectorSection title={t("inspector-section-iframe")} hasValues={hasElementSpecific}>
    {#snippet icon()}<IconBrowser size={13} stroke={1.7} />{/snippet}
    <div class="hf-row">
      <span class="hf-label">src</span>
      <input class="hf-input" type="text" value={getAttr("src")} disabled={!canEdit}
        oninput={(e) => setAttr("src", e.currentTarget.value)} onblur={() => commitField("src")} />
    </div>
    <div class="hf-row">
      <span class="hf-label">title</span>
      <input class="hf-input" type="text" value={getAttr("title")} disabled={!canEdit}
        oninput={(e) => setAttr("title", e.currentTarget.value)} onblur={() => commitField("title")} />
    </div>
    <div class="hf-row">
      <span class="hf-label">loading</span>
      <SelectControl value={getAttr("loading")} options={iframeLoadingOptions} disabled={!canEditAttribute("loading")} ariaLabel={t("inspector-iframe-loading")} onchange={(value) => commitField("loading", value)} />
    </div>
    <div class="hf-row">
      <span class="hf-label">sandbox</span>
      <input class="hf-input" type="text" placeholder="allow-scripts" value={getAttr("sandbox")} disabled={!canEdit}
        oninput={(e) => setAttr("sandbox", e.currentTarget.value)} onblur={() => commitField("sandbox")} />
    </div>
  </InspectorSection>
{/if}

{#if sourceOnlySpecializedAttributes.length}
  <p class="hf-capability-note">
    {t("inspector-source-only-attributes", { attributes: sourceOnlySpecializedAttributes.join(", ") })}
  </p>
{/if}

{#if additionalElementAttributes.length}
  <InspectorSection title={t("inspector-section-tag-attributes", { tag })} hasValues={hasElementSpecific}>
    {#snippet icon()}<IconTag size={13} stroke={1.7} />{/snippet}
    {#each additionalElementAttributes as name}
      {@const definition = htmlAttributeDefinition(name)}
      <div class="hf-schema-field">
        <div class="hf-schema-label-row">
          <span class="hf-label">{name}</span>
          <span class:source-only={attributeModeLabel(name) !== "live"} class="hf-mode-chip">{attributeModeLabel(name)}</span>
        </div>
        {#if isBooleanPresence(name)}
          <button
            type="button"
            class="hf-toggle"
            class:active={getBool(name)}
            disabled={!canEditAttribute(name)}
            onclick={() => toggleBool(name, !getBool(name))}
          >{getBool(name) ? "on" : "off"}</button>
        {:else if hasSchemaSelect(name)}
          <SelectControl
            value={getAttr(name)}
            options={schemaSelectOptions(name)}
            disabled={!canEditAttribute(name)}
            ariaLabel={t("inspector-attribute-label", { name })}
            onchange={(value) => commitField(name, value)}
          />
        {:else}
          <input
            class="hf-input"
            type="text"
            value={getAttr(name)}
            disabled={!canEditAttribute(name)}
            oninput={(event) => setAttr(name, event.currentTarget.value)}
            onblur={(event) => commitField(name, event.currentTarget.value)}
          />
        {/if}
        {#if definition?.reason}
          <span class="hf-schema-reason">{definition.reason}</span>
        {/if}
      </div>
    {/each}
  </InspectorSection>
{/if}
{@render fieldFeedback(elementSpecificAttributes, tag === "img" && !canEdit ? imageStatus : "")}

<!-- ── ATRIBUTE ──────────────────────────────────────────────────────────── -->
<InspectorSection title={t("inspector-section-attributes")} hasValues={hasGlobalAttrs}>
  {#snippet icon()}<IconAdjustments size={13} stroke={1.7} />{/snippet}

  <div class="hf-row">
    <span class="hf-label">title</span>
    <input class="hf-input" type="text" value={getAttr("title")} disabled={!canEdit}
      oninput={(e) => setAttr("title", e.currentTarget.value)} onblur={() => commitField("title")} />
  </div>
  <div class="hf-row">
    <span class="hf-label">lang</span>
    <input class="hf-input" type="text" placeholder="ro" value={getAttr("lang")} disabled={!canEdit}
      oninput={(e) => setAttr("lang", e.currentTarget.value)} onblur={() => commitField("lang")} />
  </div>
  <div class="hf-row">
    <span class="hf-label">dir</span>
    <SelectControl value={getAttr("dir")} options={dirOptions} disabled={!canEditAttribute("dir")} ariaLabel={t("inspector-text-direction")} onchange={(value) => commitField("dir", value)} />
  </div>
  <div class="hf-row">
    <span class="hf-label">tabindex</span>
    <input class="hf-input" type="text" placeholder="0" value={getAttr("tabindex")} disabled={!canEdit}
      oninput={(e) => setAttr("tabindex", e.currentTarget.value)} onblur={() => commitField("tabindex")} />
  </div>
  <div class="hf-bools">
    {#each [["hidden","hidden"],["inert","inert"]] as [name, label]}
      <button type="button" class="hf-bool-chip" class:active={getBool(name)} disabled={!canEdit}
        onclick={() => toggleBool(name, !getBool(name))}>{label}</button>
    {/each}
  </div>
  <div class="hf-row">
    <span class="hf-label">editable</span>
    <SelectControl value={getAttr("contenteditable")} options={contentEditableOptions} disabled={!canEditAttribute("contenteditable")} ariaLabel={t("inspector-content-editable")} onchange={(value) => commitField("contenteditable", value)} />
  </div>
  <div class="hf-row">
    <span class="hf-label">draggable</span>
    <SelectControl value={getAttr("draggable")} options={draggableOptions} disabled={!canEditAttribute("draggable")} ariaLabel={t("inspector-draggable")} onchange={(value) => commitField("draggable", value)} />
  </div>
  {@render fieldFeedback([...GLOBAL_ATTRS])}
</InspectorSection>

<!-- ── ACCESSIBILITY ────────────────────────────────────────────────────── -->
<InspectorSection title={t("inspector-section-accessibility")} hasValues={hasA11y}>
  {#snippet icon()}<IconAccessible size={13} stroke={1.7} />{/snippet}

  <div class="hf-row">
    <span class="hf-label">role</span>
    <input class="hf-input" type="text" placeholder="none" value={getAttr("role")} disabled={!canEdit}
      oninput={(e) => setAttr("role", e.currentTarget.value)} onblur={() => commitField("role")} />
  </div>
  <div class="hf-row">
    <span class="hf-label">aria-label</span>
    <input class="hf-input" type="text" value={getAttr("aria-label")} disabled={!canEdit}
      oninput={(e) => setAttr("aria-label", e.currentTarget.value)} onblur={() => commitField("aria-label")} />
  </div>
  <div class="hf-row">
    <span class="hf-label">labelledby</span>
    <input class="hf-input" type="text" placeholder="element-id" value={getAttr("aria-labelledby")} disabled={!canEdit}
      oninput={(e) => setAttr("aria-labelledby", e.currentTarget.value)} onblur={() => commitField("aria-labelledby")} />
  </div>
  <div class="hf-row">
    <span class="hf-label">describedby</span>
    <input class="hf-input" type="text" placeholder="element-id" value={getAttr("aria-describedby")} disabled={!canEdit}
      oninput={(e) => setAttr("aria-describedby", e.currentTarget.value)} onblur={() => commitField("aria-describedby")} />
  </div>
  {#each [["aria-hidden","aria hidden"],["aria-expanded","aria expanded"],["aria-disabled","aria disabled"]] as [name, label]}
    <div class="hf-row">
      <span class="hf-label">{label}</span>
      <SelectControl value={getAttr(name)} options={ariaBooleanOptions} disabled={!canEditAttribute(name)} ariaLabel={label} onchange={(value) => commitField(name, value)} />
    </div>
  {/each}

  {#if addingAria}
    <div class="hf-add-row">
      <input class="hf-input" type="text" placeholder="aria-live" bind:value={newAriaKey}
        onkeydown={(e) => { if (e.key === "Enter") confirmAddAria(); if (e.key === "Escape") { addingAria = false; } }} />
      <input class="hf-input" type="text" placeholder={t("inspector-value-placeholder")} bind:value={newAriaValue}
        onkeydown={(e) => { if (e.key === "Enter") confirmAddAria(); if (e.key === "Escape") { addingAria = false; } }} />
      <button type="button" class="hf-ok-btn" onclick={confirmAddAria}>{t("common-confirm")}</button>
    </div>
  {:else if canEdit}
    <button type="button" class="hf-ghost-add" onclick={() => { addingAria = true; }}>
      <IconPlus size={13} stroke={1.9} aria-hidden="true" />
      {t("inspector-add-aria")}
    </button>
  {/if}
  {@render fieldFeedback(A11Y_FIXED)}
</InspectorSection>

<!-- ── DATA ATTRIBUTES ───────────────────────────────────────────────────── -->
<InspectorSection title={t("inspector-section-data")} hasValues={hasData}>
  {#snippet icon()}<IconDatabase size={13} stroke={1.7} />{/snippet}

  {#if canEdit && !hasDataAnimAttr}
    <button type="button" class="hf-ghost-add" onclick={() => { void generateDataAnimForSelectedHtml(); }}>
      {t("inspector-generate-data-anim")}
    </button>
  {/if}

  {#each dataAttrs as [key, val]}
    <div class="hf-data-row">
      <span class="hf-data-key">{key}</span>
      <input class="hf-input" type="text" value={val} disabled={!canEdit}
        oninput={(e) => setAttr(key, e.currentTarget.value)}
        onblur={(e) => commitAttribute(key, e.currentTarget.value)} />
      {#if canEdit}
        <button type="button" class="hf-del-btn" aria-label={t("inspector-remove-attribute", { name: key })}
          onclick={() => commitAttributeRemoval(key)}>
          <IconX size={13} stroke={1.9} />
        </button>
      {/if}
    </div>
  {/each}

  {#if addingData}
    <div class="hf-add-row">
      <input class="hf-input" type="text" placeholder="data-component" bind:value={newDataKey}
        onkeydown={(e) => { if (e.key === "Enter") confirmAddData(); if (e.key === "Escape") { addingData = false; newDataKey = ""; newDataValue = ""; } }} />
      <input class="hf-input" type="text" placeholder={t("inspector-value-placeholder")} bind:value={newDataValue}
        onkeydown={(e) => { if (e.key === "Enter") confirmAddData(); if (e.key === "Escape") { addingData = false; } }} />
      <button type="button" class="hf-ok-btn" onclick={confirmAddData}>{t("common-confirm")}</button>
    </div>
  {:else if canEdit}
    <button type="button" class="hf-ghost-add" onclick={() => { addingData = true; }}>
      <IconPlus size={13} stroke={1.9} aria-hidden="true" />
      {t("inspector-add-data")}
    </button>
  {/if}
  {@render fieldFeedback(dataAttrs.map(([name]) => name))}
</InspectorSection>

{/if}

<style>
  /* ── Field row ─────────────────────────────────────────────────────────── */

  .hf-row {
    display: grid;
    grid-template-columns: 72px 1fr;
    align-items: center;
    gap: 5px;
    min-height: 26px;
  }

  .hf-row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .hf-label {
    font-size: 11px;
    font-weight: 650;
    color: var(--text-muted);
    letter-spacing: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    user-select: none;
  }

  .hf-label.mono {
    font-family: "JetBrains Mono", monospace;
    font-size: 13px;
    font-weight: 900;
    text-transform: none;
    letter-spacing: 0;
    color: var(--brand-strong);
  }

  .hf-dims {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    color: var(--text-muted);
  }

  /* ── Input ─────────────────────────────────────────────────────────────── */

  .hf-input {
    width: 100%;
    min-width: 0;
    height: 26px;
    padding: 0 7px;
    border: 1px solid var(--border-4);
    border-radius: var(--radius-control);
    background: var(--material-inset);
    box-shadow: var(--shadow-inset);
    color: var(--text);
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    outline: none;
    transition:
      border-color 100ms ease,
      box-shadow 100ms ease;
  }

  .hf-input:focus {
    border-color: var(--brand);
    box-shadow:
      var(--shadow-inset),
      0 0 0 1px color-mix(in srgb, var(--focus-ring) 38%, transparent);
  }

  .hf-input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* ── Toggle ────────────────────────────────────────────────────────────── */

  .hf-toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 24px;
    padding: 0 10px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-5);
    color: var(--text-muted);
    font-size: 12px;
    font-family: "JetBrains Mono", monospace;
    cursor: pointer;
    transition: border-color 80ms, color 80ms, background 80ms;
  }

  .hf-toggle.active {
    border-color: var(--brand);
    color: var(--brand-strong);
    background: var(--brand-soft);
  }

  .hf-toggle:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .zola-image-box {
    display: grid;
    gap: 7px;
    padding: 8px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: color-mix(in srgb, var(--brand-soft) 28%, var(--surface-4));
  }

  .hf-zola-note {
    margin: 0;
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.45;
  }

  .hf-zola-note.error {
    color: var(--danger, #cf4a4a);
  }

  /* ── Bool chips ────────────────────────────────────────────────────────── */

  .hf-bools {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .hf-bool-chip {
    display: inline-flex;
    align-items: center;
    height: 22px;
    padding: 0 8px;
    border: 1px solid var(--border-4);
    border-radius: 999px;
    background: var(--surface-5);
    color: var(--text-muted);
    font-size: 12px;
    font-family: "JetBrains Mono", monospace;
    font-weight: 600;
    cursor: pointer;
    transition: border-color 80ms, color 80ms, background 80ms;
    white-space: nowrap;
  }

  .hf-bool-chip.active {
    border-color: var(--brand);
    color: var(--brand-strong);
    background: var(--brand-soft);
  }

  .hf-bool-chip:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* ── Subheader / labels ────────────────────────────────────────────────── */

  .hf-subheader {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }

  .hf-sublabel {
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0;
    color: var(--text-muted);
    user-select: none;
  }

  .hf-sublabel-row {
    display: flex;
    align-items: center;
  }

  /* ── Add buttons ───────────────────────────────────────────────────────── */

  .hf-add-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: 1px solid var(--border-3);
    border-radius: calc(var(--radius-control) - 2px);
    background: var(--material-control);
    box-shadow: var(--shadow-control);
    color: var(--text-muted);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    transition: color 80ms, border-color 80ms;
  }

  .hf-add-btn:hover {
    color: var(--brand-strong);
    border-color: var(--brand);
  }

  .hf-ghost-add {
    display: flex;
    align-items: center;
    height: 26px;
    padding: 0 6px;
    border: 1px dashed var(--border-3);
    border-radius: var(--radius-control);
    background: color-mix(in srgb, var(--surface-inset) 44%, transparent);
    box-shadow: inset 0 1px 2px color-mix(in srgb, var(--skeuo-shade) 28%, transparent);
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: color 80ms, border-color 80ms;
    width: 100%;
  }

  .hf-ghost-add:hover {
    color: var(--brand-strong);
    border-color: var(--brand);
  }

  /* ── Add row ───────────────────────────────────────────────────────────── */

  .hf-add-row {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .hf-add-row .hf-input {
    flex: 1;
  }

  .hf-ok-btn {
    flex-shrink: 0;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--brand);
    border-radius: 6px;
    background: var(--brand);
    color: #fff;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
  }

  /* ── Del button ────────────────────────────────────────────────────────── */

  .hf-del-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex-shrink: 0;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--text-muted);
    font-size: 14px;
    cursor: pointer;
    transition: color 80ms, background 80ms;
  }

  .hf-del-btn:hover {
    color: #cf4a4a;
    background: color-mix(in srgb, #cf4a4a 15%, transparent);
  }

  /* ── Data row ──────────────────────────────────────────────────────────── */

  .hf-data-row {
    display: grid;
    grid-template-columns: 1fr 1fr 20px;
    gap: 4px;
    align-items: center;
  }

  .hf-data-key {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 600;
    color: var(--brand-strong);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ── Classes ───────────────────────────────────────────────────────────── */

  .chip-list {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    align-items: center;
  }

  .cls-chip {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 2px 6px 2px 8px;
    border: 1px solid var(--border-4);
    border-radius: 20px;
    background: var(--surface-5);
    font-size: 12px;
    color: var(--text);
  }

  .cls-chip-name {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
  }

  .cls-chip-del {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    line-height: 1;
    transition: color 80ms, background 80ms;
  }

  .cls-chip-del:hover {
    color: #cf4a4a;
    background: color-mix(in srgb, #cf4a4a 15%, transparent);
  }

  /* ── Misc ──────────────────────────────────────────────────────────────── */

  .hint-inline {
    font-size: 12px;
    color: var(--text-muted);
  }

  .hf-textarea {
    width: 100%;
    min-height: 64px;
    padding: 6px 8px;
    border: 1px solid var(--border-4);
    border-radius: var(--radius-control);
    background: var(--material-inset);
    box-shadow: var(--shadow-inset);
    color: var(--text);
    font-size: 12px;
    font-family: inherit;
    resize: vertical;
    outline: none;
    box-sizing: border-box;
    transition:
      border-color 100ms ease,
      box-shadow 100ms ease;
  }

  .hf-textarea:focus {
    border-color: var(--brand);
    box-shadow:
      var(--shadow-inset),
      0 0 0 1px color-mix(in srgb, var(--focus-ring) 38%, transparent);
  }

  .hf-textarea:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .hf-warning {
    margin: 0;
    padding: 5px 7px;
    border-radius: 6px;
    background: color-mix(in srgb, #f59e0b 10%, transparent);
    border: 1px solid color-mix(in srgb, #f59e0b 30%, transparent);
    color: #92400e;
    font-size: 12px;
  }

  .hf-capability-note,
  .hf-field-status,
  .hf-schema-reason {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.35;
  }

  .hf-field-status {
    padding: 5px 7px;
    border: 1px solid var(--border-3);
    border-radius: 6px;
    background: var(--surface-4);
  }

  .hf-field-status.success {
    border-color: color-mix(in srgb, var(--brand) 40%, var(--border-3));
    color: var(--brand-strong);
  }

  .hf-field-status.error {
    border-color: color-mix(in srgb, #cf4a4a 45%, var(--border-3));
    color: #b83c3c;
  }

  .hf-capability-note {
    padding: 5px 7px;
    border: 1px solid color-mix(in srgb, #f59e0b 30%, var(--border-3));
    border-radius: 6px;
    background: color-mix(in srgb, #f59e0b 8%, var(--surface-4));
  }

  .hf-schema-field {
    display: grid;
    gap: 4px;
  }

  .hf-schema-label-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }

  .hf-mode-chip {
    padding: 1px 5px;
    border: 1px solid var(--border-3);
    border-radius: 999px;
    color: var(--brand-strong);
    background: var(--brand-soft);
    font-size: 12px;
    font-weight: 800;
    text-transform: uppercase;
  }

  .hf-mode-chip.source-only {
    color: #92400e;
    background: color-mix(in srgb, #f59e0b 10%, var(--surface-4));
  }

</style>
