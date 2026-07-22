<script lang="ts" context="module">
  export type MarkdownToolbarState = {
    h1: boolean;
    h2: boolean;
    h3: boolean;
    h4: boolean;
    h5: boolean;
    h6: boolean;
    paragraph: boolean;
    bold: boolean;
    italic: boolean;
    strike: boolean;
    inlineCode: boolean;
    codeBlock: boolean;
    bulletList: boolean;
    orderedList: boolean;
    taskList: boolean;
    quote: boolean;
    link: boolean;
    table: boolean;
  };
</script>

<script lang="ts">
  import {
    IconBold,
    IconBrackets,
    IconChevronDown,
    IconCode,
    IconColumnInsertLeft,
    IconColumnInsertRight,
    IconColumnRemove,
    IconIndentDecrease,
    IconIndentIncrease,
    IconItalic,
    IconLink,
    IconList,
    IconListCheck,
    IconListNumbers,
    IconMoodSmile,
    IconPhoto,
    IconQuote,
    IconRowInsertBottom,
    IconRowInsertTop,
    IconRowRemove,
    IconSeparatorHorizontal,
    IconStrikethrough,
    IconTable,
    IconTablePlus,
    IconTrash,
    IconTypography,
    IconUnderline,
  } from "@tabler/icons-svelte";

  export let command: (name: string, value?: string) => void;
  export let disabled = false;
  export let active: MarkdownToolbarState = {
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

  function run(event: MouseEvent, name: string, value?: string) {
    const menu = event.currentTarget instanceof HTMLElement
      ? event.currentTarget.closest("details")
      : null;
    command(name, value);
    if (menu instanceof HTMLDetailsElement) menu.open = false;
  }

  function closeSiblingMenus(event: Event) {
    const current = event.currentTarget;
    if (!(current instanceof HTMLDetailsElement) || !current.open) return;
    current.parentElement?.querySelectorAll("details[open]").forEach((menu) => {
      if (menu !== current && menu instanceof HTMLDetailsElement) menu.open = false;
    });
  }

  function blockLabelFor(state: MarkdownToolbarState) {
    if (state.h1) return "H1";
    if (state.h2) return "H2";
    if (state.h3) return "H3";
    if (state.h4) return "H4";
    if (state.h5) return "H5";
    if (state.h6) return "H6";
    if (state.codeBlock) return "Cod";
    if (state.quote) return "Citat";
    if (state.paragraph) return "Text";
    return "Bloc";
  }

  function blockActiveFor(state: MarkdownToolbarState) {
    return state.h1
      || state.h2
      || state.h3
      || state.h4
      || state.h5
      || state.h6
      || state.paragraph
      || state.codeBlock
      || state.quote;
  }

  function listActiveFor(state: MarkdownToolbarState) {
    return state.bulletList || state.orderedList || state.taskList;
  }

  function listLabelFor(state: MarkdownToolbarState) {
    if (state.taskList) return "To-Do";
    if (state.orderedList) return "Numerotată";
    if (state.bulletList) return "Simplă";
    return "Liste";
  }

  function insertActiveFor(state: MarkdownToolbarState) {
    return state.codeBlock || state.quote || state.table;
  }

  function insertLabelFor(state: MarkdownToolbarState) {
    if (state.codeBlock) return "Cod";
    if (state.quote) return "Citat";
    if (state.table) return "Tabel";
    return "Insert";
  }

  function menuButton(event: MouseEvent, name: string, value?: string) {
    event.preventDefault();
    run(event, name, value);
  }

  $: blockLabel = blockLabelFor(active);
  $: blockActive = blockActiveFor(active);
  $: listLabel = listLabelFor(active);
  $: listActive = listActiveFor(active);
  $: insertLabel = insertLabelFor(active);
  $: insertActive = insertActiveFor(active);
</script>

<div
  class="markdown-toolbar"
  aria-label="Markdown tools"
  aria-disabled={disabled}
  inert={disabled ? true : undefined}
>
  <details class="toolbar-menu" ontoggle={closeSiblingMenus}>
    <summary class:active={blockActive} title="Bloc curent: {blockLabel}">
      <IconTypography size={17} />
      <span class="summary-label">{blockLabel}</span>
      <IconChevronDown class="chevron" size={11} />
    </summary>
    <div class="menu-panel">
      <button type="button" class:active={active.paragraph} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "paragraph")}><span class="menu-code">T</span><span>Paragraf</span></button>
      <button type="button" class:active={active.h1} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h1")}><span class="menu-code">H1</span><span>Heading 1</span></button>
      <button type="button" class:active={active.h2} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h2")}><span class="menu-code">H2</span><span>Heading 2</span></button>
      <button type="button" class:active={active.h3} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h3")}><span class="menu-code">H3</span><span>Heading 3</span></button>
      <button type="button" class:active={active.h4} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h4")}><span class="menu-code">H4</span><span>Heading 4</span></button>
      <button type="button" class:active={active.h5} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h5")}><span class="menu-code">H5</span><span>Heading 5</span></button>
      <button type="button" class:active={active.h6} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h6")}><span class="menu-code">H6</span><span>Heading 6</span></button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "downgradeHeading")}><IconTypography size={17} /><span>Micșorează heading</span></button>
    </div>
  </details>
  <span class="toolbar-separator" aria-hidden="true"></span>
  <button type="button" class:active={active.bold} aria-pressed={active.bold} title="Bold" onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "bold")}><IconBold size={17} /></button>
  <button type="button" class:active={active.italic} aria-pressed={active.italic} title="Italic" onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "italic")}><IconItalic size={17} /></button>
  <button type="button" title="Underline - necesită extensie Markdown viitoare" disabled><IconUnderline size={17} /></button>
  <button type="button" class:active={active.strike} aria-pressed={active.strike} title="Strikethrough" onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "strike")}><IconStrikethrough size={17} /></button>
  <button type="button" class:active={active.inlineCode} aria-pressed={active.inlineCode} title="Inline code" onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "inlineCode")}><IconCode size={17} /></button>
  <span class="toolbar-separator" aria-hidden="true"></span>
  <details class="toolbar-menu" ontoggle={closeSiblingMenus}>
    <summary class:active={listActive} title="Liste">
      <IconList size={17} />
      <span class="summary-label">{listLabel}</span>
      <IconChevronDown class="chevron" size={11} />
    </summary>
    <div class="menu-panel">
      <button type="button" class:active={active.bulletList} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertUnorderedList")}><IconList size={17} /> Listă simplă</button>
      <button type="button" class:active={active.orderedList} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertOrderedList")}><IconListNumbers size={17} /> Listă numerotată</button>
      <button type="button" class:active={active.taskList} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertTaskList")}><IconListCheck size={17} /> To-Do list</button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "sinkListItem")}><IconIndentIncrease size={17} /> Indent</button>
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "liftListItem")}><IconIndentDecrease size={17} /> Outdent</button>
    </div>
  </details>
  <span class="toolbar-separator" aria-hidden="true"></span>
  <details class="toolbar-menu" ontoggle={closeSiblingMenus}>
    <summary class:active={insertActive} title="Inserare">
      <IconBrackets size={17} />
      <span class="summary-label">{insertLabel}</span>
      <IconChevronDown class="chevron" size={11} />
    </summary>
    <div class="menu-panel">
      <button type="button" class:active={active.quote} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "blockquote")}><IconQuote size={17} /> Blockquote</button>
      <button type="button" class:active={active.codeBlock} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "pre")}><IconCode size={17} /> Code block</button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertHr")}><IconSeparatorHorizontal size={17} /> Separator</button>
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertHardbreak")}><span class="menu-code">↵</span> Line break</button>
    </div>
  </details>
  <details class="toolbar-menu icon-menu" ontoggle={closeSiblingMenus}>
    <summary class:active={active.table} title="Tabel">
      <IconTable size={17} />
      <IconChevronDown class="chevron" size={10} />
    </summary>
    <div class="menu-panel table-panel">
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertTable")}><IconTablePlus size={17} /> Inserează tabel</button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "addRowBefore")}><IconRowInsertTop size={17} /> Rând deasupra</button>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "addRowAfter")}><IconRowInsertBottom size={17} /> Rând dedesubt</button>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "deleteRow")}><IconRowRemove size={17} /> Șterge rând</button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "addColumnBefore")}><IconColumnInsertLeft size={17} /> Coloană la stânga</button>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "addColumnAfter")}><IconColumnInsertRight size={17} /> Coloană la dreapta</button>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "deleteColumn")}><IconColumnRemove size={17} /> Șterge coloană</button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "deleteTable")}><IconTrash size={17} /> Șterge tabel</button>
    </div>
  </details>
  <span class="toolbar-separator" aria-hidden="true"></span>
  <button type="button" class:active={active.link} aria-pressed={active.link} title="Link" onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "createLink")}><IconLink size={17} /></button>
  <button type="button" title="Imagine" onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "insertImage")}><IconPhoto size={17} /></button>
  <button type="button" title="Emoji" onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "insertEmoji")}><IconMoodSmile size={17} /></button>
