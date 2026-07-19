<script lang="ts">
  import { onMount } from "svelte";
  import {
    IconAlignLeft,
    IconArticle,
    IconBox,
    IconBrandJavascript,
    IconCursorText,
    IconForms,
    IconHeading,
    IconLayout,
    IconLink,
    IconList,
    IconPhoto,
    IconTable,
    IconVideo,
    IconVolume2,
  } from "@tabler/icons-svelte";
  import {
    htmlPaletteGroups,
    type HtmlPaletteElement,
    type HtmlPaletteGroup,
  } from "$lib/project/html-palette";
  import { readPageComponentRegistry } from "$lib/project/io";
  import { pageComponentPaletteGroupsFromRegistry } from "$lib/page-components/registry";
  import {
    teraPaletteGroups,
  } from "$lib/tera/palette";
  import type { TeraPaletteItem } from "$lib/tera/model";
  import type { SelectionInfo, SourceGraph } from "$lib/types";

  export let selectedElement: SelectionInfo | null = null;
  export let sourceGraph: SourceGraph | null = null;
  export let loopPaletteItems: TeraPaletteItem[] = [];
  export let startElementPaletteDrag: (element: HtmlPaletteElement, event: PointerEvent) => void;
  export let startTeraPaletteDrag: (item: TeraPaletteItem, event: PointerEvent) => void;

  let componentPaletteGroups: HtmlPaletteGroup[] = [];

  onMount(() => {
    let cancelled = false;
    readPageComponentRegistry()
      .then((registry) => {
        if (!cancelled) componentPaletteGroups = pageComponentPaletteGroupsFromRegistry(registry);
      })
      .catch(() => {
        if (!cancelled) componentPaletteGroups = [];
      });
    return () => {
      cancelled = true;
    };
  });
</script>

