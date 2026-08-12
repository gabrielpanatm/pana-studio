import {
  existsSync,
  renameSync,
  readFileSync,
  readdirSync,
  writeFileSync,
  mkdirSync,
  statSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "@fluent/syntax";
import { FluentBundle, FluentResource } from "@fluent/bundle";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const localesRoot = join(projectRoot, "locales");
const generatedRoot = join(projectRoot, "src/lib/i18n/generated");
const outputPath = join(generatedRoot, "catalog.ts");
const checkOnly = process.argv.includes("--check");
const baseLocale = "en-US";

function fail(message) {
  throw new Error(`[i18n] ${message}`);
}

function writeGeneratedCatalog(path, source) {
  if (existsSync(path) && readFileSync(path, "utf8") === source) {
    return false;
  }

  mkdirSync(dirname(path), { recursive: true });
  const temporaryPath = `${path}.${process.pid}.tmp`;
  writeFileSync(temporaryPath, source);
  renameSync(temporaryPath, path);
  return true;
}

function sortedDirectoryNames(path) {
  return readdirSync(path, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((left, right) => left.localeCompare(right));
}

function readManifest(locale) {
  const path = join(localesRoot, locale, "manifest.json");
  const manifest = JSON.parse(readFileSync(path, "utf8"));
  if (manifest.locale !== locale) fail(`${path}: locale must equal directory name ${locale}`);
  if (typeof manifest.nativeName !== "string" || !manifest.nativeName.trim()) {
    fail(`${path}: nativeName is required`);
  }
  if (manifest.direction !== "ltr" && manifest.direction !== "rtl") {
    fail(`${path}: direction must be ltr or rtl`);
  }
  if (!Array.isArray(manifest.contributors) || manifest.contributors.length === 0) {
    fail(`${path}: at least one contributor is required`);
  }
  if (manifest.contributors.some((contributor) =>
    typeof contributor !== "string" || !contributor.trim()
  )) {
    fail(`${path}: every contributor must be a non-empty string`);
  }
  let canonicalLocale;
  try {
    [canonicalLocale] = Intl.getCanonicalLocales(locale);
  } catch {
    fail(`${path}: ${locale} is not a valid BCP-47 locale`);
  }
  if (canonicalLocale !== locale) {
    fail(`${path}: locale directory must use canonical BCP-47 form ${canonicalLocale}`);
  }
  return manifest;
}

function collectReferences(
  node,
  result = { variables: new Set(), messages: new Set(), terms: new Set() },
) {
  if (!node || typeof node !== "object") return result;
  if (node.type === "VariableReference") result.variables.add(node.id.name);
  if (node.type === "MessageReference") result.messages.add(node.id.name);
  if (node.type === "TermReference") result.terms.add(`-${node.id.name}`);
  for (const [key, value] of Object.entries(node)) {
    if (key === "span" || key === "annotations") continue;
    if (Array.isArray(value)) {
      for (const item of value) collectReferences(item, result);
    } else {
      collectReferences(value, result);
    }
  }
  return result;
}

function parseDomain(locale, domain, source) {
  const resource = parse(source, { withSpans: true });
  const messages = new Map();
  for (const entry of resource.body) {
    if (entry.type === "Junk") {
      const diagnostic = entry.annotations
        .map((annotation) => `${annotation.code}: ${annotation.message}`)
        .join("; ");
      fail(`${locale}/${domain}.ftl contains invalid Fluent syntax: ${diagnostic}`);
    }
    if (entry.type !== "Message" && entry.type !== "Term") continue;
    const id = entry.type === "Term" ? `-${entry.id.name}` : entry.id.name;
    if (messages.has(id)) fail(`${locale}/${domain}.ftl contains duplicate id ${id}`);
    const references = collectReferences(entry);
    messages.set(id, {
      variables: [...references.variables].sort(),
      messages: [...references.messages].sort(),
      terms: [...references.terms].sort(),
      attributes: (entry.attributes ?? [])
        .map((attribute) => attribute.id.name)
        .sort(),
    });
  }
  return messages;
}

function sameList(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

const discoveredLocales = sortedDirectoryNames(localesRoot);
if (!discoveredLocales.includes(baseLocale)) fail(`base locale ${baseLocale} is missing`);
const locales = [
  baseLocale,
  ...discoveredLocales.filter((locale) => locale !== baseLocale),
];

const baseDomains = readdirSync(join(localesRoot, baseLocale))
  .filter((name) => name.endsWith(".ftl"))
  .map((name) => name.slice(0, -4))
  .sort((left, right) => left.localeCompare(right));
if (baseDomains.length === 0) fail("the base locale has no Fluent domains");

const catalog = {};
const baseMessages = new Map();
for (const locale of locales) {
  const manifest = readManifest(locale);
  const localeDomains = readdirSync(join(localesRoot, locale))
    .filter((name) => name.endsWith(".ftl"))
    .map((name) => name.slice(0, -4))
    .sort((left, right) => left.localeCompare(right));
  if (!sameList(localeDomains, baseDomains)) {
    fail(`${locale} domains must exactly match ${baseLocale}`);
  }
  const resources = {};
  const seenIds = new Set();
  for (const domain of baseDomains) {
    const path = join(localesRoot, locale, `${domain}.ftl`);
    if (!existsSync(path)) fail(`${locale} is missing domain ${domain}.ftl`);
    const source = readFileSync(path, "utf8");
    resources[domain] = source;
    const messages = parseDomain(locale, domain, source);
    for (const [id, references] of messages) {
      if (seenIds.has(id)) fail(`${locale} contains duplicate id ${id} across domains`);
      seenIds.add(id);
      const key = `${domain}:${id}`;
      if (locale === baseLocale) {
        baseMessages.set(key, references);
        continue;
      }
      const base = baseMessages.get(key);
      if (!base) fail(`${locale}/${domain}.ftl contains unknown id ${id}`);
      if (!sameList(base.variables, references.variables)) {
        fail(`${locale}/${domain}.ftl id ${id} has incompatible variables`);
      }
      if (!sameList(base.messages, references.messages)) {
        fail(`${locale}/${domain}.ftl id ${id} has incompatible message references`);
      }
      if (!sameList(base.terms, references.terms)) {
        fail(`${locale}/${domain}.ftl id ${id} has incompatible term references`);
      }
      if (!sameList(base.attributes, references.attributes)) {
        fail(`${locale}/${domain}.ftl id ${id} has incompatible attributes`);
      }
    }
  }
  if (locale !== baseLocale) {
    for (const key of baseMessages.keys()) {
      const [domain, id] = key.split(":");
      const messages = parseDomain(locale, domain, resources[domain]);
      if (!messages.has(id)) fail(`${locale}/${domain}.ftl is missing id ${id}`);
    }
  }
  catalog[locale] = { manifest, resources };
}

const messageIds = [...baseMessages.keys()]
  .map((entry) => entry.slice(entry.indexOf(":") + 1))
  .filter((id) => !id.startsWith("-"))
  .sort((left, right) => left.localeCompare(right));

validateEveryMessageFormats(catalog, messageIds, baseMessages);
validateMessageUsage(messageIds, baseMessages);

const localeModules = locales.map((locale, index) => ({
  locale,
  binding: `localeCatalog${index}`,
  path: join(generatedRoot, `catalog.${locale}.ts`),
  source: `/* This file is generated by scripts/generate-i18n-catalog.mjs. */\n`
    + `export const localeCatalog = ${JSON.stringify(catalog[locale], null, 2)} as const;\n`,
}));

const generated = `/* This file is generated by scripts/generate-i18n-catalog.mjs. */\n`
  + localeModules
    .map(({ locale, binding }) =>
      `import { localeCatalog as ${binding} } from ${JSON.stringify(`./catalog.${locale}`)};\n`
    )
    .join("")
  + `export const BASE_LOCALE = ${JSON.stringify(baseLocale)} as const;\n`
  + `export const localeCatalogs = {\n`
  + localeModules
    .map(({ locale, binding }) => `  ${JSON.stringify(locale)}: ${binding},\n`)
    .join("")
  + `} as const;\n`
  + `export const availableLocales = Object.keys(localeCatalogs) as AvailableLocale[];\n`
  + `export const messageIds = ${JSON.stringify(messageIds, null, 2)} as const;\n`
  + `export type AvailableLocale = keyof typeof localeCatalogs;\n`
  + `export type MessageId = typeof messageIds[number];\n`;

if (checkOnly) {
  if (!existsSync(outputPath) || readFileSync(outputPath, "utf8") !== generated) {
    fail("generated catalog is stale; run npm run i18n:generate");
  }
  for (const localeModule of localeModules) {
    if (
      !existsSync(localeModule.path)
      || readFileSync(localeModule.path, "utf8") !== localeModule.source
    ) {
      fail(`generated catalog for ${localeModule.locale} is stale; run npm run i18n:generate`);
    }
  }
} else {
  writeGeneratedCatalog(outputPath, generated);
  for (const localeModule of localeModules) {
    writeGeneratedCatalog(localeModule.path, localeModule.source);
  }
}

console.log(
  `[i18n] ${locales.length} locales, ${baseDomains.length} domains, ${messageIds.length} messages`,
);

function validateEveryMessageFormats(catalogs, ids, messages) {
  const referencesById = new Map(
    [...messages].map(([key, references]) => [
      key.slice(key.indexOf(":") + 1),
      references,
    ]),
  );
  for (const [locale, catalog_] of Object.entries(catalogs)) {
    const bundle = new FluentBundle(locale, { useIsolating: true });
    for (const [domain, source] of Object.entries(catalog_.resources)) {
      const errors = bundle.addResource(new FluentResource(source));
      if (errors.length > 0) {
        fail(`${locale}/${domain}.ftl cannot be added to a Fluent bundle: ${errors.join("; ")}`);
      }
    }
    for (const id of ids) {
      const message = bundle.getMessage(id);
      if (!message?.value) fail(`${locale} message ${id} has no value`);
      const variables = referencesById.get(id)?.variables ?? [];
      const arguments_ = Object.fromEntries(variables.map((name) => [name, 2]));
      const errors = [];
      const formatted = bundle.formatPattern(message.value, arguments_, errors);
      if (errors.length > 0) {
        fail(`${locale} message ${id} cannot be formatted: ${errors.join("; ")}`);
      }
      if (!formatted.trim()) fail(`${locale} message ${id} formats to an empty string`);
    }
  }
}

function validateMessageUsage(ids, messages) {
  const roots = [join(projectRoot, "src"), join(projectRoot, "src-tauri", "src")];
  const source = roots
    .flatMap((root) => recursiveFiles(root))
    .filter((path) => !path.includes(`${join("i18n", "generated")}`))
    .map((path) => readFileSync(path, "utf8"))
    .join("\n");
  const allIds = [...messages.keys()]
    .map((key) => key.slice(key.indexOf(":") + 1));
  const used = new Set(allIds.filter((id) => source.includes(id)));
  const referencesById = new Map(
    [...messages].map(([key, references]) => [
      key.slice(key.indexOf(":") + 1),
      [...references.messages, ...references.terms],
    ]),
  );
  const pending = [...used];
  while (pending.length > 0) {
    const id = pending.pop();
    for (const reference of referencesById.get(id) ?? []) {
      if (used.has(reference)) continue;
      used.add(reference);
      pending.push(reference);
    }
  }
  const unused = allIds.filter((id) => !used.has(id));
  if (unused.length > 0) {
    fail(`unused Fluent messages: ${unused.join(", ")}`);
  }
}

function recursiveFiles(root) {
  return readdirSync(root).flatMap((name) => {
    const path = join(root, name);
    if (statSync(path).isDirectory()) return recursiveFiles(path);
    return /\.(?:rs|ts|svelte|html)$/.test(path) ? [path] : [];
  });
}
