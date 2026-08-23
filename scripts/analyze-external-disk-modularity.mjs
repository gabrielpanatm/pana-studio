import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = "src/lib/session/external-disk/";
const FACADE = "src/lib/session/external-disk-state.svelte.ts";
const MONITOR = `${ROOT}monitor.ts`;
const RECONCILE = `${ROOT}reconcile.ts`;
const EXPECTED_MODULES = new Set([
  `${ROOT}contracts.ts`,
  MONITOR,
  RECONCILE,
  `${ROOT}state.ts`,
]);
const FORBIDDEN_FILES = new Set([
  "src/lib/state/external-disk-controller.ts",
  `${ROOT}index.ts`,
]);
const BOUNDARY_OWNERS = new Map([
  ["readCurrentProjectDiskManifest", RECONCILE],
  ["reconcileCleanExternalProjectFiles", RECONCILE],
  ["startProjectDiskWatch", MONITOR],
  ["stopProjectDiskWatch", MONITOR],
  ["subscribeProjectDiskChanges", MONITOR],
]);
const CANONICAL_BOUNDARIES = new Set([
  "readCurrentProjectDiskManifest",
  "reconcileCleanExternalProjectFiles",
  "startProjectDiskWatch",
  "stopProjectDiskWatch",
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

export function analyzeExternalDiskSources({
  sources,
  maxModuleLines = 600,
  maxFacadeLines = 250,
  requireCompleteLayout = false,
}) {
  const violations = [];
  const boundaries = new Set();

  if (requireCompleteLayout) {
    for (const file of EXPECTED_MODULES) {
      if (!sources.has(file)) {
        violations.push({ code: "missing-external-disk-module", file, line: 1 });
      }
    }
  }

  for (const [rawFile, source] of sources) {
    const file = rawFile.split(path.sep).join("/");
    if (FORBIDDEN_FILES.has(file)) {
      violations.push({ code: "legacy-external-disk-path", file, line: 1 });
    }
    for (const match of source.matchAll(/\$lib\/state\/external-disk-controller/g)) {
      violations.push({
        code: "legacy-external-disk-import",
        file,
        line: lineForOffset(source, match.index),
      });
    }

    const lines = source.split("\n").length;
    if (file.startsWith(ROOT) && lines > maxModuleLines) {
      violations.push({
        code: "oversized-external-disk-module",
        file,
        line: 1,
        detail: `${lines}>${maxModuleLines}`,
      });
    }
    if (file === FACADE && lines > maxFacadeLines) {
      violations.push({
        code: "oversized-external-disk-facade",
        file,
        line: 1,
        detail: `${lines}>${maxFacadeLines}`,
      });
    }

    const featureImport = /from\s+["']\$lib\/session\/external-disk\/(monitor|reconcile|state)["']/g;
    for (const match of source.matchAll(featureImport)) {
      if (file !== FACADE && !file.startsWith(ROOT)) {
        violations.push({
          code: "external-disk-facade-bypass",
          file,
          line: lineForOffset(source, match.index),
          detail: match[1],
        });
      }
    }
    if (
      file !== FACADE
      && !file.startsWith(ROOT)
      && /import[\s\S]*?ExternalDisk(?:Context|Runtime|Environment)[\s\S]*?from\s+["']\$lib\/session\/external-disk\/contracts["']/.test(source)
    ) {
      violations.push({ code: "external-disk-runtime-owner-bypass", file, line: 1 });
    }

    if (!file.startsWith(ROOT) && file !== FACADE) continue;
    if (/(?:^|\n)\s*(?:export\s+)?let\s+\w*[Rr]econcileGeneration\b/.test(source)) {
      violations.push({ code: "global-external-reconcile-generation", file, line: 1 });
    }
    if (/\.session\.epoch\s*(?:\+=|=)/.test(source)) {
      violations.push({ code: "external-disk-project-epoch-mutation", file, line: 1 });
    }
    for (const [symbol, owner] of BOUNDARY_OWNERS) {
      for (const match of source.matchAll(new RegExp(`\\b${symbol}\\b`, "g"))) {
        if (CANONICAL_BOUNDARIES.has(symbol)) boundaries.add(symbol);
        if (file !== owner) {
          violations.push({
            code: "external-disk-boundary-owner-bypass",
            file,
            line: lineForOffset(source, match.index),
            detail: symbol,
          });
        }
      }
    }
    for (const match of source.matchAll(/(?:@tauri-apps\/api|\binvoke\s*\()/g)) {
      violations.push({
        code: "direct-external-disk-invoke",
        file,
        line: lineForOffset(source, match.index),
      });
    }
    if (/\bsetInterval\s*\(/.test(source)) {
      violations.push({ code: "external-disk-polling-loop", file, line: 1 });
    }
  }

  if (requireCompleteLayout) {
    for (const boundary of CANONICAL_BOUNDARIES) {
      if (!boundaries.has(boundary)) {
        violations.push({
          code: "missing-external-disk-boundary",
          file: ROOT,
          line: 1,
          detail: boundary,
        });
      }
    }
    const facade = sources.get(FACADE) ?? "";
    if (!/\breconcileGeneration\s*=\s*0\b/.test(facade)) {
      violations.push({ code: "missing-runtime-reconcile-generation", file: FACADE, line: 1 });
    }
  }

  return {
    violations: violations.sort((left, right) => (
      left.file.localeCompare(right.file)
      || left.line - right.line
      || left.code.localeCompare(right.code)
    )),
    modules: [...sources.keys()].filter((file) => file.startsWith(ROOT)).length,
    boundaries: [...boundaries].sort(),
  };
}

export function analyzeExternalDiskModularity(projectRoot = process.cwd()) {
  const sourceFiles = filesUnder(path.join(projectRoot, "src"));
  return analyzeExternalDiskSources({
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
  const report = analyzeExternalDiskModularity();
  console.log(JSON.stringify(report, null, 2));
  if (report.violations.length > 0) process.exitCode = 1;
}
