import type { SourceLanguage } from "$lib/types";
import { StateEffect, StateField, type Extension } from "@codemirror/state";
import { Decoration, EditorView } from "@codemirror/view";

type CodeSelectionRange = { from: number; to: number };
type CodeSelectionRanges = CodeSelectionRange | CodeSelectionRange[];

export const setSelectedSourceRange = StateEffect.define<CodeSelectionRanges | null>();

export const selectedSourceRangeField = StateField.define({
  create() {
    return Decoration.none;
  },
  update(decorations, transaction) {
    const mappedDecorations = decorations.map(transaction.changes);

    for (const effect of transaction.effects) {
      if (effect.is(setSelectedSourceRange)) {
        if (!effect.value) {
          return Decoration.none;
        }

        const ranges = (Array.isArray(effect.value) ? effect.value : [effect.value])
          .filter((range) => range.to > range.from);

        return Decoration.set(
          ranges.map((range) =>
            Decoration.mark({ class: "cm-selected-source-node" }).range(range.from, range.to),
          ),
        );
      }
    }

    return mappedDecorations;
  },
  provide: (field) => EditorView.decorations.from(field),
});

export function createCodeEditorTheme(theme: "dark" | "light") {
  const dark = theme === "dark";

  return EditorView.theme(
    {
      "&": {
        height: "100%",
        color: dark ? "#e7ede9" : "#1d2521",
        backgroundColor: dark ? "#101512" : "#f4f7f5",
      },
      ".cm-scroller": {
        overflow: "auto",
        fontFamily: '"JetBrains Mono", "SFMono-Regular", Consolas, monospace',
        lineHeight: "1.55",
      },
      ".cm-content": {
        padding: "16px",
        minHeight: "100%",
      },
      ".cm-gutters": {
        backgroundColor: dark ? "#121518" : "#eef3f0",
        color: dark ? "#74817b" : "#6b7972",
        borderRight: dark ? "1px solid #24282c" : "1px solid #d8e0db",
      },
      ".cm-activeLine": {
        backgroundColor: dark ? "rgba(29, 127, 106, 0.08)" : "rgba(29, 127, 106, 0.07)",
      },
      ".cm-activeLineGutter": {
        backgroundColor: dark ? "rgba(29, 127, 106, 0.14)" : "rgba(29, 127, 106, 0.12)",
      },
      ".cm-selectionBackground, ::selection": {
        backgroundColor: dark ? "rgba(47, 170, 140, 0.26)" : "rgba(29, 127, 106, 0.22)",
      },
      ".cm-cursor": {
        borderLeftColor: dark ? "#2faa8c" : "#1d7f6a",
      },
      ".cm-searchMatch": {
        backgroundColor: dark ? "rgba(201, 140, 255, 0.18)" : "rgba(201, 140, 255, 0.12)",
      },
      ".cm-selected-source-node": {
        backgroundColor: dark ? "rgba(29, 127, 106, 0.18)" : "rgba(29, 127, 106, 0.14)",
        boxShadow: `inset 0 0 0 1px ${dark ? "rgba(47, 170, 140, 0.55)" : "rgba(29, 127, 106, 0.5)"}`,
        borderRadius: "4px",
      },
    },
    { dark },
  );
}

export async function languageExtensionFor(language: SourceLanguage): Promise<Extension> {
  if (language === "html") {
    const { html } = await import("@codemirror/lang-html");
    return html();
  }

  if (language === "css") {
    const { css } = await import("@codemirror/lang-css");
    return css();
  }

  if (language === "scss") {
    const { css } = await import("@codemirror/lang-css");
    return css();
  }

  if (language === "js") {
    const { javascript } = await import("@codemirror/lang-javascript");
    return javascript();
  }

  if (language === "markdown") {
    const { markdown } = await import("@codemirror/lang-markdown");
    return markdown();
  }

  return [];
}
