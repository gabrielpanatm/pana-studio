import assert from "node:assert/strict";
import { test } from "node:test";
import { ApplicationPreferencesState } from "$lib/application/preferences.svelte";

function settingsSnapshot(revision, generation = 1) {
  return {
    revision,
    preferences: {
      language: { mode: "fixed", value: "en-US" },
      theme: { mode: "fixed", value: "dark" },
      accent: { mode: "fixed", value: "#1d7f6a" },
    },
    effective: {
      locale: "en-US",
      direction: "ltr",
      theme: "dark",
      accent: "#1d7f6a",
      accentSource: "fixed",
    },
    system: { generation, contrast: null, reducedMotion: null },
    brandAccent: "#1d7f6a",
    blockPropertiesHeight: 220,
    blockPropertiesCollapsed: false,
  };
}

function statusHarness() {
  const escalations = [];
  const clears = [];
  return {
    escalations,
    clears,
    status: {
      escalate: (value) => escalations.push(value),
      clear: (id) => clears.push(id),
    },
  };
}

test("Preferences serializează salvările CAS în ordinea reviziilor Rust", async () => {
  const { status, escalations } = statusHarness();
  let current = settingsSnapshot(1);
  let inFlight = 0;
  let maximumInFlight = 0;
  const expectedRevisions = [];
  const preferences = new ApplicationPreferencesState(status, {
    read: async () => current,
    save: async (expectedRevision) => {
      expectedRevisions.push(expectedRevision);
      inFlight += 1;
      maximumInFlight = Math.max(maximumInFlight, inFlight);
      await Promise.resolve();
      current = settingsSnapshot(expectedRevision + 1);
      inFlight -= 1;
      return current;
    },
    listen: async () => () => {},
  });

  const first = preferences.persistPatch({ theme: { mode: "fixed", value: "light" } });
  const second = preferences.persistPatch({ accent: { mode: "brand" } });
  await Promise.all([first, second]);

  assert.deepEqual(expectedRevisions, [1, 2]);
  assert.equal(maximumInFlight, 1);
  assert.equal(preferences.snapshot?.revision, 3);
  assert.deepEqual(escalations, []);
});

test("Preferences eliberează listenerul rezolvat după stop și ignoră callbackul vechi", async () => {
  const { status } = statusHarness();
  let resolveListener;
  let staleHandler;
  let unlistenCalls = 0;
  let reads = 0;
  const preferences = new ApplicationPreferencesState(status, {
    read: async () => {
      reads += 1;
      return settingsSnapshot(1, 9);
    },
    save: async () => settingsSnapshot(2),
    listen: (handler) => {
      staleHandler = handler;
      return new Promise((resolve) => {
        resolveListener = resolve;
      });
    },
  });

  const starting = preferences.start();
  preferences.stop();
  resolveListener(() => { unlistenCalls += 1; });
  await starting;
  staleHandler({ generation: 9 });
  await Promise.resolve();

  assert.equal(unlistenCalls, 1);
  assert.equal(reads, 0);
});
