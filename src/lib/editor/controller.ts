import { createCodeEditorTheme, languageExtensionFor, selectedSourceRangeField, setSelectedSourceRange } from "$lib/editor/codemirror";
import type { SourceLanguage } from "$lib/types";
import { defaultKeymap } from "@codemirror/commands";
import { bracketMatching, defaultHighlightStyle, indentOnInput, syntaxHighlighting } from "@codemirror/language";
import { Compartment, EditorState, Transaction } from "@codemirror/state";
import {
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  EditorView,
  type ViewUpdate,
} from "@codemirror/view";
import type { FileBufferTextChange } from "$lib/types";

type CodeEditorControllerOptions = {
  host: HTMLDivElement;
  doc: string;
  language: SourceLanguage;
  theme: "dark" | "light";
  readOnly?: boolean;
  onDocumentChange: (nextSource: string, cursorPosition: number, changeSet: CodeEditorDocumentChangeSet) => void;
  onSelectionChange: (cursorPosition: number, docText: string) => void;
  onContextMenu?: (request: CodeEditorContextMenuRequest) => void;
};

type CodeSelectionRange = { from: number; to: number };
type CodeSelectionRanges = CodeSelectionRange | CodeSelectionRange[];

export type CodeEditorController = {
  destroy: () => void;
  getDoc: () => string;
  setDoc: (source: string) => void;
  setLanguage: (language: SourceLanguage) => void;
  setTheme: (theme: "dark" | "light") => void;
  setReadOnly: (readOnly: boolean) => void;
  setSelectedRange: (range: CodeSelectionRanges | null, reveal?: boolean) => void;
};

export type CodeEditorDocumentChangeSet = {
  coordinateSpace: "utf16";
  changes: FileBufferTextChange[];
};

export type CodeEditorContextMenuRequest = {
  event: MouseEvent;
  position: number;
  line: number;
  column: number;
  hasSelection: boolean;
  selectedText: string;
  docText: string;
};

const panaStudioEditorSetup = [
  lineNumbers(),
  highlightActiveLineGutter(),
  highlightSpecialChars(),
  drawSelection(),
  dropCursor(),
  EditorState.allowMultipleSelections.of(true),
  indentOnInput(),
  syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
  bracketMatching(),
  highlightActiveLine(),
  keymap.of(defaultKeymap),
];

export function createCodeEditorController(options: CodeEditorControllerOptions): CodeEditorController {
  const languageCompartment = new Compartment();
  const themeCompartment = new Compartment();
  const readOnlyCompartment = new Compartment();
  let syncingDoc = false;
  let syncingSelection = false;
  let languageRequest = 0;

  const view = new EditorView({
    parent: options.host,
    state: EditorState.create({
      doc: options.doc,
      extensions: [
        panaStudioEditorSetup,
        languageCompartment.of([]),
        themeCompartment.of(createCodeEditorTheme(options.theme)),
        readOnlyCompartment.of([
          EditorState.readOnly.of(Boolean(options.readOnly)),
          EditorView.editable.of(!options.readOnly),
        ]),
        selectedSourceRangeField,
        EditorView.domEventHandlers({
          contextmenu: (event, view) => {
            if (!options.onContextMenu) return false;
            event.preventDefault();
            const position = view.posAtCoords({ x: event.clientX, y: event.clientY }) ?? view.state.selection.main.head;
            const line = view.state.doc.lineAt(position);
            const selection = view.state.selection.main;
            const selectedText = selection.empty ? "" : view.state.doc.sliceString(selection.from, selection.to);
            options.onContextMenu({
              event,
              position,
              line: line.number,
              column: position - line.from + 1,
              hasSelection: !selection.empty,
              selectedText,
              docText: view.state.doc.toString(),
            });
            return true;
          },
        }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged && !syncingDoc) {
            const nextSource = update.state.doc.toString();
            options.onDocumentChange(
              nextSource,
              update.state.selection.main.head,
              codeEditorChangeSetFromUpdate(update),
            );
          }

          if (update.selectionSet && !syncingSelection) {
            options.onSelectionChange(update.state.selection.main.head, update.state.doc.toString());
          }
        }),
      ],
    }),
  });

  async function applyLanguage(language: SourceLanguage) {
    const request = ++languageRequest;
    const extension = await languageExtensionFor(language);
    if (request !== languageRequest) return;
    view.dispatch({
      effects: languageCompartment.reconfigure(extension),
    });
  }

  void applyLanguage(options.language);

  return {
    destroy() {
      view.destroy();
    },
    getDoc() {
      return view.state.doc.toString();
    },
    setDoc(source: string) {
      const currentDoc = view.state.doc.toString();

      if (currentDoc === source) {
        return;
      }

      syncingDoc = true;
      view.dispatch({
        changes: {
          from: 0,
          to: currentDoc.length,
          insert: source,
        },
        annotations: Transaction.addToHistory.of(false),
      });
      syncingDoc = false;
    },
    setLanguage(language: SourceLanguage) {
      void applyLanguage(language);
    },
    setTheme(theme: "dark" | "light") {
      view.dispatch({
        effects: themeCompartment.reconfigure(createCodeEditorTheme(theme)),
      });
    },
    setReadOnly(readOnly: boolean) {
      view.dispatch({
        effects: readOnlyCompartment.reconfigure([
          EditorState.readOnly.of(readOnly),
          EditorView.editable.of(!readOnly),
        ]),
      });
    },
    setSelectedRange(range, reveal = false) {
      syncingSelection = true;
      const effects: Array<ReturnType<typeof setSelectedSourceRange.of> | ReturnType<typeof EditorView.scrollIntoView>> = [
        setSelectedSourceRange.of(range),
      ];

      if (reveal && range) {
        const revealRange = Array.isArray(range) ? range[0] : range;
        if (revealRange) {
          effects.push(EditorView.scrollIntoView(revealRange.from, { y: "center" }));
        }
      }

      view.dispatch({ effects });
      syncingSelection = false;
    },
  };
}

function codeEditorChangeSetFromUpdate(update: ViewUpdate): CodeEditorDocumentChangeSet {
  const changes: FileBufferTextChange[] = [];
  update.changes.iterChanges((from, to, _fromB, _toB, inserted) => {
    changes.push({
      from,
      to,
      insert: inserted.toString(),
    });
  });
  return {
    coordinateSpace: "utf16",
    changes,
  };
}
