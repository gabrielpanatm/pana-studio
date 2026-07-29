/**
 * Stable language-neutral marker used while Rust is loading a source buffer.
 * It must never be translated because application logic compares it by identity.
 */
export const SOURCE_LOADING_SENTINEL = "\u0000pana-studio:source-loading";
