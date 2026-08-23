import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const SOURCE_EXTENSIONS = [".ts", ".svelte", ".js", ".mjs"];
const FORBIDDEN_CENTRAL_PATHS = new Set([
  "src/lib/types.ts",
  "src/lib/contracts.ts",
  "src/lib/contracts/index.ts",
  "src/lib/models.ts",
  "src/lib/models/index.ts",
]);
const MIGRATED_CONTRACT_MODULES = new Set([
  "src/lib/ai/contracts.ts",
  "src/lib/application/contracts.ts",
  "src/lib/audit/contracts.ts",
  "src/lib/blocks/contracts.ts",
  "src/lib/canvas/contracts.ts",
  "src/lib/content-models/contracts.ts",
  "src/lib/contracts/json-value.ts",
  "src/lib/contracts/localized-diagnostic.ts",
  "src/lib/creation/contracts.ts",
  "src/lib/data/contracts.ts",
  "src/lib/deploy/contracts.ts",
  "src/lib/editor/contracts.ts",
  "src/lib/fonts/contracts.ts",
  "src/lib/js/contracts.ts",
  "src/lib/kernel/observability-contract.ts",
  "src/lib/kernel/recovery-contract.ts",
  "src/lib/page-contracts/contracts.ts",
  "src/lib/preview/contracts.ts",
  "src/lib/project/external-disk-contract.ts",
  "src/lib/project/file-explorer-contract.ts",
  "src/lib/project/lifecycle-contract.ts",
  "src/lib/project/template-workbench-contract.ts",
  "src/lib/project/transition-contract.ts",
  "src/lib/project/workspace-contract.ts",
  "src/lib/source-graph/contracts.ts",
  "src/lib/source-graph/graph-contract.ts",
  "src/lib/taxonomies/contracts.ts",
  "src/lib/templates/contracts.ts",
  "src/lib/versioning/contracts.ts",
  "src/lib/workbench/contracts.ts",
]);

function filesUnder(directory, extensions) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) return filesUnder(target, extensions);
    return extensions.some((extension) => entry.name.endsWith(extension)) ? [target] : [];
  });
}

function numericInitializer(initializer) {
  let node = initializer;
  while (ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isParenthesizedExpression(node)) {
    node = node.expression;
  }
  return ts.isNumericLiteral(node) ? Number(node.text) : null;
}

function exported(statement) {
  return statement.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword) ?? false;
}

function pureContractModule(sourceFile) {
  let contractDeclarations = 0;
  for (const statement of sourceFile.statements) {
    if (ts.isImportDeclaration(statement)) continue;
    if (ts.isTypeAliasDeclaration(statement) || ts.isInterfaceDeclaration(statement)) {
      if (exported(statement)) contractDeclarations += 1;
      continue;
    }
    if (ts.isVariableStatement(statement)) {
      const schemaConstants = statement.declarationList.declarations.every(
        (declaration) => ts.isIdentifier(declaration.name)
          && declaration.name.text.endsWith("_SCHEMA_VERSION")
          && numericInitializer(declaration.initializer) !== null,
      );
      if (!exported(statement) || !schemaConstants) return false;
      contractDeclarations += statement.declarationList.declarations.length;
      continue;
    }
    return false;
  }
  return contractDeclarations > 0;
}

function lineForOffset(source, offset) {
  return source.slice(0, offset).split("\n").length;
}

