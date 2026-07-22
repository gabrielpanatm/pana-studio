import assert from "node:assert/strict";
import { test } from "node:test";

import {
  createZolaImageIntent,
  decodeZolaImagePresentation,
  resolveZolaImageSource,
} from "$lib/html/zola-image";
import { htmlAttributeRecordForKernel } from "$lib/state/html-actions-controller";

function image(relativePath) {
  return {
    name: relativePath.split("/").at(-1),
    relativePath,
    absolutePath: `/project/${relativePath}`,
    kind: "IMAGE",
    role: "asset",
    previewPath: null,
  };
}

test("rezolvă strict o singură sursă locală suportată", () => {
  const resolved = resolveZolaImageSource("/images/hero.jpg", [
    image("static/images/hero.jpg"),
    image("static/images/logo.svg"),
  ]);
  assert.deepEqual(resolved.eligible && {
    sourceUrl: resolved.sourceUrl,
    sourcePath: resolved.sourcePath,
  }, {
    sourceUrl: "/images/hero.jpg",
    sourcePath: "static/images/hero.jpg",
  });

  assert.deepEqual(createZolaImageIntent({
    enabled: true,
    source: resolved,
    width: 1200,
    height: null,
    operation: "fit_width",
    format: "webp",
    quality: 82,
  }), {
    enabled: true,
    sourceUrl: "/images/hero.jpg",
    sourcePath: "static/images/hero.jpg",
    width: 1200,
    height: null,
    operation: "fit_width",
    format: "webp",
    quality: 82,
  });
});

test("refuză surse externe, dinamice, nesuportate și ambigue", () => {
  const assets = [
    image("static/images/hero.jpg"),
    image("themes/demo/static/images/hero.jpg"),
    image("static/images/logo.svg"),
  ];
  assert.equal(resolveZolaImageSource("https://example.test/hero.jpg", assets).eligible, false);
  assert.equal(resolveZolaImageSource("/{{ hero }}", assets).eligible, false);
  assert.equal(resolveZolaImageSource("/images/logo.svg", assets).eligible, false);
  const ambiguous = resolveZolaImageSource("/images/hero.jpg", assets);
  assert.equal(ambiguous.eligible, false);
  assert.match(ambiguous.reason, /ambiguu/);
});

test("decodează metadata preview base64url și refuză payload invalid", () => {
  const state = {
    sourceUrl: "/imagini/erou.jpg",
    sourcePath: "static/imagini/erou.jpg",
    width: 960,
    height: 540,
    operation: "fill",
    format: "avif",
    quality: 74,
  };
  const payload = Buffer.from(JSON.stringify(state), "utf8").toString("base64url");
  assert.deepEqual(decodeZolaImagePresentation(payload), state);
  assert.equal(decodeZolaImagePresentation("nu-este-base64-json"), null);
});

test("editările HTML generice nu suprascriu atributele administrate de Zola", () => {
  assert.deepEqual(htmlAttributeRecordForKernel(
    {
      src: "http://preview.test/processed_images/hero.webp",
      width: "960",
      height: "540",
      alt: "Descriere nouă",
      loading: "lazy",
    },
    {
      src: "http://preview.test/processed_images/hero.webp",
      width: "960",
      height: "540",
      alt: "Descriere veche",
      title: "Eliminat",
    },
    true,
  ), {
    alt: "Descriere nouă",
    loading: "lazy",
    title: null,
  });
});
