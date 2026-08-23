import assert from "node:assert/strict";
import { test } from "node:test";
import { WorkspacePageLifecycle } from "$lib/application/workspace-page-lifecycle";

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => { resolve = resolvePromise; });
  return { promise, resolve };
}

function fakeWindow() {
  let nextHandle = 1;
  const listeners = new Map();
  const frames = new Set();
  const timers = new Set();
  const viewportListeners = new Map();
  const key = (type, listener) => `${type}:${String(listener)}`;
  return {
    localStorage: {},
    visualViewport: {
      scale: 1,
      addEventListener(type, listener) {
        viewportListeners.set(key(type, listener), listener);
      },
      removeEventListener(type, listener) {
        viewportListeners.delete(key(type, listener));
      },
    },
    addEventListener(type, listener) {
      listeners.set(key(type, listener), listener);
    },
    removeEventListener(type, listener) {
      listeners.delete(key(type, listener));
    },
    requestAnimationFrame() {
      const handle = nextHandle++;
      frames.add(handle);
      return handle;
    },
    cancelAnimationFrame(handle) { frames.delete(handle); },
    setTimeout() {
      const handle = nextHandle++;
      timers.add(handle);
      return handle;
    },
    clearTimeout(handle) { timers.delete(handle); },
    counts() {
      return {
        listeners: listeners.size,
        viewportListeners: viewportListeners.size,
        frames: frames.size,
        timers: timers.size,
      };
    },
  };
}

function runtime(events) {
  const previewHost = {
    session: { previewRefreshSerial: 0, previewDomTreeSerial: 0 },
    timers: { previewSync: null, domTreeFetch: null },
    projection: { confirmation: null },
  };
  return {
    status: { async refreshGlobalStatusFromKernel() {} },
    project: {
      async reattach() { return false; },
      startup: { async refreshFlow() {} },
    },
    preview: {
      runtime: { reset() { events.push("preview:reset"); } },
      commands: () => previewHost,
    },
    terminal: { destroy() { events.push("terminal:destroy"); } },
    source: {
      controller: { destroy() { events.push("source:destroy"); } },
    },
    selection: { pendingRestoredTimer: null },
    ai: {
      context: { clear() { events.push("ai-context:clear"); } },
      coordination: {
        start() {},
        stop() { events.push("ai:stop"); },
      },
    },
    externalDisk: { stop() { events.push("disk:stop"); } },
    editor: { destroy() { events.push("editor:destroy"); } },
  };
}

test("page lifecycle cleanup removes listeners, scheduled work and late async disposers", async () => {
  const originalWindow = globalThis.window;
  const windowFixture = fakeWindow();
  globalThis.window = windowFixture;
  const events = [];
  const listenerRegistration = deferred();
  let lifecycleListenerDisposed = 0;
  let smoothScrollingDisposed = 0;

  try {
    const lifecycle = new WorkspacePageLifecycle({
      resources: {
        domains: {
          start() { events.push("domains:start"); return true; },
          stop() { events.push("domains:stop"); return true; },
        },
        layout: {
          initialize() {},
          destroy() { events.push("layout:destroy"); },
        },
        preferences: {
          async start() {},
          async initialize() {},
          destroy() { events.push("preferences:destroy"); },
        },
        status: { destroy() { events.push("status:destroy"); } },
        unregisterRuntimeProbe() { events.push("probe:unregister"); },
      },
      runtime: runtime(events),
      events: {
        message() {},
        shortcut() {},
        deleteShortcut() {},
        projectLifecycle() {},
      },
      platform: {
        listenProjectLifecycle: () => listenerRegistration.promise,
        installSmoothScrolling: () => () => { smoothScrollingDisposed += 1; },
        async showWindow() {},
        resetNativeZoom() {},
      },
    });

    assert.equal(lifecycle.start(), true);
    assert.equal(lifecycle.start(), false);
    assert.deepEqual(windowFixture.counts(), {
      listeners: 7,
      viewportListeners: 2,
      frames: 1,
      timers: 0,
    });

    assert.equal(lifecycle.stop(), true);
    assert.equal(lifecycle.stop(), false);
    assert.deepEqual(windowFixture.counts(), {
      listeners: 0,
      viewportListeners: 0,
      frames: 0,
      timers: 0,
    });
    assert.equal(smoothScrollingDisposed, 1);

    listenerRegistration.resolve(() => { lifecycleListenerDisposed += 1; });
    await listenerRegistration.promise;
    await Promise.resolve();
    assert.equal(lifecycleListenerDisposed, 1);
    assert.deepEqual(events, [
      "domains:start",
      "domains:stop",
      "layout:destroy",
      "preferences:destroy",
      "status:destroy",
      "editor:destroy",
      "preview:reset",
      "terminal:destroy",
      "source:destroy",
      "ai-context:clear",
      "ai:stop",
      "disk:stop",
      "probe:unregister",
    ]);
  } finally {
    if (originalWindow === undefined) delete globalThis.window;
    else globalThis.window = originalWindow;
  }
});
