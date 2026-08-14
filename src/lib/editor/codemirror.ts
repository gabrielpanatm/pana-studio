import type { SourceLanguage } from "$lib/types";
import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import {
  StateEffect,
  StateField,
  type ChangeDesc,
  type EditorState,
  type Extension,
} from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";

type CodeSelectionRange = { from: number; to: number };
type CodeSelectionRanges = CodeSelectionRange | CodeSelectionRange[];

export type CodeSelectionPresentation = "range" | "htmlElement" | "cssRule";

export type CodeSelectionProjection = {
  ranges: CodeSelectionRanges;
  presentation: CodeSelectionPresentation;
};

export const setSelectedSourceProjection = StateEffect.define<CodeSelectionProjection | null>();

export const selectedSourceProjectionField = StateField.define<CodeSelectionProjection | null>({
  create() {
    return null;
  },
  update(projection, transaction) {
    let next = transaction.docChanged && projection
      ? mapCodeSelectionProjection(projection, transaction.changes)
      : projection;

    for (const effect of transaction.effects) {
      if (effect.is(setSelectedSourceProjection)) {
        next = effect.value;
      }
    }

    return next;
  },
});

const selectedSourceProjectionDecorations = ViewPlugin.fromClass(class {
  decorations: DecorationSet;

  constructor(view: EditorView) {
    this.decorations = selectedSourceDecorations(view.state);
  }

  update(update: ViewUpdate) {
    const previous = update.startState.field(selectedSourceProjectionField, false);
    const current = update.state.field(selectedSourceProjectionField, false);
    const syntaxChanged = syntaxTree(update.startState) !== syntaxTree(update.state);
    const reconfigured = update.transactions.some((transaction) => transaction.reconfigured);
    if (
      previous !== current
      || update.docChanged
      || update.viewportChanged
      || syntaxChanged
      || reconfigured
    ) {
      this.decorations = selectedSourceDecorations(update.state);
    }
  }
}, {
  decorations: (plugin) => plugin.decorations,
});

export const selectedSourceProjectionExtension: Extension = [
  selectedSourceProjectionField,
  selectedSourceProjectionDecorations,
];

export function codeSelectionDecorationRanges(
  state: EditorState,
  projection: CodeSelectionProjection,
): CodeSelectionRange[] {
  const sourceRanges = normalizedCodeSelectionRanges(projection.ranges, state.doc.length);
  if (projection.presentation === "range") return sourceRanges;
  if (sourceRanges.length === 0) return [];

  const parseTo = sourceRanges.reduce((maximum, range) => Math.max(maximum, range.to), 0);
  const tree = ensureSyntaxTree(state, parseTo, 30) ?? syntaxTree(state);
  const projected = sourceRanges.flatMap((range) => {
    if (projection.presentation === "htmlElement") {
      return htmlElementBoundaryRanges(tree, range);
    }
    return cssRuleBoundaryRanges(tree, range);
  });
  return normalizedCodeSelectionRanges(projected, state.doc.length);
}

function selectedSourceDecorations(state: EditorState): DecorationSet {
  const projection = state.field(selectedSourceProjectionField, false);
  if (!projection) return Decoration.none;
  const ranges = codeSelectionDecorationRanges(state, projection);
  return Decoration.set(
    ranges.map((range) =>
      Decoration.mark({ class: "cm-selected-source-node" }).range(range.from, range.to),
    ),
  );
}

function mapCodeSelectionProjection(
  projection: CodeSelectionProjection,
  changes: ChangeDesc,
): CodeSelectionProjection {
  const ranges = Array.isArray(projection.ranges) ? projection.ranges : [projection.ranges];
  return {
    presentation: projection.presentation,
    ranges: ranges.map((range) => ({
      from: changes.mapPos(range.from, 1),
      to: changes.mapPos(range.to, -1),
    })),
  };
}

type CodeSyntaxTree = ReturnType<typeof syntaxTree>;
type CodeSyntaxNode = ReturnType<CodeSyntaxTree["resolveInner"]>;

