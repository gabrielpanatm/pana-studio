// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import { readApplicationSettings } from "$lib/application/io";
import { initializeLocalization } from "$lib/i18n/runtime.svelte";

export const ssr = false;

export async function load() {
  let locale: string | undefined;
  try {
    // Rust is authoritative even on the first launch, when no paint projection
    // exists in localStorage yet. The preferences owner reads a fresh snapshot again after it
    // installs the live system-preference listener.
    locale = (await readApplicationSettings()).effective.locale;
  } catch {
    // Browser-only development has no Tauri IPC. The runtime uses the validated
    // paint projection when present and the base locale otherwise.
  }
  await initializeLocalization(locale);
}
