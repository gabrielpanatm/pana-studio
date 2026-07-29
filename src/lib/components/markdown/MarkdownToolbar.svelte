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
    IconCornerDownLeft,
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
  import {
    l10n,
    legacyTranslator,
    localeRevision,
  } from "$lib/i18n/runtime.svelte";

  $: t = legacyTranslator($localeRevision);

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
    if (state.codeBlock) return t("markdown-code");
    if (state.quote) return t("markdown-quote");
    if (state.paragraph) return t("markdown-text");
    return t("markdown-block");
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
    if (state.taskList) return t("markdown-task-list-short");
    if (state.orderedList) return t("markdown-ordered-short");
    if (state.bulletList) return t("markdown-bullet-short");
    return t("markdown-lists");
  }

  function insertActiveFor(state: MarkdownToolbarState) {
    return state.codeBlock || state.quote || state.table;
  }

  function insertLabelFor(state: MarkdownToolbarState) {
    if (state.codeBlock) return t("markdown-code");
    if (state.quote) return t("markdown-quote");
    if (state.table) return t("markdown-table");
    return t("markdown-insert");
  }

  function menuButton(event: MouseEvent, name: string, value?: string) {
    event.preventDefault();
    run(event, name, value);
  }

  let blockLabel: string;
  let blockActive: boolean;
  let listLabel: string;
  let listActive: boolean;
  let insertLabel: string;
  let insertActive: boolean;

  $: {
    l10n.revision;
    blockLabel = blockLabelFor(active);
    blockActive = blockActiveFor(active);
    listLabel = listLabelFor(active);
    listActive = listActiveFor(active);
    insertLabel = insertLabelFor(active);
    insertActive = insertActiveFor(active);
  }
</script>

<div
  class="markdown-toolbar"
  aria-label={t("markdown-tools")}
  aria-disabled={disabled}
  inert={disabled ? true : undefined}
