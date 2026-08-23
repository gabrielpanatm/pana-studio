import assert from "node:assert/strict";
import { test } from "node:test";

import { analyzeContractOwnershipSources } from "../scripts/analyze-contract-ownership.mjs";

function analyze(sources, rustSources = new Map()) {
  return analyzeContractOwnershipSources({ sources: new Map(sources), rustSources });
}

test("guard-ul acceptă contracte de domeniu focalizate și schema aliniată cu Rust", () => {
  const report = analyze(
    [
      ["src/lib/project/contracts.ts", `
        export const PROJECT_SCHEMA_VERSION = 3 as const;
        export type ProjectSnapshot = { schemaVersion: typeof PROJECT_SCHEMA_VERSION };
      `],
      ["src/lib/project/io.ts", `
        import { PROJECT_SCHEMA_VERSION, type ProjectSnapshot } from "$lib/project/contracts";
        export function valid(snapshot: ProjectSnapshot) {
          return snapshot.schemaVersion === PROJECT_SCHEMA_VERSION;
        }
      `],
    ],
    new Map([["src-tauri/src/project.rs", "pub const PROJECT_SCHEMA_VERSION: u32 = 3;"]]),
  );
  assert.deepEqual(report.violations, []);
  assert.equal(report.contractModules, 1);
});

test("guard-ul respinge registrul legacy, importul vechi și barrel-ul central", () => {
  const legacySpecifier = "$lib/" + "types";
  const report = analyze([
    ["src/lib/types.ts", "export type Legacy = string;"],
    ["src/lib/consumer.ts", `import type { Legacy } from '${legacySpecifier}';`],
    ["src/lib/contracts.ts", `
      export type { ProjectSnapshot } from "$lib/project/contracts";
      export type { DeploySnapshot } from "$lib/deploy/contracts";
    `],
  ]);
  assert.deepEqual(
    new Set(report.violations.map((violation) => violation.code)),
    new Set(["central-contract-registry", "legacy-types-import", "central-contract-barrel"]),
  );
});

test("guard-ul respinge modulele supradimensionate, ownerii dubli și drift-ul de schemă", () => {
  const oversized = `export type Huge = string;\n${"\n".repeat(800)}`;
  const report = analyze(
    [
      ["src/lib/project/contracts.ts", oversized],
      ["src/lib/project/schema.ts", "export const PROJECT_SCHEMA_VERSION = 2 as const;"],
      ["src/lib/project/duplicate-schema.ts", "export const PROJECT_SCHEMA_VERSION = 2 as const;"],
      ["src/lib/project/consumer.ts", `
        import type { Huge } from "$lib/project/contracts";
        import { PROJECT_SCHEMA_VERSION as first } from "$lib/project/schema";
        import { PROJECT_SCHEMA_VERSION as second } from "$lib/project/duplicate-schema";
        export const fixture: [Huge, number] = ["value", first + second];
      `],
    ],
    new Map([["src-tauri/src/project.rs", "pub const PROJECT_SCHEMA_VERSION: u32 = 3;"]]),
  );
  assert.deepEqual(
    new Set(report.violations.map((violation) => violation.code)),
    new Set(["oversized-contract-module", "duplicate-schema-owner", "schema-version-drift"]),
  );
});

test("guard-ul respinge exporturile de contract moarte sau care sunt doar detalii interne", () => {
  const report = analyze([
    ["src/lib/project/contracts.ts", `
      export type InternalDetail = { value: string };
      export type PublicSnapshot = InternalDetail & { id: string };
      export type DeadSnapshot = { obsolete: true };
    `],
    ["src/lib/project/consumer.ts", `
      import type { PublicSnapshot } from "$lib/project/contracts";
      export const consume = (snapshot: PublicSnapshot) => snapshot.id;
    `],
  ]);
  assert.deepEqual(
    report.violations.map(({ code, detail }) => [code, detail]),
    [
      ["unnecessary-contract-export", "InternalDetail"],
      ["dead-contract-export", "DeadSnapshot"],
    ],
  );
});
