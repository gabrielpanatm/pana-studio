<script lang="ts">
  import {
    IconAdjustmentsHorizontal,
    IconCut,
    IconDownload,
    IconPalette,
    IconPhotoCheck,
    IconPhotoEdit,
    IconShadow,
    IconUnlink,
  } from "@tabler/icons-svelte";
  import MoodBoardImageAdjustmentsPopover from "$lib/components/canvas/MoodBoardImageAdjustmentsPopover.svelte";
  import {
    isDefaultImageAdjustments,
    normalizeImageAdjustments,
  } from "$lib/mood-board/image-adjustments";
  import {
    isDefaultImageFraming,
    normalizeImageFraming,
  } from "$lib/mood-board/image-framing";
  import {
    moodBoardImageWithAdjustment,
    moodBoardImageWithDefaultAdjustments,
    moodBoardImageWithDefaultFraming,
    moodBoardImageWithFit,
    moodBoardImageWithFraming,
    moodBoardImageWithPath,
    moodBoardImageWithRadius,
    moodBoardImageWithToggledShadow,
  } from "$lib/mood-board/context-actions";
  import { cloneMoodBoardItem } from "$lib/mood-board/item-view";
  import type { MoodBoardImageItem, MoodBoardItem } from "$lib/mood-board/model";

  export let imageItem: MoodBoardImageItem;
  export let previewItem: (item: MoodBoardItem) => void;
  export let commitItemEdit: (beforeItem: MoodBoardItem, nextItem: MoodBoardItem) => void;
  export let applyImageToSelectedElement: ((path: string) => void | Promise<void>) | undefined = undefined;
  export let extractPaletteFromImage: ((itemId: string, path: string) => void | Promise<void>) | undefined = undefined;
  export let canApplyVectorMask: (itemId: string) => boolean = () => false;
  export let applyVectorMask: (itemId: string) => void = () => undefined;
  export let clearVectorMask: (itemId: string) => void = () => undefined;
  export let exportImageWebp: (itemId: string) => void | Promise<void> = () => undefined;
  export let heavyAssetBusy = false;

  let editBeforeItem: MoodBoardImageItem | null = null;
  let imageAdjustmentsOpen = false;

  $: imageAdjustments = normalizeImageAdjustments(imageItem.adjustments);
  $: imageFraming = normalizeImageFraming(imageItem.framing);

  function cloneItem(value: MoodBoardImageItem): MoodBoardImageItem {
    return cloneMoodBoardItem(value);
  }

  function beginEdit() {
    editBeforeItem ??= cloneItem(imageItem);
  }

  function commitEdit(nextItem: MoodBoardImageItem) {
    if (!editBeforeItem) return;
    const before = editBeforeItem;
    editBeforeItem = null;
    commitItemEdit(before, nextItem);
  }

  function updateImageFit(value: "cover" | "contain") {
    commitItemEdit(cloneItem(imageItem), moodBoardImageWithFit(imageItem, value));
  }

  function updateImageRadius(value: string) {
    beginEdit();
    previewItem(moodBoardImageWithRadius(imageItem, value));
  }

  function commitImageRadius(value: string) {
    commitEdit(moodBoardImageWithRadius(imageItem, value));
  }

  function toggleImageShadow() {
    commitItemEdit(cloneItem(imageItem), moodBoardImageWithToggledShadow(imageItem));
  }

  function updateImageAdjustment(field: keyof typeof imageAdjustments, value: string) {
    beginEdit();
    previewItem(moodBoardImageWithAdjustment(imageItem, imageAdjustments, field, value));
  }

  function commitImageAdjustment(field: keyof typeof imageAdjustments, value: string) {
    commitEdit(moodBoardImageWithAdjustment(imageItem, imageAdjustments, field, value));
  }

  function resetImageAdjustments() {
    commitItemEdit(cloneItem(imageItem), moodBoardImageWithDefaultAdjustments(imageItem));
  }

  function updateImageFraming(field: keyof typeof imageFraming, value: string) {
    beginEdit();
    previewItem(moodBoardImageWithFraming(imageItem, imageFraming, field, value));
  }

  function commitImageFraming(field: keyof typeof imageFraming, value: string) {
    commitEdit(moodBoardImageWithFraming(imageItem, imageFraming, field, value));
  }

  function resetImageFraming() {
    commitItemEdit(cloneItem(imageItem), moodBoardImageWithDefaultFraming(imageItem));
  }

  function editImagePath() {
    const nextPath = window.prompt("Path imagine", imageItem.path);
    if (nextPath === null) return;
    commitItemEdit(cloneItem(imageItem), moodBoardImageWithPath(imageItem, nextPath));
  }

  function applyImagePath() {
    if (!applyImageToSelectedElement) return;
    void applyImageToSelectedElement(imageItem.path);
  }

  function extractPalette() {
    if (!extractPaletteFromImage) return;
    void extractPaletteFromImage(imageItem.id, imageItem.path);
  }

  function applyMask() {
    applyVectorMask(imageItem.id);
  }

  function clearMask() {
    clearVectorMask(imageItem.id);
  }

  function exportImage() {
    void exportImageWebp(imageItem.id);
  }