>
  <details class="toolbar-menu" ontoggle={closeSiblingMenus}>
    <summary class:active={blockActive} title={t("markdown-current-block", { block: blockLabel })}>
      <IconTypography size={17} />
      <span class="summary-label">{blockLabel}</span>
      <IconChevronDown class="chevron" size={11} />
    </summary>
    <div class="menu-panel">
      <button type="button" class:active={active.paragraph} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "paragraph")}><span class="menu-code">T</span><span>{t("markdown-paragraph")}</span></button>
      <button type="button" class:active={active.h1} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h1")}><span class="menu-code">H1</span><span>{t("markdown-heading", { level: 1 })}</span></button>
      <button type="button" class:active={active.h2} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h2")}><span class="menu-code">H2</span><span>{t("markdown-heading", { level: 2 })}</span></button>
      <button type="button" class:active={active.h3} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h3")}><span class="menu-code">H3</span><span>{t("markdown-heading", { level: 3 })}</span></button>
      <button type="button" class:active={active.h4} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h4")}><span class="menu-code">H4</span><span>{t("markdown-heading", { level: 4 })}</span></button>
      <button type="button" class:active={active.h5} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h5")}><span class="menu-code">H5</span><span>{t("markdown-heading", { level: 5 })}</span></button>
      <button type="button" class:active={active.h6} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "h6")}><span class="menu-code">H6</span><span>{t("markdown-heading", { level: 6 })}</span></button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "downgradeHeading")}><IconTypography size={17} /><span>{t("markdown-demote-heading")}</span></button>
    </div>
  </details>
  <span class="toolbar-separator" aria-hidden="true"></span>
  <button type="button" class:active={active.bold} aria-pressed={active.bold} title={t("markdown-bold")} onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "bold")}><IconBold size={17} /></button>
  <button type="button" class:active={active.italic} aria-pressed={active.italic} title={t("markdown-italic")} onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "italic")}><IconItalic size={17} /></button>
  <button type="button" title={t("markdown-underline-future")} disabled><IconUnderline size={17} /></button>
  <button type="button" class:active={active.strike} aria-pressed={active.strike} title={t("markdown-strikethrough")} onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "strike")}><IconStrikethrough size={17} /></button>
  <button type="button" class:active={active.inlineCode} aria-pressed={active.inlineCode} title={t("markdown-inline-code")} onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "inlineCode")}><IconCode size={17} /></button>
  <span class="toolbar-separator" aria-hidden="true"></span>
  <details class="toolbar-menu" ontoggle={closeSiblingMenus}>
    <summary class:active={listActive} title={t("markdown-lists")}>
      <IconList size={17} />
      <span class="summary-label">{listLabel}</span>
      <IconChevronDown class="chevron" size={11} />
    </summary>
    <div class="menu-panel">
      <button type="button" class:active={active.bulletList} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertUnorderedList")}><IconList size={17} /> {t("markdown-bullet-list")}</button>
      <button type="button" class:active={active.orderedList} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertOrderedList")}><IconListNumbers size={17} /> {t("markdown-ordered-list")}</button>
      <button type="button" class:active={active.taskList} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertTaskList")}><IconListCheck size={17} /> {t("markdown-task-list")}</button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "sinkListItem")}><IconIndentIncrease size={17} /> {t("markdown-indent")}</button>
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "liftListItem")}><IconIndentDecrease size={17} /> {t("markdown-outdent")}</button>
    </div>
  </details>
  <span class="toolbar-separator" aria-hidden="true"></span>
  <details class="toolbar-menu" ontoggle={closeSiblingMenus}>
    <summary class:active={insertActive} title={t("markdown-insert")}>
      <IconBrackets size={17} />
      <span class="summary-label">{insertLabel}</span>
      <IconChevronDown class="chevron" size={11} />
    </summary>
    <div class="menu-panel">
      <button type="button" class:active={active.quote} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "blockquote")}><IconQuote size={17} /> {t("markdown-blockquote")}</button>
      <button type="button" class:active={active.codeBlock} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "formatBlock", "pre")}><IconCode size={17} /> {t("markdown-code-block")}</button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertHr")}><IconSeparatorHorizontal size={17} /> {t("markdown-separator")}</button>
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertHardbreak")}><IconCornerDownLeft size={17} /> {t("markdown-line-break")}</button>
    </div>
  </details>
  <details class="toolbar-menu icon-menu" ontoggle={closeSiblingMenus}>
    <summary class:active={active.table} title={t("markdown-table")}>
      <IconTable size={17} />
      <IconChevronDown class="chevron" size={10} />
    </summary>
    <div class="menu-panel table-panel">
      <button type="button" onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "insertTable")}><IconTablePlus size={17} /> {t("markdown-insert-table")}</button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "addRowBefore")}><IconRowInsertTop size={17} /> {t("markdown-row-above")}</button>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "addRowAfter")}><IconRowInsertBottom size={17} /> {t("markdown-row-below")}</button>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "deleteRow")}><IconRowRemove size={17} /> {t("markdown-delete-row")}</button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "addColumnBefore")}><IconColumnInsertLeft size={17} /> {t("markdown-column-left")}</button>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "addColumnAfter")}><IconColumnInsertRight size={17} /> {t("markdown-column-right")}</button>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "deleteColumn")}><IconColumnRemove size={17} /> {t("markdown-delete-column")}</button>
      <span class="menu-divider" aria-hidden="true"></span>
      <button type="button" disabled={!active.table} onmousedown={(event) => event.preventDefault()} onclick={(event) => menuButton(event, "deleteTable")}><IconTrash size={17} /> {t("markdown-delete-table")}</button>
    </div>
  </details>
  <span class="toolbar-separator" aria-hidden="true"></span>
  <button type="button" class:active={active.link} aria-pressed={active.link} title={t("markdown-link")} onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "createLink")}><IconLink size={17} /></button>
  <button type="button" title={t("markdown-image")} onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "insertImage")}><IconPhoto size={17} /></button>
  <button type="button" title={t("markdown-emoji")} onmousedown={(event) => event.preventDefault()} onclick={(event) => run(event, "insertEmoji")}><IconMoodSmile size={17} /></button>
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
