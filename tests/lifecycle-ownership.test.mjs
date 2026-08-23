import assert from "node:assert/strict";
import { test } from "node:test";
import { LifecycleGroup } from "$lib/lifecycle/group";
import { ReactiveEffectsLifecycle } from "$lib/lifecycle/reactive-effects.svelte";

test("ReactiveEffectsLifecycle înregistrează o singură rădăcină și o eliberează", () => {
  const effect = () => {};
  const registrations = [];
  let cleanupCount = 0;
  const lifecycle = new ReactiveEffectsLifecycle([effect], (effects) => {
    registrations.push(effects);
    return () => {
      cleanupCount += 1;
    };
  });

  assert.equal(lifecycle.start(), true);
  assert.equal(lifecycle.start(), false);
  assert.deepEqual(registrations, [[effect]]);
  assert.equal(lifecycle.stop(), true);
  assert.equal(lifecycle.stop(), false);
  assert.equal(cleanupCount, 1);
});

test("LifecycleGroup oprește resursele în ordine inversă și face rollback la eroare", () => {
  const events = [];
  const first = {
    start: () => { events.push("start:first"); return true; },
    stop: () => { events.push("stop:first"); return true; },
  };
  const second = {
    start: () => { events.push("start:second"); throw new Error("boom"); },
    stop: () => { events.push("stop:second"); return true; },
  };
  const group = new LifecycleGroup([first, second]);

  assert.throws(() => group.start(), /boom/);
  assert.deepEqual(events, ["start:first", "start:second", "stop:first"]);
  assert.equal(group.active, false);
});
