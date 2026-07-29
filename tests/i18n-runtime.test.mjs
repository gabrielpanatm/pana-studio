import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { FluentCatalogRuntime } from "$lib/i18n/runtime-core";
import {
  BASE_LOCALE,
  localeCatalogs,
} from "$lib/i18n/generated/catalog";
import {
  APPLICATION_BOOT_PROJECTION_STORAGE_KEY,
  isApplicationBootProjection,
  parseApplicationBootProjection,
  storeApplicationBootProjection,
} from "$lib/system-preferences/boot-projection";

function collectSvelteFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return collectSvelteFiles(path);
    return entry.isFile() && entry.name.endsWith(".svelte") ? [path] : [];
  });
}

test("runtime-ul Fluent schimbă limba fără restart și folosește fallbackul en-US", () => {
  const runtime = new FluentCatalogRuntime(localeCatalogs, BASE_LOCALE);
  assert.equal(runtime.format("settings-language-title"), "Interface language");

  runtime.setLocale("ro");
  assert.equal(runtime.locale, "ro");
  assert.equal(runtime.format("settings-language-title"), "Limba interfeței");
  assert.equal(runtime.formatNumber(1234.5), "1.234,5");

  runtime.setLocale("de-DE");
  assert.equal(runtime.locale, "en-US");
  assert.equal(runtime.format("common-loading"), "Loading…");
  assert.equal(runtime.format("missing-message"), "[translation unavailable]");
});

test("direcția este metadată de catalog, inclusiv pentru un locale RTL", () => {
  const runtime = new FluentCatalogRuntime({
    "en-US": {
      manifest: {
        locale: "en-US",
        nativeName: "English",
        direction: "ltr",
        contributors: ["test"],
      },
      resources: { core: "hello = Hello\n" },
    },
    ar: {
      manifest: {
        locale: "ar",
        nativeName: "العربية",
        direction: "rtl",
        contributors: ["test"],
      },
      resources: { core: "hello = مرحبًا\n" },
    },
  }, "en-US");

  runtime.setLocale("ar");
  assert.equal(runtime.direction, "rtl");
  assert.equal(runtime.format("hello"), "مرحبًا");
});

test("boot-ul nativ nu afișează fereastra înaintea snapshotului Rust", () => {
  const config = readFileSync(
    new URL("../src-tauri/tauri.conf.json", import.meta.url),
    "utf8",
  );
  const page = readFileSync(
    new URL("../src/routes/+page.svelte", import.meta.url),
    "utf8",
  );
  const appHtml = readFileSync(
    new URL("../src/app.html", import.meta.url),
    "utf8",
  );
  const appState = readFileSync(
    new URL("../src/lib/state/app.svelte.ts", import.meta.url),
    "utf8",
  );
  const rustModel = readFileSync(
    new URL("../src-tauri/src/commands/config/model.rs", import.meta.url),
    "utf8",
  );
  const rustBoot = readFileSync(
    new URL("../src-tauri/src/lib.rs", import.meta.url),
    "utf8",
  );

  assert.equal(JSON.parse(config).app.windows[0].visible, false);
  assert.match(page, /app\.initFromStorage[\s\S]*finally\(revealApplication\)/);
  assert.match(page, /getCurrentWindow\(\)\.show\(\)/);
  assert.doesNotMatch(appHtml, /localStorage\.getItem\("pana-studio-ui-theme"\)/);
  assert.match(appHtml, /pana-studio-boot-projection-v1/);
  assert.match(appHtml, /__PANA_APPLY_BOOT_PROJECTION__/);
  assert.doesNotMatch(appHtml, /Pană Studio is loading|Preparing the visual editor/);
  assert.match(appState, /storeApplicationBootProjection\(window\.localStorage, snapshot\.boot\)/);
  assert.match(rustModel, /pub struct ApplicationBootProjection/);
  assert.match(rustBoot, /read_application_settings[\s\S]*settings\.boot[\s\S]*window\.eval/);
});

test("cache-ul de boot acceptă numai proiecția versionată a snapshotului Rust", () => {
  const projection = {
    schemaVersion: 1,
    authority: "rust_application_settings",
    settingsSchemaVersion: 3,
    settingsRevision: 12,
    systemGeneration: 8,
    locale: "ro",
    direction: "ltr",
    theme: "dark",
    accent: "#c2410c",
    contrast: "normal",
    reducedMotion: false,
    loadingLabel: "Pană Studio se încarcă",
    loadingSubtitle: "Se pregătește editorul vizual",
  };
  assert.equal(isApplicationBootProjection(projection), true);
  assert.deepEqual(
    parseApplicationBootProjection(JSON.stringify(projection)),
    projection,
  );
  assert.equal(
    isApplicationBootProjection({ ...projection, authority: "local_storage" }),
    false,
  );
  assert.equal(parseApplicationBootProjection("{invalid"), null);

  const stored = new Map();
  assert.equal(
    storeApplicationBootProjection(
      { setItem: (key, value) => stored.set(key, value) },
      projection,
    ),
    true,
  );
  assert.equal(
    stored.get(APPLICATION_BOOT_PROJECTION_STORAGE_KEY),
    JSON.stringify(projection),
  );
});

test("componentele Svelte legacy reacționează la schimbarea limbii", () => {
  const componentRoot = fileURLToPath(
    new URL("../src/lib/components/", import.meta.url),
  );
  const localizedLegacyComponents = collectSvelteFiles(componentRoot).filter(
    (path) => {
      const source = readFileSync(path, "utf8");
      return (
        source.includes('from "$lib/i18n/runtime.svelte"') &&
        /\bt\(/.test(source) &&
        !/\$(?:props|state|derived|effect)(?:\.by)?\s*\(/.test(source)
      );
    },
  );

  assert.ok(
    localizedLegacyComponents.length > 0,
    "contractul trebuie să acopere componentele localizate legacy",
  );

  for (const path of localizedLegacyComponents) {
    const source = readFileSync(path, "utf8");
    assert.match(
      source,
      /\blegacyTranslator\b/,
      `${path} nu reconstruiește traducătorul la schimbarea limbii`,
    );
    assert.match(
      source,
      /\blocaleRevision\b/,
      `${path} nu urmărește revizia limbii`,
    );
    assert.match(
      source,
      /\$:\s*t\s*=\s*legacyTranslator\(\$localeRevision\)/,
      `${path} nu leagă traducerile de revizia limbii`,
    );
  }

  const runtime = readFileSync(
    new URL("../src/lib/i18n/runtime.svelte.ts", import.meta.url),
    "utf8",
  );
  const topbar = readFileSync(
    new URL("../src/lib/components/Topbar.svelte", import.meta.url),
    "utf8",
  );
  assert.match(runtime, /localeRevision\.set\(this\.revision\)/);
  assert.match(topbar, /t\("workbench-command-center-search"\)/);
  assert.doesNotMatch(topbar, /Search commands, files, and symbols/);
});

test("generatorul i18n nu invalidează inutil HMR și publică atomic catalogul", () => {
  const generator = readFileSync(
    new URL("../scripts/generate-i18n-catalog.mjs", import.meta.url),
    "utf8",
  );

  assert.match(
    generator,
    /existsSync\(path\)\s*&&\s*readFileSync\(path,\s*"utf8"\)\s*===\s*source/,
  );
  assert.match(generator, /const temporaryPath = `\$\{path\}\.\$\{process\.pid\}\.tmp`/);
  assert.match(generator, /writeFileSync\(temporaryPath,\s*source\)/);
  assert.match(generator, /renameSync\(temporaryPath,\s*path\)/);
});
