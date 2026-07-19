<script lang="ts">
  import { onMount } from "svelte";
  import { Editor as TipTapEditor } from "@tiptap/core";
  import Image from "@tiptap/extension-image";
  import Link from "@tiptap/extension-link";
  import { Table } from "@tiptap/extension-table";
  import TableCell from "@tiptap/extension-table-cell";
  import TableHeader from "@tiptap/extension-table-header";
  import TableRow from "@tiptap/extension-table-row";
  import TaskItem from "@tiptap/extension-task-item";
  import TaskList from "@tiptap/extension-task-list";
  import { Markdown } from "@tiptap/markdown";
  import StarterKit from "@tiptap/starter-kit";
  import MarkdownToolbar, {
    type MarkdownToolbarState,
  } from "$lib/components/markdown/MarkdownToolbar.svelte";
  import { registerEditFlushHandler } from "$lib/session/edit-flush-registry";

  export let markdown = "";
  export let onChange: (nextMarkdown: string) => void = () => {};
  export let flushId = "";
  export let readOnly = false;
  export let onFlushSnapshot: (nextMarkdown: string) => void | Promise<void> = () => {};

  let host: HTMLDivElement;
  let editor: TipTapEditor | null = null;
  let syncedMarkdown = markdown;
  let lastAppMarkdown = markdown;
  let applyingExternalMarkdown = false;
  let localEditVersion = 0;
  let flushedLocalEditVersion = 0;
  let toolbarRefreshFrame = 0;
  let active: MarkdownToolbarState = createInactiveToolbarState();

  function createInactiveToolbarState(): MarkdownToolbarState {
    return {
      h1: false,
      h2: false,
      h3: false,
      h4: false,
      h5: false,
      h6: false,
      paragraph: false,
      bold: false,
      italic: false,
      strike: false,
      inlineCode: false,
      codeBlock: false,
      bulletList: false,
      orderedList: false,
      taskList: false,
      quote: false,
      link: false,
      table: false,
    };
  }

  function refreshToolbarState() {
    if (!editor) {
      active = createInactiveToolbarState();
      return;
    }

    const currentHeading = currentHeadingLevel();

    active = mergeMarkState({
      h1: currentHeading === 1,
      h2: currentHeading === 2,
      h3: currentHeading === 3,
      h4: currentHeading === 4,
      h5: currentHeading === 5,
      h6: currentHeading === 6,
      paragraph: isSelectionInsideNode("paragraph"),
      bold: false,
      italic: false,
      strike: false,
      inlineCode: false,
      codeBlock: isSelectionInsideNode("codeBlock"),
      bulletList: isSelectionInsideNode("bulletList"),
      orderedList: isSelectionInsideNode("orderedList"),
      taskList: isSelectionInsideNode("taskList"),
      quote: isSelectionInsideNode("blockquote"),
      link: false,
      table: isSelectionInsideNode("table"),
    });
  }

  function mergeMarkState(blockState: MarkdownToolbarState): MarkdownToolbarState {
    if (!editor) return blockState;

    return {
      ...blockState,
      bold: editor.isActive("bold"),
      italic: editor.isActive("italic"),
      strike: editor.isActive("strike"),
      inlineCode: editor.isActive("code"),
      link: editor.isActive("link"),
    };
  }

  function scheduleToolbarRefresh() {
    if (toolbarRefreshFrame) cancelAnimationFrame(toolbarRefreshFrame);
    toolbarRefreshFrame = requestAnimationFrame(() => {
      toolbarRefreshFrame = 0;
      refreshToolbarState();
    });
  }

  function emitMarkdownFromEditor(currentEditor: TipTapEditor) {
    if (applyingExternalMarkdown) return;

    const nextMarkdown = currentEditor.getMarkdown();
    if (nextMarkdown === syncedMarkdown) {
      scheduleToolbarRefresh();
      return;
    }

    syncedMarkdown = nextMarkdown;
    lastAppMarkdown = nextMarkdown;
    localEditVersion += 1;
    onChange(nextMarkdown);
    scheduleToolbarRefresh();
  }

  function applyMarkdownFromApp(nextMarkdown: string) {
    if (!editor) return;

    applyingExternalMarkdown = true;
    lastAppMarkdown = nextMarkdown;
    editor.commands.setContent(nextMarkdown, {
      contentType: "markdown",
      emitUpdate: false,
    });
    syncedMarkdown = editor.getMarkdown();
    localEditVersion = 0;
    flushedLocalEditVersion = 0;
    scheduleToolbarRefresh();
    applyingExternalMarkdown = false;
  }

  function isSelectionInsideNode(name: string) {
    if (!editor) return false;

    const { $from } = editor.state.selection;
    for (let depth = $from.depth; depth >= 0; depth -= 1) {
      if ($from.node(depth).type.name === name) return true;
    }
    return false;
  }

  function currentHeadingLevel() {
    if (!editor) return 0;

    const { $from } = editor.state.selection;
    for (let depth = $from.depth; depth >= 0; depth -= 1) {
      const node = $from.node(depth);
      if (node.type.name === "heading") return Number(node.attrs.level) || 1;
    }
    return 0;
  }

  function normalizeUrl(value: string) {
    const trimmed = value.trim();
    if (!trimmed) return "";
    if (/^(https?:|mailto:|tel:|\/|\.\/|\.\.\/|#)/i.test(trimmed)) return trimmed;
    return `https://${trimmed}`;
  }

  function runCommand(name: string, value?: string) {
    if (!editor || readOnly) return;

    switch (name) {
      case "paragraph":
        editor.chain().focus().setParagraph().run();
        break;
      case "downgradeHeading": {
        const level = currentHeadingLevel();
        if (level > 0 && level < 6) {
          editor.chain().focus().setHeading({ level: (level + 1) as 2 | 3 | 4 | 5 | 6 }).run();
        } else {
          editor.chain().focus().setParagraph().run();
        }
        break;
      }
      case "formatBlock":
        if (value?.startsWith("h")) {
          const level = Number(value.slice(1));
          if (level >= 1 && level <= 6) {
            editor.chain().focus().setHeading({ level: level as 1 | 2 | 3 | 4 | 5 | 6 }).run();
          }
        } else if (value === "blockquote") {
          editor.chain().focus().toggleBlockquote().run();
        } else if (value === "pre") {
          editor.chain().focus().toggleCodeBlock().run();
        }
        break;
      case "bold":
        editor.chain().focus().toggleBold().run();
        break;
      case "italic":
        editor.chain().focus().toggleItalic().run();
        break;
      case "strike":
        editor.chain().focus().toggleStrike().run();
        break;
      case "inlineCode":
        editor.chain().focus().toggleCode().run();
        break;
      case "insertUnorderedList":
        editor.chain().focus().toggleBulletList().run();
        break;
      case "insertOrderedList":
        editor.chain().focus().toggleOrderedList().run();
        break;
      case "insertTaskList":
        editor.chain().focus().toggleTaskList().run();
        break;
      case "sinkListItem":
        if (editor.isActive("taskItem")) {
          editor.chain().focus().sinkListItem("taskItem").run();
        } else {
          editor.chain().focus().sinkListItem("listItem").run();
        }
        break;
      case "liftListItem":
        if (editor.isActive("taskItem")) {
          editor.chain().focus().liftListItem("taskItem").run();
        } else {
          editor.chain().focus().liftListItem("listItem").run();
        }
        break;
      case "insertHardbreak":
        editor.chain().focus().setHardBreak().run();
        break;
      case "insertHr":
        editor.chain().focus().setHorizontalRule().run();
        break;
      case "insertTable":
        editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run();
        break;
      case "addRowBefore":
        editor.chain().focus().addRowBefore().run();
        break;
      case "addRowAfter":
        editor.chain().focus().addRowAfter().run();
        break;
      case "deleteRow":
        editor.chain().focus().deleteRow().run();
        break;
      case "addColumnBefore":
        editor.chain().focus().addColumnBefore().run();
        break;
      case "addColumnAfter":
        editor.chain().focus().addColumnAfter().run();
        break;
      case "deleteColumn":
        editor.chain().focus().deleteColumn().run();
        break;
      case "deleteTable":
        editor.chain().focus().deleteTable().run();
        break;
      case "createLink": {
        const currentHref = editor.getAttributes("link").href ?? "";
        const enteredHref = window.prompt("Link", currentHref);
        if (enteredHref === null) break;

        const href = normalizeUrl(enteredHref);
        if (!href) {
          editor.chain().focus().extendMarkRange("link").unsetLink().run();
        } else {
          editor.chain().focus().extendMarkRange("link").setLink({ href }).run();
        }
        break;
      }
      case "insertImage": {
        const src = window.prompt("Adresă imagine");
        if (!src) break;

        const alt = window.prompt("Text alternativ", "") ?? "";
        editor.chain().focus().setImage({ src: src.trim(), alt }).run();
        break;
      }
      case "insertEmoji":
        editor.chain().focus().insertContent("🙂").run();
        break;
    }

    scheduleToolbarRefresh();
  }

  onMount(() => {
    editor = new TipTapEditor({
      element: host,
      extensions: [
        StarterKit.configure({
          link: false,
          trailingNode: false,
          undoRedo: false,
        }),
        Link.configure({
          openOnClick: false,
          linkOnPaste: true,
          autolink: true,
          HTMLAttributes: {
            rel: null,
            target: null,
          },
        }),
        Image.configure({
          allowBase64: true,
        }),
        TaskList,
        TaskItem.configure({
          nested: true,
        }),
        Table.configure({
          resizable: false,
        }),
        TableRow,
        TableHeader,
        TableCell,
        Markdown.configure({
          indentation: {
            style: "space",
            size: 2,
          },
          markedOptions: {
            gfm: true,
          },
        }),
      ],
      content: markdown,
      contentType: "markdown",
      editable: !readOnly,
      editorProps: {
        attributes: {
          class: "tiptap-document",
          spellcheck: "true",
        },
        handleDOMEvents: {
          click: () => {
            scheduleToolbarRefresh();
            return false;
          },
          focus: () => {
            scheduleToolbarRefresh();
            return false;
          },
          keyup: () => {
            scheduleToolbarRefresh();
            return false;
          },
          mouseup: () => {
            scheduleToolbarRefresh();
            return false;
          },
        },
      },
      onUpdate: ({ editor: currentEditor }) => emitMarkdownFromEditor(currentEditor),
      onSelectionUpdate: scheduleToolbarRefresh,
      onFocus: scheduleToolbarRefresh,
      onBlur: scheduleToolbarRefresh,
      onTransaction: scheduleToolbarRefresh,
    });

    lastAppMarkdown = markdown;
    syncedMarkdown = editor.getMarkdown();
    refreshToolbarState();
    const unregisterFlushHandler = flushId
      ? registerEditFlushHandler(`markdown-editor:${flushId}`, async () => {
          if (localEditVersion === flushedLocalEditVersion) return;
          const flushingVersion = localEditVersion;
          const nextMarkdown = editor?.getMarkdown() ?? syncedMarkdown;
          await onFlushSnapshot(nextMarkdown);
          flushedLocalEditVersion = Math.max(flushedLocalEditVersion, flushingVersion);
        })
      : () => {};

    return () => {
      unregisterFlushHandler();
      if (toolbarRefreshFrame) cancelAnimationFrame(toolbarRefreshFrame);
      editor?.destroy();
      editor = null;
    };
  });

  $: if (editor && markdown !== lastAppMarkdown) {
    applyMarkdownFromApp(markdown);
  }

  $: if (editor) {
    editor.setEditable(!readOnly);
  }
</script>

<section class="tiptap-markdown-editor" aria-label="Document Markdown">
  <MarkdownToolbar command={runCommand} {active} disabled={readOnly} />
  <div bind:this={host} class="editor-host" role="presentation"></div>
</section>

<style>
  .tiptap-markdown-editor {
    --markdown-toolbar-top: 16px;
    position: relative;
    min-height: 0;
    height: 100%;
    background: var(--surface);
  }

  .editor-host {
    min-height: 0;
    height: 100%;
    overflow: auto;
    scrollbar-gutter: stable;
  }

  .editor-host :global(.tiptap-document) {
    box-sizing: border-box;
    width: min(100%, 920px);
    min-height: 100%;
    margin: 0 auto;
    padding: 82px clamp(28px, 6vw, 72px) 64px;
    outline: none;
    color: var(--text);
    font-family: "Inter", "Segoe UI", system-ui, sans-serif;
    font-size: 18px;
    line-height: 1.65;
  }

  .editor-host :global(.tiptap-document > *:first-child) {
    margin-top: 0;
  }

  .editor-host :global(.tiptap-document > *:last-child) {
    margin-bottom: 0;
  }

  .editor-host :global(p) {
    margin: 0.78em 0;
  }

  .editor-host :global(h1),
  .editor-host :global(h2),
  .editor-host :global(h3),
  .editor-host :global(h4),
  .editor-host :global(h5),
  .editor-host :global(h6) {
    margin: 1.16em 0 0.5em;
    color: var(--text);
    font-weight: 850;
    line-height: 1.12;
    letter-spacing: 0;
  }

  .editor-host :global(h1) {
    font-size: 42px;
  }

  .editor-host :global(h2) {
    font-size: 32px;
  }

  .editor-host :global(h3) {
    font-size: 25px;
  }

  .editor-host :global(h4) {
    font-size: 21px;
  }

  .editor-host :global(h5),
  .editor-host :global(h6) {
    font-size: 18px;
  }

  .editor-host :global(strong) {
    font-weight: 800;
  }

  .editor-host :global(blockquote) {
    margin: 1em 0;
    padding: 0.35em 0 0.35em 1.1em;
    border-left: 4px solid var(--brand);
    color: var(--text-muted);
  }

  .editor-host :global(ul),
  .editor-host :global(ol) {
    margin: 0.85em 0;
    padding-left: 1.45em;
  }

  .editor-host :global(li) {
    margin: 0.35em 0;
  }

  .editor-host :global(li p) {
    margin: 0.2em 0;
  }

  .editor-host :global(ul[data-type="taskList"]) {
    padding-left: 0;
    list-style: none;
  }

  .editor-host :global(li[data-type="taskItem"]) {
    display: flex;
    gap: 0.55em;
    align-items: flex-start;
  }

  .editor-host :global(li[data-type="taskItem"] > label) {
    display: inline-flex;
    padding-top: 0.22em;
  }

  .editor-host :global(li[data-type="taskItem"] input) {
    accent-color: var(--brand);
  }

  .editor-host :global(code) {
    padding: 0.14em 0.34em;
    border: 1px solid var(--border-2);
    border-radius: 5px;
    background: var(--surface-2);
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
    font-size: 0.86em;
  }

  .editor-host :global(pre) {
    overflow: auto;
    margin: 1.15em 0;
    padding: 14px 16px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .editor-host :global(pre code) {
    padding: 0;
    border: 0;
    background: transparent;
    font-size: 14px;
    line-height: 1.6;
  }

  .editor-host :global(hr) {
    height: 1px;
    margin: 1.6em 0;
    border: 0;
    background: var(--border-2);
  }

  .editor-host :global(a) {
    color: var(--brand);
    text-decoration: underline;
    text-underline-offset: 0.16em;
  }

  .editor-host :global(img) {
    display: block;
    max-width: 100%;
    height: auto;
    margin: 1em 0;
    border-radius: 8px;
  }

  .editor-host :global(table) {
    width: 100%;
    margin: 1.1em 0;
    border-collapse: collapse;
    table-layout: fixed;
    font-size: 0.94em;
  }

  .editor-host :global(th),
  .editor-host :global(td) {
    position: relative;
    min-width: 1em;
    padding: 8px 10px;
    border: 1px solid var(--border-2);
    vertical-align: top;
  }

  .editor-host :global(th) {
    background: var(--surface-2);
    font-weight: 800;
  }

  .editor-host :global(.selectedCell::after) {
    position: absolute;
    inset: 0;
    z-index: 2;
    pointer-events: none;
    content: "";
    background: color-mix(in srgb, var(--brand) 14%, transparent);
  }

  .editor-host :global(.ProseMirror-selectednode) {
    outline: 2px solid var(--brand);
  }
</style>
