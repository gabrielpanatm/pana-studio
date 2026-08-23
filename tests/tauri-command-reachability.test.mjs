import assert from "node:assert/strict";
import { test } from "node:test";

import {
  analyzeTauriCommandReachability,
  collectCommandLiteralsFromSource,
  findUnreachableTauriCommands,
  parseTauriCommandRegistry,
} from "../scripts/analyze-tauri-command-reachability.mjs";

test("parserul extrage exclusiv registry-ul canonic Tauri", () => {
  const registry = `
    macro_rules! pana_tauri_commands {
      ($consumer:ident) => { $consumer!(read_alpha, write_beta,) };
    }
    const NOISE: &[&str] = &["dead_gamma"];
  `;
  assert.deepEqual(parseTauriCommandRegistry(registry), ["read_alpha", "write_beta"]);
});

test("reachability acceptă orice helper real, dar nu simple mențiuni de nume", () => {
  const commands = new Set(["read_alpha", "write_beta", "dead_gamma"]);
  const found = collectCommandLiteralsFromSource(`
    const documentation = "dead_gamma";
    invoke("read_alpha", {});
    invokeWorkspaceMutation("write_beta", input);
    log("label", "dead_gamma");
  `, commands);
  assert.deepEqual(found, new Set(["read_alpha", "write_beta"]));
  assert.deepEqual(
    findUnreachableTauriCommands([...commands], found),
    ["dead_gamma"],
  );
});

test("allowlist-ul cere o comandă existentă și o justificare explicită", () => {
  assert.deepEqual(
    findUnreachableTauriCommands(
      ["read_alpha"],
      new Set(),
      new Map([["read_alpha", "Consumator extern documentat în contractul desktop."]]),
    ),
    [],
  );
  assert.throws(
    () => findUnreachableTauriCommands(["read_alpha"], new Set(), new Map([["ghost", "Motiv suficient de lung."]])),
    /nu este în registry/,
  );
});

test("registry-ul aplicației nu expune comenzi fără consumator frontend", () => {
  assert.deepEqual(analyzeTauriCommandReachability().unreachable, []);
});