<section class="panel-card">
  <div class="section-heading">
    <h3>Elemente HTML</h3>
    <span>{selectedElement?.tag ?? "-"}</span>
  </div>

  <div class="palette-groups">
    <section class="palette-mode component-mode" aria-label="Componente mici">
      <div class="palette-mode-heading">
        <strong>Componente mici</strong>
        <span>contract pagina</span>
      </div>

      {#each componentPaletteGroups as group}
        <section class="palette-group" aria-label={group.label}>
          <h4>{group.label}</h4>
          <div class="palette-grid">
            {#each group.elements as element}
              <button
                class="palette-card component-card"
                type="button"
                aria-label={`Adaugă ${element.label}`}
                onpointerdown={(event) => startElementPaletteDrag(element, event)}
              >
                <span class="palette-icon component-icon">
                  <IconBrandJavascript size={15} stroke={1.9} />
                </span>
                <span class="palette-copy">
                  <strong>{element.label}</strong>
                  <small>{element.componentKind?.toUpperCase() ?? "CSS"} · {element.description}</small>
                </span>
              </button>
            {/each}
          </div>
        </section>
      {/each}
    </section>

    <section class="palette-mode" aria-label="Elemente HTML">
      <div class="palette-mode-heading">
        <strong>HTML</strong>
        <span>elemente vizuale</span>
      </div>

    {#each htmlPaletteGroups as group}
      <section class="palette-group" aria-label={group.label}>
        <h4>{group.label}</h4>
        <div class="palette-grid">
          {#each group.elements as element}
            <button
              class="palette-card"
              type="button"
              aria-label={`Adaugă ${element.label}`}
              onpointerdown={(event) => startElementPaletteDrag(element, event)}
            >
              <span class="palette-icon">
                {#if element.tag === "section" || element.tag === "main" || element.tag === "header" || element.tag === "footer" || element.tag === "nav" || element.tag === "aside" || element.tag === "figure"}<IconLayout size={15} stroke={1.9} />
                {:else if element.tag === "article"}<IconArticle size={15} stroke={1.9} />
                {:else if element.tag === "h1" || element.tag === "h2" || element.tag === "h3" || element.tag === "h4" || element.tag === "h5" || element.tag === "h6"}<IconHeading size={15} stroke={1.9} />
                {:else if element.tag === "ul" || element.tag === "ol" || element.tag === "li" || element.tag === "dl" || element.tag === "dt" || element.tag === "dd"}<IconList size={15} stroke={1.9} />
                {:else if element.tag === "img" || element.tag === "picture" || element.tag === "source"}<IconPhoto size={15} stroke={1.9} />
                {:else if element.tag === "video" || element.tag === "iframe"}<IconVideo size={15} stroke={1.9} />
                {:else if element.tag === "audio"}<IconVolume2 size={15} stroke={1.9} />
                {:else if element.tag === "a"}<IconLink size={15} stroke={1.9} />
                {:else if element.tag === "button"}<IconCursorText size={15} stroke={1.9} />
                {:else if element.tag === "form" || element.tag === "input" || element.tag === "textarea" || element.tag === "select" || element.tag === "option" || element.tag === "fieldset" || element.tag === "legend"}<IconForms size={15} stroke={1.9} />
                {:else if element.tag === "table" || element.tag === "thead" || element.tag === "tbody" || element.tag === "tfoot" || element.tag === "tr" || element.tag === "th" || element.tag === "td" || element.tag === "caption"}<IconTable size={15} stroke={1.9} />
                {:else if element.tag === "p" || element.tag === "span" || element.tag === "blockquote" || element.tag === "pre" || element.tag === "code" || element.tag === "strong" || element.tag === "em" || element.tag === "small" || element.tag === "label" || element.tag === "figcaption"}<IconAlignLeft size={15} stroke={1.9} />
                {:else}<IconBox size={15} stroke={1.9} />
                {/if}
              </span>
              <span class="palette-copy">
                <strong>{element.label}</strong>
                <small>&lt;{element.tag}&gt; · {element.description}</small>
              </span>
            </button>
          {/each}
        </div>
      </section>
    {/each}
    </section>

    <section class="palette-mode tera-mode" aria-label="Structură Tera">
      <div class="palette-mode-heading">
        <strong>Tera</strong>
        <span>template, block, include</span>
      </div>

      {#each teraPaletteGroups(sourceGraph, loopPaletteItems) as group}
        <section class="palette-group" aria-label={group.label}>
          <h4>{group.label}</h4>
          <div class="palette-grid">
            {#each group.items as item}
              <button
                class="palette-card tera-card"
                type="button"
                aria-label={`Adaugă ${item.label}`}
                onpointerdown={(event) => startTeraPaletteDrag(item, event)}
              >
                <span class="palette-icon tera-icon">
                  <IconLayout size={15} stroke={1.9} />
                </span>
                <span class="palette-copy">
                  <strong>{item.label}</strong>
                  <small>{item.kind} · {item.description}</small>
                </span>
              </button>
            {/each}
          </div>
        </section>
      {/each}
    </section>
  </div>
</section>

<style>
  .panel-card {
    padding: 9px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .section-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 8px;
  }

  .section-heading h3 {
    margin: 0;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 900;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .section-heading span {
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 700;
  }

  .palette-groups {
    display: grid;
    gap: 10px;
  }

  .palette-mode {
    display: grid;
    gap: 8px;
  }

  .palette-mode + .palette-mode {
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }

  .palette-mode-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .palette-mode-heading strong {
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 900;
  }

  .palette-mode-heading span {
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 800;
    text-transform: uppercase;
  }

  .palette-group {
    display: grid;
    gap: 6px;
  }

  .palette-group h4 {
    margin: 0;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 900;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .palette-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .palette-card {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr);
    align-items: center;
    gap: 7px;
    min-height: 58px;
    padding: 7px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    color: var(--text);
    text-align: left;
    background: var(--surface-4);
    cursor: grab;
    transition: border-color 120ms ease, background 120ms ease, color 120ms ease;
  }

  .palette-card:hover {
    border-color: var(--brand);
    background: var(--brand-soft);
  }

  .palette-card.tera-card {
    border-color: rgba(124, 58, 237, 0.24);
  }

  .palette-card.tera-card:hover {
    border-color: rgba(124, 58, 237, 0.72);
    background: rgba(124, 58, 237, 0.08);
  }

  .palette-card.component-card {
    border-color: rgba(29, 127, 106, 0.28);
  }

  .palette-card.component-card:hover {
    border-color: rgba(29, 127, 106, 0.72);
    background: rgba(29, 127, 106, 0.08);
  }

  .palette-card:active {
    cursor: grabbing;
  }

  .palette-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    color: var(--brand-strong);
    background: var(--surface-2);
  }

  .palette-icon.tera-icon {
    color: #6d28d9;
    border-color: rgba(124, 58, 237, 0.28);
    background: rgba(124, 58, 237, 0.07);
  }

  .palette-icon.component-icon {
    color: #13745f;
    border-color: rgba(29, 127, 106, 0.28);
    background: rgba(29, 127, 106, 0.08);
  }

  .palette-icon :global(svg) {
    display: block;
  }

  .palette-copy {
    display: grid;
    gap: 3px;
    min-width: 0;
  }

  .palette-copy strong,
  .palette-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .palette-copy strong {
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 800;
  }

  .palette-copy small {
    color: var(--text-muted);
    font-size: 10px;
    line-height: 1.2;
  }

  :global(body.element-palette-dragging),
  :global(body.element-palette-dragging *),
  :global(body.tera-palette-dragging),
  :global(body.tera-palette-dragging *) {
    cursor: grabbing !important;
  }
</style>