function htmlElementBoundaryRanges(
  tree: CodeSyntaxTree,
  range: CodeSelectionRange,
): CodeSelectionRange[] {
  const element = enclosingSyntaxNode(tree, range, "Element");
  if (!element) return [];
  const boundaries: CodeSelectionRange[] = [];
  for (let child = element.firstChild; child; child = child.nextSibling) {
    if (
      child.name === "OpenTag"
      || child.name === "SelfClosingTag"
      || child.name === "CloseTag"
    ) {
      boundaries.push({ from: child.from, to: child.to });
    }
  }
  return boundaries;
}

function cssRuleBoundaryRanges(
  tree: CodeSyntaxTree,
  selectorRange: CodeSelectionRange,
): CodeSelectionRange[] {
  const rule = enclosingSyntaxNode(tree, selectorRange, "RuleSet");
  if (!rule) return [selectorRange];
  const block = directChild(rule, "Block");
  if (!block) return [selectorRange];

  const ranges = [selectorRange];
  const openingBrace = directChild(block, "{");
  const closingBrace = directChild(block, "}");
  if (openingBrace) ranges.push({ from: openingBrace.from, to: openingBrace.to });
  if (closingBrace) ranges.push({ from: closingBrace.from, to: closingBrace.to });
  return ranges;
}

function enclosingSyntaxNode(
  tree: CodeSyntaxTree,
  range: CodeSelectionRange,
  nodeName: string,
): CodeSyntaxNode | null {
  const position = Math.min(range.from, tree.length);
  for (let node: CodeSyntaxNode | null = tree.resolveInner(position, 1); node; node = node.parent) {
    if (node.name === nodeName && node.from <= range.from && node.to >= range.to) {
      return node;
    }
  }
  return null;
}

function directChild(node: CodeSyntaxNode, nodeName: string): CodeSyntaxNode | null {
  for (let child = node.firstChild; child; child = child.nextSibling) {
    if (child.name === nodeName) return child;
  }
  return null;
}

function normalizedCodeSelectionRanges(
  ranges: CodeSelectionRanges,
  documentLength: number,
): CodeSelectionRange[] {
  const normalized = (Array.isArray(ranges) ? ranges : [ranges])
    .map((range) => ({
      from: Math.max(0, Math.min(documentLength, range.from)),
      to: Math.max(0, Math.min(documentLength, range.to)),
    }))
    .filter((range) => range.to > range.from)
    .sort((left, right) => left.from - right.from || left.to - right.to);

  return normalized.filter((range, index) => {
    const previous = normalized[index - 1];
    return !previous || previous.from !== range.from || previous.to !== range.to;
  });
}

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
      ".cm-foldGutter": {
        width: "18px",
      },
      ".cm-foldGutter .cm-gutterElement": {
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        boxSizing: "border-box",
        width: "18px",
        padding: "0",
        color: dark ? "#8c9a93" : "#607169",
      },
      ".cm-foldGutter .cm-gutterElement > span": {
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: "16px",
        height: "18px",
        padding: "0",
        borderRadius: "3px",
        fontSize: "16px",
        lineHeight: "1",
      },
      ".cm-foldGutter .cm-gutterElement > span:hover": {
        color: "var(--brand-strong)",
        backgroundColor: `color-mix(in srgb, var(--brand) ${dark ? "18%" : "12%"}, transparent)`,
      },
      ".cm-activeLine": {
        backgroundColor: `color-mix(in srgb, var(--brand) ${dark ? "8%" : "7%"}, transparent)`,
      },
      ".cm-activeLineGutter": {
        backgroundColor: `color-mix(in srgb, var(--brand) ${dark ? "14%" : "12%"}, transparent)`,
      },
      ".cm-selectionBackground, ::selection": {
        backgroundColor: `color-mix(in srgb, var(--brand) ${dark ? "26%" : "22%"}, transparent)`,
      },
      ".cm-cursor": {
        borderLeftColor: "var(--brand-strong)",
      },
      ".cm-searchMatch": {
        backgroundColor: dark ? "rgba(201, 140, 255, 0.18)" : "rgba(201, 140, 255, 0.12)",
      },
      ".cm-selected-source-node": {
        backgroundColor: `color-mix(in srgb, var(--brand) ${dark ? "18%" : "14%"}, transparent)`,
        boxShadow: `inset 0 0 0 1px color-mix(in srgb, var(--brand) ${dark ? "55%" : "50%"}, transparent)`,
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
    const { sass } = await import("@codemirror/lang-sass");
    return sass();
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
