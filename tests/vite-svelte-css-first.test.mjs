import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  owningSvelteModuleUrl,
  svelteCssFirstGuard,
} from "../scripts/vite-svelte-css-first.mjs";

test("identifică numai submodulele CSS virtuale deținute de Svelte", () => {
  assert.equal(
    owningSvelteModuleUrl(
      "/src/lib/Panel.svelte?svelte&type=style&lang.css&t=123",
    ),
    "/src/lib/Panel.svelte",
  );
  assert.equal(owningSvelteModuleUrl("/src/lib/Panel.svelte"), null);
  assert.equal(
    owningSvelteModuleUrl("/src/app.css?svelte&type=style&lang.css"),
    null,
  );
  assert.equal(
    owningSvelteModuleUrl("/src/lib/Panel.svelte?svelte&type=script"),
    null,
  );
});

test("guard-ul rulează înaintea pipeline-ului CSS și numai în development", () => {
  const plugin = svelteCssFirstGuard();
  const config = readFileSync(
    new URL("../vite.config.js", import.meta.url),
    "utf8",
  );

  assert.equal(plugin.name, "pana:svelte-css-first-guard");
  assert.equal(plugin.apply, "serve");
  assert.equal(plugin.enforce, "pre");
  assert.equal(typeof plugin.configureServer, "function");
  assert.match(config, /svelteCssFirstGuard\(\)[\s\S]*sveltekit\(\)/);
});

test("cererile CSS concurente încălzesc o singură dată modulul proprietar", async () => {
  const plugin = svelteCssFirstGuard();
  let transformCount = 0;
  let middleware;
  let releaseWarmup;
  const warmupGate = new Promise((resolve) => {
    releaseWarmup = resolve;
  });

  plugin.configureServer({
    transformRequest: async () => {
      transformCount += 1;
      await warmupGate;
    },
    middlewares: {
      use(handler) {
        middleware = handler;
      },
    },
  });

  const request = {
    url: "/src/lib/Panel.svelte?svelte&type=style&lang.css",
  };
  const run = () => new Promise((resolve, reject) => {
    middleware(request, {}, (error) => {
      if (error) reject(error);
      else resolve();
    });
  });
  const first = run();
  const second = run();

  await Promise.resolve();
  assert.equal(transformCount, 1);
  releaseWarmup();
  await Promise.all([first, second]);
});
