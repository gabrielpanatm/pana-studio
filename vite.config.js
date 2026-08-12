import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { svelteCssFirstGuard } from "./scripts/vite-svelte-css-first.mjs";

const host = process.env.TAURI_DEV_HOST;

const sourceChunkRules = [
  ["/src/lib/i18n/generated/catalog.en-US.ts", "pana-locale-en-US"],
  ["/src/lib/i18n/generated/catalog.ro.ts", "pana-locale-ro"],
  ["/src/lib/i18n/", "pana-i18n-runtime"],
  ["/src/lib/state/", "pana-state"],
  ["/src/lib/components/inspector/", "pana-inspector"],
  ["/src/lib/inspector/", "pana-inspector"],
  ["/src/lib/css/", "pana-inspector"],
  ["/src/lib/editor-runtime/", "pana-editor"],
  ["/src/lib/editor/", "pana-editor"],
  ["/src/lib/html/", "pana-editor"],
  ["/src/lib/tera/", "pana-editor"],
  ["/src/lib/preview/", "pana-editor"],
  ["/src/lib/project/", "pana-project-bridge"],
  ["/src/lib/kernel/", "pana-project-bridge"],
  ["/src/lib/session/", "pana-project-bridge"],
  ["/src/lib/source-graph/", "pana-project-bridge"],
  ["/src/lib/workbench/", "pana-project-bridge"],
];

const acceptedCircularChunks = new Set([
  "Circular chunk: pana-editor -> pana-project-bridge -> pana-editor. Please adjust the manual chunk logic for these chunks.",
  "Circular chunk: pana-editor -> pana-state -> pana-editor. Please adjust the manual chunk logic for these chunks.",
  "Circular chunk: pana-editor -> pana-state -> pana-inspector -> pana-editor. Please adjust the manual chunk logic for these chunks.",
]);

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    svelteCssFirstGuard(),
    sveltekit(),
  ],
  build: {
    rollupOptions: {
      /**
       * @param {import("rollup").LogLevel} level
       * @param {import("rollup").RollupLog} warning
       * @param {import("rollup").LogOrStringHandler} defaultHandler
       */
      onLog(level, warning, defaultHandler) {
        // These cycles already exist between UI adapters (callbacks, projections and editor
        // controllers). The named chunks only expose that graph; no module is eagerly executed
        // for authority decisions, which remain Rust-owned. Keep every other warning visible.
        if (
          warning.code === "CIRCULAR_CHUNK"
          && acceptedCircularChunks.has(warning.message)
        ) {
          return;
        }
        defaultHandler(level, warning);
      },
      output: {
        /** @param {string} id */
        manualChunks(id) {
          const normalizedId = id.replaceAll("\\", "/");
          if (normalizedId.includes("/node_modules/@tabler/icons-svelte/")) {
            return "icons";
          }
          const sourceRule = sourceChunkRules.find(([fragment]) =>
            normalizedId.includes(fragment),
          );
          return sourceRule?.[1];
        },
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1431,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
