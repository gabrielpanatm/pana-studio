<script lang="ts">
  import { onMount } from "svelte";
  import { imageFilterValue, imageOpacityValue } from "$lib/mood-board/image-adjustments";
  import {
    imageObjectPositionValue,
    imageTransformOriginValue,
    imageTransformValue,
  } from "$lib/mood-board/image-framing";
  import {
    isMoodBoardStaleSessionError,
    requireCurrentMoodBoardIdentity,
    resolveMoodBoardImageSrc,
    type MoodBoardIdentityGuard,
    type MoodBoardRequestIdentity,
  } from "$lib/mood-board/io";
  import { moodBoardImageMaskPath } from "$lib/mood-board/item-view";
  import type { MoodBoardImageItem } from "$lib/mood-board/model";

  export let item: MoodBoardImageItem;
  export let sessionIdentity: MoodBoardRequestIdentity;
  export let isSessionCurrent: MoodBoardIdentityGuard;

  let imageEl: HTMLElement | null = null;
  let imageNearViewport = false;
  let imageSrc = "";
  let imageError = "";
  let imageRequestId = 0;
  let resolvedImagePath = "";
  let imageLoadingPath = "";
  let disposed = false;

  $: {
    const nextPath = item.path.trim().replaceAll("\\", "/");
    if (nextPath !== resolvedImagePath) {
      resolvedImagePath = nextPath;
      imageSrc = "";
      imageError = "";
      imageLoadingPath = "";
      if (imageNearViewport) resolveImage(nextPath);
    }
  }

  $: if (
    imageNearViewport
    && resolvedImagePath
    && !imageSrc
    && !imageError
    && imageLoadingPath !== resolvedImagePath
  ) {
    resolveImage(resolvedImagePath);
  }

  async function resolveImage(path: string) {
    const identity = { ...sessionIdentity };
    const requestId = ++imageRequestId;
    imageSrc = "";
    imageError = "";
    imageLoadingPath = path;
    if (!path.trim()) {
      imageLoadingPath = "";
      imageError = "Path imagine lipsă.";
      return;
    }

    try {
      requireCurrentMoodBoardIdentity(identity, isSessionCurrent);
      const src = await resolveMoodBoardImageSrc(identity, isSessionCurrent, path);
      requireCurrentMoodBoardIdentity(identity, isSessionCurrent);
      if (disposed || requestId !== imageRequestId) return;
      imageSrc = src;
      imageLoadingPath = "";
    } catch (error) {
      if (
        disposed
        || requestId !== imageRequestId
        || isMoodBoardStaleSessionError(error)
        || !isSessionCurrent(identity)
      ) return;
      imageLoadingPath = "";
      imageError = error instanceof Error ? error.message : String(error);
    }
  }

  onMount(() => {
    if (!imageEl) {
      return () => {
        disposed = true;
        imageRequestId += 1;
      };
    }
    if (!("IntersectionObserver" in window)) {
      imageNearViewport = true;
      return () => {
        disposed = true;
        imageRequestId += 1;
      };
    }

    const observer = new IntersectionObserver((entries) => {
      imageNearViewport = entries.some((entry) => entry.isIntersecting);
    }, {
      root: null,
      rootMargin: "220px",
      threshold: 0,
    });
    observer.observe(imageEl);
    return () => {
      disposed = true;
      imageRequestId += 1;
      observer.disconnect();
    };
  });
</script>

<div
  bind:this={imageEl}
  class:with-shadow={item.shadow}
  class:masked={Boolean(item.mask)}
  class="image-shell"
  style={`--image-radius:${item.radius}px;`}
>
  <div class:masked={Boolean(item.mask)} class="image-preview">
    {#if imageSrc}
      {#if item.mask}
        <svg
          class="masked-image"
          viewBox={`0 0 ${item.mask.viewBoxWidth} ${item.mask.viewBoxHeight}`}
          preserveAspectRatio="none"
          aria-label={item.title || item.path}
        >
          <defs>
            <clipPath id={`mask_${item.id}`}>
              <path d={moodBoardImageMaskPath(item.mask)} />
            </clipPath>
          </defs>
          <image
            href={imageSrc}
            width={item.mask.viewBoxWidth}
            height={item.mask.viewBoxHeight}
            preserveAspectRatio={item.fit === "contain" ? "xMidYMid meet" : "xMidYMid slice"}
            clip-path={`url(#mask_${item.id})`}
            opacity={imageOpacityValue(item.adjustments)}
            style={`filter:${imageFilterValue(item.adjustments)};transform:${imageTransformValue(item.framing)};transform-origin:${imageTransformOriginValue(item.framing)};transform-box:fill-box;`}
          />
        </svg>
      {:else}
        <img
          class:contain={item.fit === "contain"}
          src={imageSrc}
          alt={item.title || item.path}
          draggable="false"
          style={`filter:${imageFilterValue(item.adjustments)};opacity:${imageOpacityValue(item.adjustments)};object-position:${imageObjectPositionValue(item.framing)};transform:${imageTransformValue(item.framing)};transform-origin:${imageTransformOriginValue(item.framing)};`}
        />
      {/if}
    {:else}
      <span>{imageError || "Se încarcă imaginea..."}</span>
    {/if}
  </div>
</div>

<style>
  .image-shell {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    border-radius: min(var(--image-radius), 50%);
  }

  .image-shell.with-shadow {
    box-shadow: 0 16px 34px rgba(0, 0, 0, 0.22);
  }

  .image-shell.masked.with-shadow {
    box-shadow: none;
    filter: drop-shadow(0 16px 24px rgba(0, 0, 0, 0.18));
  }

  .image-preview {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    border: 0;
    border-radius: inherit;
    color: var(--text-muted);
    background: transparent;
    overflow: hidden;
    clip-path: inset(0 round min(var(--image-radius), 50%));
  }

  .image-preview.masked {
    border-radius: 0;
    clip-path: none;
  }

  .image-preview img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    pointer-events: none;
  }

  .masked-image {
    display: block;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }

  .image-preview img.contain {
    object-fit: contain;
  }

  .image-preview span {
    padding: 10px;
    border: 1px solid color-mix(in srgb, var(--border-3) 54%, transparent);
    border-radius: inherit;
    text-align: center;
    background: var(--surface-3);
    font-size: 11px;
    line-height: 1.35;
  }
</style>
