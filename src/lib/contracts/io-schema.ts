import { t } from "$lib/i18n/runtime.svelte";

/** @internal Shared only by concrete domain IO boundaries. */
export function schemaMismatch(resource: string, actual: number, expected: number) {
  return new Error(t("io-schema-mismatch", { resource, actual, expected }));
}
