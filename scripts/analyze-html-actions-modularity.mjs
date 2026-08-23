import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ACTIONS_ROOT = "src/lib/editor/html-actions/";
const SERVICE = "src/lib/editor/html-editing-service.ts";
const NAVIGATION_SERVICE = "src/lib/editor/navigation-service.ts";
const EXECUTION_OWNER = `${ACTIONS_ROOT}execution.ts`;
const EXPECTED_MODULES = new Set([
  `${ACTIONS_ROOT}attribute-values.ts`,
  `${ACTIONS_ROOT}attributes.ts`,
  `${ACTIONS_ROOT}execution.ts`,
  `${ACTIONS_ROOT}host.ts`,
  `${ACTIONS_ROOT}identity.ts`,
  `${ACTIONS_ROOT}insertion.ts`,
  `${ACTIONS_ROOT}media.ts`,
  `${ACTIONS_ROOT}structure.ts`,
  `${ACTIONS_ROOT}target.ts`,
  `${ACTIONS_ROOT}text.ts`,
]);
const FORBIDDEN_FILES = new Set([
  "src/lib/state/html-actions-controller.ts",
  "src/lib/state/html-mutation-controller.ts",
  "src/lib/session/kernel-planned-draft.ts",
  `${ACTIONS_ROOT}index.ts`,
]);
const MUTATION_MODULES = new Set([
  "attributes",
  "identity",
  "insertion",
  "media",
  "structure",
  "text",
]);
const CANONICAL_BOUNDARIES = new Set([
  "executePreviewHtmlAttributesIntent",
  "executePreviewHtmlDeleteIntent",
  "executePreviewHtmlDuplicateIntent",
  "executePreviewHtmlInsertDropIntent",
  "executePreviewHtmlTextIntent",
  "executePreviewSelectionBatchIntent",
]);

function filesUnder(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) return filesUnder(target);
    return /\.(?:ts|svelte|js|mjs)$/.test(entry.name) ? [target] : [];
  });
}

function lineForOffset(source, offset) {
  return source.slice(0, offset).split("\n").length;
}

export function analyzeHtmlActionsSources({
  sources,
  maxModuleLines = 600,
  maxFacadeLines = 300,
  requireCompleteLayout = false,
}) {
  const violations = [];
  const boundaries = new Set();

  if (requireCompleteLayout) {
    for (const file of EXPECTED_MODULES) {
      if (!sources.has(file)) violations.push({ code: "missing-html-actions-module", file, line: 1 });
    }
  }

  for (const [rawFile, source] of sources) {
    const file = rawFile.split(path.sep).join("/");
    if (FORBIDDEN_FILES.has(file)) {
      violations.push({ code: "legacy-html-actions-path", file, line: 1 });
    }
    for (const match of source.matchAll(/\$lib\/(?:state\/html-actions-controller|state\/html-mutation-controller|session\/kernel-planned-draft)/g)) {
      violations.push({
        code: "legacy-html-actions-import",
        file,
        line: lineForOffset(source, match.index),
      });
    }

    const lines = source.split("\n").length;
    if (file.startsWith(ACTIONS_ROOT) && lines > maxModuleLines) {
      violations.push({
        code: "oversized-html-actions-module",
        file,
        line: 1,
        detail: `${lines}>${maxModuleLines}`,
      });
    }
    if (file === SERVICE && lines > maxFacadeLines) {
      violations.push({
        code: "oversized-html-editing-facade",
        file,
        line: 1,
        detail: `${lines}>${maxFacadeLines}`,
      });
    }
    if ((file === SERVICE || file === NAVIGATION_SERVICE) && /\bcontrollerHost\s*\(/.test(source)) {
      violations.push({ code: "html-actions-host-leak", file, line: 1 });
    }

    const runtimeFeatureImport = /import\s+(?!type\b)([^;]+)\s+from\s+["']\$lib\/editor\/html-actions\/([^"']+)["']/g;
    for (const match of source.matchAll(runtimeFeatureImport)) {
      if (
        MUTATION_MODULES.has(match[2])
        && file !== SERVICE
        && !file.startsWith(ACTIONS_ROOT)
      ) {
        violations.push({
          code: "html-actions-facade-bypass",
          file,
          line: lineForOffset(source, match.index),
          detail: match[2],
        });
      }
    }
    if (
      file !== SERVICE
      && !file.startsWith(ACTIONS_ROOT)
      && /from\s+["']\$lib\/editor\/html-actions\/host["']/.test(source)
    ) {
      violations.push({ code: "html-actions-host-owner-bypass", file, line: 1 });
    }

    if (!file.startsWith(ACTIONS_ROOT)) continue;
    for (const match of source.matchAll(/host\.structural\.(?:run|projectCommitted|projectCommittedBatch|settleMutation|leaseMatches)\b/g)) {
      if (file !== EXECUTION_OWNER) {
        violations.push({
          code: "html-actions-execution-owner-bypass",
          file,
          line: lineForOffset(source, match.index),
        });
      }
    }
    for (const match of source.matchAll(/\b(executePreview[A-Za-z0-9]+Intent)\s*\(/g)) {
      boundaries.add(match[1]);
      if (!CANONICAL_BOUNDARIES.has(match[1])) {
        violations.push({
          code: "noncanonical-html-actions-boundary",
          file,
          line: lineForOffset(source, match.index),
          detail: match[1],
        });
      }
    }
    for (const match of source.matchAll(/(?:@tauri-apps\/api|\binvoke\s*\()/g)) {
      violations.push({
        code: "direct-html-actions-invoke",
        file,
        line: lineForOffset(source, match.index),
      });
    }
    for (const match of source.matchAll(/["'`][^"'`\n]*[ăâîșțĂÂÎȘȚ][^"'`\n]*["'`]/g)) {
      violations.push({
        code: "hardcoded-html-actions-message",
        file,
        line: lineForOffset(source, match.index),
      });
    }
  }

  if (requireCompleteLayout) {
    for (const boundary of CANONICAL_BOUNDARIES) {
      if (!boundaries.has(boundary)) {
        violations.push({
          code: "missing-canonical-html-actions-boundary",
          file: ACTIONS_ROOT,
          line: 1,
          detail: boundary,
        });
      }
    }
  }

  return {
    violations: violations.sort((left, right) => (
      left.file.localeCompare(right.file)
      || left.line - right.line
      || left.code.localeCompare(right.code)
    )),
    modules: [...sources.keys()].filter((file) => file.startsWith(ACTIONS_ROOT)).length,
    boundaries: [...boundaries].sort(),
  };
}

export function analyzeHtmlActionsModularity(projectRoot = process.cwd()) {
  const sourceFiles = filesUnder(path.join(projectRoot, "src"));
  return analyzeHtmlActionsSources({
    sources: new Map(sourceFiles.map((file) => [
      path.relative(projectRoot, file).split(path.sep).join("/"),
      readFileSync(file, "utf8"),
    ])),
    requireCompleteLayout: true,
  });
}

const isMain = process.argv[1]
  && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));
if (isMain) {
  const report = analyzeHtmlActionsModularity();
  console.log(JSON.stringify(report, null, 2));
  if (report.violations.length > 0) process.exitCode = 1;
}
