import assert from "node:assert/strict";
import { test } from "node:test";
import {
  destroyApplicationRuntime,
  initializeApplicationRuntime,
} from "$lib/application/runtime-lifecycle";

if (!globalThis.window) globalThis.window = globalThis;

function dependencies(events, reattached = false) {
  const previewHost = {
    session: { previewRefreshSerial: 4, previewDomTreeSerial: 7 },
    timers: { previewSync: null, domTreeFetch: null },
    projection: { confirmation: null },
  };
  const source = {
    controller: {
      destroy() { events.push("source:destroy"); },
    },
  };
  const selection = { pendingRestoredTimer: null };
  return {
    value: {
      status: {
        async refreshGlobalStatusFromKernel() { events.push("status:refresh"); },
      },
      project: {
        async reattach() { events.push("project:reattach"); return reattached; },
        startup: {
          async refreshFlow() { events.push("startup:refresh"); },
        },
      },
      preview: {
        runtime: { reset() { events.push("preview:reset"); } },
        commands: () => previewHost,
      },
      terminal: { destroy() { events.push("terminal:destroy"); } },
      source,
      selection,
      ai: {
        context: { clear() { events.push("ai-context:clear"); } },
        coordination: {
          start() { events.push("ai:start"); },
          stop() { events.push("ai:stop"); },
        },
      },
      externalDisk: { stop() { events.push("disk:stop"); } },
      editor: { destroy() { events.push("editor:destroy"); } },
    },
    previewHost,
    selection,
    source,
  };
}

test("runtime startup pornește coordonarea și folosește Startup numai fără reattach", async () => {
  const events = [];
  await initializeApplicationRuntime(dependencies(events).value);
  assert.deepEqual(events, [
    "status:refresh",
    "ai:start",
    "project:reattach",
    "startup:refresh",
  ]);

  events.length = 0;
  await initializeApplicationRuntime(dependencies(events, true).value);
  assert.deepEqual(events, ["status:refresh", "ai:start", "project:reattach"]);
});

test("runtime startup păstrează Startup interactiv după eșecul reattach", async () => {
  const events = [];
  const fixture = dependencies(events).value;
  fixture.project.reattach = async () => {
    events.push("project:reattach");
    throw new Error("frontend lease failed");
  };

  await initializeApplicationRuntime(fixture);

  assert.deepEqual(events, [
    "status:refresh",
    "ai:start",
    "project:reattach",
    "startup:refresh",
  ]);
});

test("runtime destroy eliberează fiecare resursă și invalidează timer-ele Preview", () => {
  const events = [];
  const fixture = dependencies(events);
  destroyApplicationRuntime(fixture.value);

  assert.deepEqual(events, [
    "editor:destroy",
    "preview:reset",
    "terminal:destroy",
    "source:destroy",
    "ai-context:clear",
    "ai:stop",
    "disk:stop",
  ]);
  assert.equal(fixture.source.controller, null);
  assert.equal(fixture.previewHost.session.previewRefreshSerial, 5);
  assert.equal(fixture.previewHost.session.previewDomTreeSerial, 8);
});
