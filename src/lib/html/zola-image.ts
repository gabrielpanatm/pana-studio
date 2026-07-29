import { projectAssetPublicUrl } from "$lib/project/assets";
import { zolaRelativePath } from "$lib/project/files";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  ProjectFile,
  ProjectZolaImageIntent,
  ZolaImageFormat,
  ZolaImageOperation,
  ZolaImagePresentation,
} from "$lib/types";

const SUPPORTED_SOURCE_EXTENSIONS = new Set(["jpg", "jpeg", "png", "webp", "avif"]);

export type ZolaImageSourceResolution =
  | { eligible: true; sourceUrl: string; sourcePath: string; asset: ProjectFile }
  | { eligible: false; code: ZolaImageSourceFailureCode };

export type ZolaImageSourceFailureCode =
  | "invalid_source_url"
  | "asset_not_found"
  | "asset_ambiguous"
  | "source_unresolved";

export function zolaImageSourceFailureMessage(code: ZolaImageSourceFailureCode) {
  switch (code) {
    case "invalid_source_url":
      return t("zola-image-source-invalid");
    case "asset_not_found":
      return t("zola-image-source-not-found");
    case "asset_ambiguous":
      return t("zola-image-source-ambiguous");
    case "source_unresolved":
      return t("zola-image-source-unresolved");
  }
}

export function decodeZolaImagePresentation(payload: string | null): ZolaImagePresentation | null {
  if (!payload) return null;
  try {
    const standard = payload.replaceAll("-", "+").replaceAll("_", "/");
    const padded = standard.padEnd(Math.ceil(standard.length / 4) * 4, "=");
    const binary = globalThis.atob(padded);
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    const candidate = JSON.parse(new TextDecoder().decode(bytes)) as Partial<ZolaImagePresentation>;
    if (
      typeof candidate.sourceUrl !== "string"
      || typeof candidate.sourcePath !== "string"
      || !Number.isInteger(candidate.width)
      || (candidate.height !== null && !Number.isInteger(candidate.height))
      || !isZolaImageOperation(candidate.operation)
      || !isZolaImageFormat(candidate.format)
      || !Number.isInteger(candidate.quality)
    ) return null;
    return candidate as ZolaImagePresentation;
  } catch {
    return null;
  }
}

export function zolaImagePresentationFromElement(element: Element): ZolaImagePresentation | null {
  return decodeZolaImagePresentation(element.getAttribute("data-pana-zola-image"));
}

export function resolveZolaImageSource(
  rawSourceUrl: string,
  assets: readonly ProjectFile[],
): ZolaImageSourceResolution {
  const sourceUrl = rawSourceUrl.trim();
  if (
    !sourceUrl.startsWith("/")
    || sourceUrl.startsWith("//")
    || /[?#{}\\\r\n\0]/.test(sourceUrl)
    || sourceUrl.split("/").includes("..")
  ) {
    return {
      eligible: false,
      code: "invalid_source_url",
    };
  }

  const matches = assets.filter((asset) => (
    asset.kind === "IMAGE"
    && supportedAsset(asset)
    && projectAssetPublicUrl(asset) === sourceUrl
  ));
  if (matches.length === 0) {
    return {
      eligible: false,
      code: "asset_not_found",
    };
  }
  if (matches.length > 1) {
    return {
      eligible: false,
      code: "asset_ambiguous",
    };
  }
  const asset = matches[0];
  return {
    eligible: true,
    sourceUrl,
    sourcePath: zolaRelativePath(asset.relativePath).replaceAll("\\", "/").replace(/^\/+/, ""),
    asset,
  };
}

export function createZolaImageIntent(input: {
  enabled: boolean;
  source?: ZolaImageSourceResolution;
  width?: number;
  height?: number | null;
  operation?: ZolaImageOperation;
  format?: ZolaImageFormat;
  quality?: number;
}): ProjectZolaImageIntent {
  if (!input.enabled) return { enabled: false };
  if (!input.source?.eligible) {
    throw new Error(zolaImageSourceFailureMessage(input.source?.code ?? "source_unresolved"));
  }
  return {
    enabled: true,
    sourceUrl: input.source.sourceUrl,
    sourcePath: input.source.sourcePath,
    width: input.width,
    height: input.height ?? null,
    operation: input.operation,
    format: input.format,
    quality: input.quality,
  };
}

function supportedAsset(asset: ProjectFile) {
  const extension = asset.relativePath.split(".").at(-1)?.toLocaleLowerCase("en") ?? "";
  return SUPPORTED_SOURCE_EXTENSIONS.has(extension);
}

function isZolaImageOperation(value: unknown): value is ZolaImageOperation {
  return value === "fit_width" || value === "fit" || value === "fill";
}

function isZolaImageFormat(value: unknown): value is ZolaImageFormat {
  return value === "auto" || value === "webp" || value === "avif" || value === "jpg" || value === "png";
}
