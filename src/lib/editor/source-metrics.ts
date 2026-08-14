export type SourceTextMetrics = {
  characterCount: number;
  lineCount: number;
  utf8Bytes: number;
};

const utf8Encoder = new TextEncoder();

export function measureSourceText(source: string): SourceTextMetrics {
  let lineCount = 1;
  for (let index = 0; index < source.length; index += 1) {
    if (source.charCodeAt(index) === 10) {
      lineCount += 1;
    } else if (
      source.charCodeAt(index) === 13
      && source.charCodeAt(index + 1) !== 10
    ) {
      lineCount += 1;
    }
  }

  return {
    // CodeMirror and the surrounding editor contracts use UTF-16 offsets.
    characterCount: source.length,
    lineCount,
    utf8Bytes: utf8Encoder.encode(source).byteLength,
  };
}