</script>

<span class="separator"></span>
<button type="button" class:active={imageItem.fit === "cover"} title="Cover" onclick={() => updateImageFit("cover")}>Cover</button>
<button type="button" class:active={imageItem.fit === "contain"} title="Contain" onclick={() => updateImageFit("contain")}>Contain</button>
<label class="radius-control" title="Radius imagine">
  R
  <input
    type="number"
    min="0"
    max="48"
    step="1"
    value={imageItem.radius}
    aria-label="Radius imagine"
    onfocus={beginEdit}
    oninput={(event) => updateImageRadius(event.currentTarget.value)}
    onblur={(event) => commitImageRadius(event.currentTarget.value)}
  />
</label>
<button type="button" class:active={imageItem.shadow} title="Shadow" onclick={toggleImageShadow}>
  <IconShadow size={15} stroke={2} />
</button>
<button
  type="button"
  class:active={imageAdjustmentsOpen || !isDefaultImageAdjustments(imageAdjustments) || !isDefaultImageFraming(imageFraming)}
  title="Ajustări și încadrare imagine"
  aria-expanded={imageAdjustmentsOpen}
  onclick={() => imageAdjustmentsOpen = !imageAdjustmentsOpen}
>
  <IconAdjustmentsHorizontal size={15} stroke={2} />
</button>
{#if extractPaletteFromImage}
  <button type="button" title="Extrage paletă" disabled={heavyAssetBusy} onclick={extractPalette}>
    <IconPalette size={15} stroke={2} />
  </button>
{/if}
{#if applyImageToSelectedElement}
  <button type="button" title="Aplică pe imaginea selectată" onclick={applyImagePath}>
    <IconPhotoCheck size={15} stroke={2} />
  </button>
{/if}
<button type="button" title="Editează path" onclick={editImagePath}>
  <IconPhotoEdit size={15} stroke={2} />
</button>
<button type="button" title="Exportă WebP lossless / 1920px" disabled={heavyAssetBusy} onclick={exportImage}>
  <IconDownload size={15} stroke={2} />
</button>
{#if canApplyVectorMask(imageItem.id)}
  <button type="button" title="Aplică path-ul selectat ca mască" onclick={applyMask}>
    <IconCut size={15} stroke={2} />
  </button>
{/if}
{#if imageItem.mask}
  <button type="button" title="Elimină masca" onclick={clearMask}>
    <IconUnlink size={15} stroke={2.2} />
  </button>
{/if}

{#if imageAdjustmentsOpen}
  <MoodBoardImageAdjustmentsPopover
    {imageAdjustments}
    {imageFraming}
    beginImageEdit={beginEdit}
    {updateImageAdjustment}
    {commitImageAdjustment}
    {resetImageAdjustments}
    {updateImageFraming}
    {commitImageFraming}
    {resetImageFraming}
  />
{/if}