export function analyzeContractOwnershipSources({
  sources,
  rustSources = new Map(),
  maxContractLines = 800,
  exportOwnershipFiles = null,
}) {
  const violations = [];
  const pureContractModules = new Map();
  const schemaOwners = new Map();
  const rustSchemas = new Map();
  let contractModules = 0;

  for (const [file, source] of rustSources) {
    for (const match of source.matchAll(/\bconst\s+([A-Z][A-Z0-9_]*_SCHEMA_VERSION)\s*:\s*u(?:32|64|size)\s*=\s*(\d+)\s*;/g)) {
      if (!rustSchemas.has(match[1])) rustSchemas.set(match[1], new Set());
      rustSchemas.get(match[1]).add(Number(match[2]));
    }
  }

  for (const [file, source] of sources) {
    const normalized = file.split(path.sep).join("/");
    if (FORBIDDEN_CENTRAL_PATHS.has(normalized)) {
      violations.push({ code: "central-contract-registry", file: normalized, line: 1 });
    }

    for (const match of source.matchAll(/["']\$lib\/types(?:["']|\/)/g)) {
      violations.push({
        code: "legacy-types-import",
        file: normalized,
        line: lineForOffset(source, match.index),
      });
    }

    const rootLevel = /^src\/lib\/[^/]+\.(?:ts|js)$/.test(normalized)
      || normalized === "src/lib/contracts/index.ts";
    if (rootLevel) {
      const domains = new Set();
      const exportPattern = /export\s+(?:type\s+)?(?:\*|\{[^}]*\})\s+from\s+["']\$lib\/([^/"']+)\/[^"']*(?:contract|model)[^"']*["']/g;
      for (const match of source.matchAll(exportPattern)) domains.add(match[1]);
      if (domains.size >= 2) {
        violations.push({
          code: "central-contract-barrel",
          file: normalized,
          line: 1,
          detail: [...domains].sort().join(","),
        });
      }
    }

    if (!normalized.endsWith(".ts")) continue;
    const sourceFile = ts.createSourceFile(normalized, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
    if (pureContractModule(sourceFile)) {
      contractModules += 1;
      pureContractModules.set(normalized, { source, sourceFile });
      const lines = source.split("\n").length;
      if (lines > maxContractLines) {
        violations.push({
          code: "oversized-contract-module",
          file: normalized,
          line: 1,
          detail: `${lines}>${maxContractLines}`,
        });
      }
    }

    for (const statement of sourceFile.statements) {
      if (!ts.isVariableStatement(statement) || !exported(statement)) continue;
      for (const declaration of statement.declarationList.declarations) {
        if (!ts.isIdentifier(declaration.name) || !declaration.name.text.endsWith("_SCHEMA_VERSION")) continue;
        const value = numericInitializer(declaration.initializer);
        if (value === null) continue;
        if (!schemaOwners.has(declaration.name.text)) schemaOwners.set(declaration.name.text, []);
        schemaOwners.get(declaration.name.text).push({ file: normalized, value });
      }
    }
  }

  const exportOwnershipModules = new Map(
    [...pureContractModules].filter(([file]) => !exportOwnershipFiles || exportOwnershipFiles.has(file)),
  );
  const importedContractNames = new Map(
    [...exportOwnershipModules].map(([file]) => [file, new Set()]),
  );
  const specifierToContract = new Map(
    [...exportOwnershipModules].flatMap(([file]) => {
      if (!file.startsWith("src/lib/") || !file.endsWith(".ts")) return [];
      return [[`$lib/${file.slice("src/lib/".length, -".ts".length)}`, file]];
    }),
  );
  for (const source of sources.values()) {
    const namedImportPattern = /(?:import|export)\s+(?:type\s+)?\{([^}]*)\}\s+from\s+["']([^"']+)["']/g;
    for (const match of source.matchAll(namedImportPattern)) {
      const owner = specifierToContract.get(match[2]);
      if (!owner) continue;
      for (const rawName of match[1].split(",")) {
        const name = rawName.trim().replace(/^type\s+/, "").split(/\s+as\s+/)[0]?.trim();
        if (name) importedContractNames.get(owner).add(name);
      }
    }
    for (const match of source.matchAll(/import\(["']([^"']+)["']\)\.([A-Za-z_$][\w$]*)/g)) {
      const owner = specifierToContract.get(match[1]);
      if (owner) importedContractNames.get(owner).add(match[2]);
    }
  }

  for (const [file, { sourceFile }] of exportOwnershipModules) {
    const identifierCounts = new Map();
    const countIdentifiers = (node) => {
      if (ts.isIdentifier(node)) {
        identifierCounts.set(node.text, (identifierCounts.get(node.text) ?? 0) + 1);
      }
      ts.forEachChild(node, countIdentifiers);
    };
    countIdentifiers(sourceFile);

    const checkExport = (name, statement) => {
      if (importedContractNames.get(file).has(name)) return;
      const { line } = sourceFile.getLineAndCharacterOfPosition(statement.getStart(sourceFile));
      violations.push({
        code: (identifierCounts.get(name) ?? 0) > 1
          ? "unnecessary-contract-export"
          : "dead-contract-export",
        file,
        line: line + 1,
        detail: name,
      });
    };

    for (const statement of sourceFile.statements) {
      if (!exported(statement)) continue;
      if (ts.isTypeAliasDeclaration(statement) || ts.isInterfaceDeclaration(statement)) {
        checkExport(statement.name.text, statement);
        continue;
      }
      if (!ts.isVariableStatement(statement)) continue;
      for (const declaration of statement.declarationList.declarations) {
        if (ts.isIdentifier(declaration.name)) checkExport(declaration.name.text, statement);
      }
    }
  }

  for (const [name, owners] of schemaOwners) {
    if (owners.length > 1) {
      violations.push({
        code: "duplicate-schema-owner",
        file: owners[0].file,
        line: 1,
        detail: `${name}:${owners.map((owner) => owner.file).join(",")}`,
      });
    }
    const rustValues = rustSchemas.get(name);
    if (rustValues && (rustValues.size !== 1 || !rustValues.has(owners[0].value))) {
      violations.push({
        code: "schema-version-drift",
        file: owners[0].file,
        line: 1,
        detail: `${name}:ts=${owners[0].value},rust=${[...rustValues].sort().join("|")}`,
      });
    }
  }

  return {
    violations: violations.sort((left, right) =>
      left.file.localeCompare(right.file) || left.line - right.line || left.code.localeCompare(right.code)),
    contractModules,
    schemaConstants: schemaOwners.size,
    rustSchemaConstants: rustSchemas.size,
  };
}

export function analyzeContractOwnership(projectRoot = process.cwd()) {
  const sourceFiles = [
    ...filesUnder(path.join(projectRoot, "src"), SOURCE_EXTENSIONS),
    ...filesUnder(path.join(projectRoot, "tests"), SOURCE_EXTENSIONS),
  ];
  const rustFiles = filesUnder(path.join(projectRoot, "src-tauri/src"), [".rs"]);
  const relative = (file) => path.relative(projectRoot, file);
  return analyzeContractOwnershipSources({
    sources: new Map(sourceFiles.map((file) => [relative(file), readFileSync(file, "utf8")])),
    rustSources: new Map(rustFiles.map((file) => [relative(file), readFileSync(file, "utf8")])),
    exportOwnershipFiles: MIGRATED_CONTRACT_MODULES,
  });
}

const isMain = process.argv[1]
  && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));
if (isMain) {
  const report = analyzeContractOwnership();
  console.log(JSON.stringify(report, null, 2));
  if (report.violations.length > 0) process.exitCode = 1;
}
