import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("Stocare este un tab separat, între Sistem și Despre", () => {
  const workspace = source("../src/lib/components/settings/SettingsWorkspace.svelte");

  assert.match(
    workspace,
    /\["general", "ai", "system", "storage", "about"\]/,
  );
  assert.match(workspace, /import StoragePane from/);
  assert.match(workspace, /activeSection === "storage"[\s\S]*<StoragePane \{app\} \/>/);
});

test("frontendul expune numai operațiile tipizate ale mecanismului de stocare", () => {
  const api = source("../src/lib/application/storage.ts");
  const pane = source("../src/lib/components/settings/StoragePane.svelte");

  for (const command of [
    "read_application_storage_inventory",
    "clear_application_cache_storage",
    "clear_application_log_storage",
    "delete_application_session_storage",
  ]) {
    assert.match(api, new RegExp(`invoke<[^>]+>\\("${command}"`));
  }
  assert.doesNotMatch(api, /path\s*:/i);
  assert.doesNotMatch(api, /removeDirectory|deleteDirectory|clearAll/i);
  assert.match(pane, /expectedRevision:\s*snapshot\.sessions\.revision/);
  assert.match(pane, /confirmedRecoverySessionIds:[\s\S]*hasRecovery/);
  assert.match(pane, /item\.defaultSelected/);
  assert.match(pane, /function selectAllDeletableSessions\(\)[\s\S]*filter\(\(item\) => item\.deletable\)/);
  assert.match(pane, /onclick=\{selectAllDeletableSessions\}/);
  assert.match(pane, /selectionChangedFromDefault\(\)/);
  assert.match(pane, /applyReceipt\(receipt, !selectionWasCustomized\)/);
  assert.match(pane, /lastReceipt\s*=\s*receipt/);
  assert.match(pane, /class="storage-success" role="status"/);
  assert.match(pane, /lastReceipt\.removedItems/);
  assert.match(pane, /lastReceipt\.freedBytes/);
  assert.match(pane, /lastReceipt\.protectedBytes/);
  assert.match(pane, /disabled=\{!item\.deletable/);
  assert.doesNotMatch(pane, /window\.confirm|confirm\(/);
});

test("Rust revalidează sesiunile și curăță selectiv cache-ul WebKit", () => {
  const storage = source("../src-tauri/src/application_storage.rs");
  const commands = source("../src-tauri/src/commands/storage.rs");
  const registry = source("../src-tauri/src/kernel/write_authority/registry.rs");

  assert.match(storage, /before\.sessions\.revision != request\.expected_revision/);
  assert.match(storage, /item\.active \|\| !item\.deletable/);
  assert.match(storage, /item\.has_recovery && !confirmed_recovery\.contains\(id\)/);
  assert.match(storage, /project_lifecycle_transition[\s\S]*\.lock\(\)/);
  assert.match(registry, /\["sessions", id\][\s\S]*valid_storage_session_id/);
  assert.match(registry, /\["preview", entry\][\s\S]*valid_storage_preview_entry/);

  assert.match(
    commands,
    /WebsiteDataTypes::DISK_CACHE \| WebsiteDataTypes::MEMORY_CACHE/,
  );
  assert.doesNotMatch(commands, /WebsiteDataTypes::ALL|LOCAL_STORAGE|clear_all_browsing_data/);
});

test("contractul UI declară explicit datele aflate în afara curățării", () => {
  const ro = source("../locales/ro/settings.ftl");
  const en = source("../locales/en-US/settings.ftl");

  for (const catalog of [ro, en]) {
    assert.match(catalog, /settings-storage-boundary\s*=/);
    assert.match(catalog, /settings-storage-confirm-cache-description\s*=/);
    assert.match(catalog, /settings-storage-confirm-recovery-description\s*=/);
  }
  assert.match(ro, /Configurația, MCP, WAL, credențialele, local storage, proiectele și \.panastudio/);
  assert.match(en, /Configuration, MCP, WAL, credentials, local storage, projects, and \.panastudio/);
});
