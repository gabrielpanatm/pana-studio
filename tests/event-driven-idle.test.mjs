import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function source(relativePath) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("coordonarea AI este publicată prin evenimente și expiră la deadline în Rust", () => {
  const frontend = source("../src/lib/state/ai-coordination-controller.ts");
  const state = source("../src/lib/ai/coordination-state.svelte.ts");
  const backend = source("../src-tauri/src/commands/ai_coordination.rs");

  assert.match(frontend, /subscribeAiCoordinationChanges/);
  assert.match(frontend, /startAiCoordinationEvents/);
  assert.doesNotMatch(frontend, /AI_COORDINATION_POLL_MS/);
  assert.doesNotMatch(frontend, /setTimeout/);
  assert.match(state, /startAiCoordinationEvents\(this\.host\)/);
  assert.match(state, /stopAiCoordinationEvents\(this\.host\)/);

  assert.match(backend, /pana-ai-coordination-changed/);
  assert.match(backend, /schedule_ai_coordination_deadline/);
  assert.match(backend, /ai_coordination_deadline_generation/);
  assert.match(backend, /tokio::time::sleep\(Duration::from_millis\(delay_ms\)\)/);
});

test("monitorizarea proiectului este inotify, debounced și generation-safe", () => {
  const frontend = source("../src/lib/session/external-disk/monitor.ts");
  const io = source("../src/lib/project/io/external-disk.ts");
  const watcher = source("../src-tauri/src/project/watcher.rs");
  const command = [
    source("../src-tauri/src/commands/project/contracts.rs"),
    source("../src-tauri/src/commands/project/disk_watch.rs"),
  ].join("\n");

  assert.match(frontend, /subscribeProjectDiskChanges/);
  assert.match(frontend, /FULL_MANIFEST_AUDIT_INTERVAL\s*=\s*5\s*\*\s*60_000/);
  assert.doesNotMatch(frontend, /ACTIVE_CHECK_INTERVAL|BACKGROUND_CHECK_INTERVAL/);
  assert.doesNotMatch(frontend, /\b5_000\b|\b15_000\b/);
  assert.match(frontend, /expectedWatchGeneration:\s*receipt\.watchGeneration/);
  assert.match(io, /ProjectDiskWatchStopIdentity[\s\S]*expectedWatchGeneration:\s*number/);

  assert.match(watcher, /Inotify::init/);
  assert.match(watcher, /inotify\.read_events\(\)/);
  assert.match(watcher, /recv_timeout\(WATCH_DEBOUNCE\)/);
  assert.match(watcher, /WATCH_DEBOUNCE:\s*Duration\s*=\s*Duration::from_millis\(240\)/);
  assert.doesNotMatch(watcher, /thread::sleep/);
  assert.match(
    command,
    /ProjectDiskWatchStopRequest[\s\S]*expected_watch_generation:\s*u64/,
  );
  assert.match(command, /disk_watch_stop_request_is_current/);
  assert.match(command, /stop_identity_requires_exact_generation/);
  assert.match(command, /project_disk_watch_transition/);
});

test("serverele Preview dorm blocant în accept și sunt trezite explicit la stop", () => {
  for (const relativePath of [
    "../src-tauri/src/preview/server.rs",
    "../src-tauri/src/preview/source_browser/server.rs",
  ]) {
    const server = source(relativePath);
    assert.match(server, /listener\.accept\(\)/);
    assert.match(server, /TcpStream::connect\(\("127\.0\.0\.1", self\.port\)\)/);
    assert.doesNotMatch(server, /set_nonblocking\(true\)/);
    assert.doesNotMatch(server, /WouldBlock/);
    assert.doesNotMatch(server, /from_millis\(15\)/);
  }
});
