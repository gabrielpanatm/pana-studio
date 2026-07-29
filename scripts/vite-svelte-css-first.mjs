/**
 * @param {string | undefined} requestUrl
 * @returns {string | null}
 */
export function owningSvelteModuleUrl(requestUrl) {
  if (!requestUrl) return null;

  const url = new URL(requestUrl, "http://pana-studio.local");
  if (
    !url.pathname.endsWith(".svelte")
    || !url.searchParams.has("svelte")
    || url.searchParams.get("type") !== "style"
  ) {
    return null;
  }

  return url.pathname;
}

/** @returns {import("vite").Plugin} */
export function svelteCssFirstGuard() {
  /** @type {Map<string, Promise<void>>} */
  const pendingWarmups = new Map();

  return {
    name: "pana:svelte-css-first-guard",
    apply: "serve",
    enforce: "pre",
    /** @param {import("vite").ViteDevServer} server */
    configureServer(server) {
      server.middlewares.use(async (request, _response, next) => {
        const ownerUrl = owningSvelteModuleUrl(request.url);
        if (!ownerUrl) {
          next();
          return;
        }

        let warmup = pendingWarmups.get(ownerUrl);
        if (!warmup) {
          warmup = server.transformRequest(ownerUrl).then(() => undefined);
          pendingWarmups.set(ownerUrl, warmup);
        }

        try {
          await warmup;
          next();
        } catch (error) {
          next(error);
        } finally {
          if (pendingWarmups.get(ownerUrl) === warmup) {
            pendingWarmups.delete(ownerUrl);
          }
        }
      });
    },
  };
}