</div>

<style>
  .markdown-toolbar {
    position: absolute;
    top: var(--markdown-toolbar-top, 12px);
    left: 50%;
    z-index: 3;
    display: inline-flex;
    align-items: center;
    width: max-content;
    max-width: calc(100% - 24px);
    gap: 3px;
    padding: 4px;
    border: 1px solid var(--border-3);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    box-shadow: 0 14px 34px rgba(0, 0, 0, 0.14);
    backdrop-filter: blur(10px);
    transform: translateX(-50%);
    overflow: visible;
  }

  .markdown-toolbar[aria-disabled="true"] {
    opacity: 0.62;
    pointer-events: none;
  }

  button {
    display: inline-grid;
    place-items: center;
    flex: 0 0 auto;
    width: 26px;
    height: 24px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--text-muted);
    background: transparent;
    cursor: pointer;
  }

  button:hover,
  button.active {
    color: var(--text);
    border-color: var(--brand);
    background: var(--brand-soft);
  }

  button.active {
    color: var(--brand);
  }

  button:disabled {
    color: color-mix(in srgb, var(--text-muted) 45%, transparent);
    cursor: not-allowed;
  }

  button:disabled:hover {
    border-color: transparent;
    background: transparent;
  }

  .toolbar-separator {
    flex: 0 0 auto;
    width: 1px;
    height: 18px;
    margin: 0 2px;
    background: var(--border-3);
  }

  .toolbar-menu {
    position: relative;
    flex: 0 0 auto;
  }

  .icon-menu summary {
    padding: 0 6px;
    gap: 2px;
  }

  summary {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    height: 24px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
    list-style: none;
    cursor: pointer;
    white-space: nowrap;
  }

  summary::-webkit-details-marker {
    display: none;
  }

  .toolbar-menu[open] summary,
  summary:hover,
  summary.active {
    color: var(--text);
    border-color: var(--brand);
    background: var(--brand-soft);
  }

  summary.active {
    color: var(--brand);
  }

  .summary-label {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 22px;
    line-height: 1;
  }

  :global(.chevron) {
    opacity: 0.8;
  }

  .menu-panel {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 10;
    display: grid;
    gap: 3px;
    min-width: 170px;
    padding: 6px;
    border: 1px solid var(--border-3);
    border-radius: 9px;
    background: var(--surface);
    box-shadow: 0 16px 36px rgba(0, 0, 0, 0.16);
  }

  .toolbar-menu:last-child .menu-panel {
    right: 0;
    left: auto;
  }

  .table-panel {
    min-width: 205px;
  }

  .menu-panel button {
    place-items: center start;
    align-items: center;
    justify-items: start;
    justify-content: start;
    grid-auto-flow: column;
    grid-template-columns: auto 1fr;
    width: 100%;
    height: 28px;
    padding: 0 8px;
    text-align: left;
    white-space: nowrap;
    gap: 7px;
  }

  .menu-panel button span:not(.menu-code) {
    justify-self: start;
    text-align: left;
  }

  .menu-code {
    display: inline-flex;
    min-width: 20px;
    color: var(--text-muted);
    font-weight: 800;
  }

  .menu-divider {
    height: 1px;
    margin: 4px 6px;
    background: var(--border-3);
  }
</style>
