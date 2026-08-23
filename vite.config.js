import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createSourceLayerChunkMap } from "./scripts/source-layer-chunks.mjs";
import { svelteCssFirstGuard } from "./scripts/vite-svelte-css-first.mjs";

const host = process.env.TAURI_DEV_HOST;

const sourceChunkRules = [
  ["/src/lib/i18n/generated/catalog.en-US.ts", "pana-locale-en-US"],
  ["/src/lib/i18n/generated/catalog.ro.ts", "pana-locale-ro"],
];

const projectRoot = dirname(fileURLToPath(import.meta.url));
const sourceLayerChunks = createSourceLayerChunkMap({
  projectRoot,
  entry: "src/routes/+page.svelte",
  chunkNames: [
    "pana-core-foundation",
    "pana-core-domain",
    "pana-core-runtime",
    "pana-core-orchestration",
    "pana-application-shell",
  ],
  excludedFragments: [
    "/src/lib/i18n/generated/catalog.en-US.ts",
    "/src/lib/i18n/generated/catalog.ro.ts",
  ],
});

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    svelteCssFirstGuard(),
    sveltekit(),
  ],
  build: {
    rollupOptions: {
      output: {
        /** @param {string} id */
        manualChunks(id) {
          const normalizedId = id.replaceAll("\\", "/");
          if (normalizedId.includes("/node_modules/svelte/")) {
            return "pana-svelte-runtime";
          }
          if (normalizedId.includes("/node_modules/@tabler/icons-svelte/")) {
            return "pana-icons";
          }
          if (normalizedId.includes("/node_modules/@tauri-apps/")) {
            return "pana-tauri-runtime";
          }
          const sourceRule = sourceChunkRules.find(([fragment]) =>
            normalizedId.includes(fragment),
          );
          if (sourceRule) return sourceRule[1];
          return sourceLayerChunks.get(resolve(normalizedId.split("?", 1)[0]).replaceAll("\\", "/"));
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
