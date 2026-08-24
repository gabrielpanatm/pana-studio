<script lang="ts">
  import {
    IconAlignLeft,
    IconAlertTriangle,
    IconArticle,
    IconBox,
    IconBraces,
    IconBrush,
    IconCode,
    IconComponents,
    IconCursorText,
    IconDatabase,
    IconForms,
    IconGripVertical,
    IconHeading,
    IconLayout,
    IconLayoutGrid,
    IconLayoutNavbar,
    IconLayoutSidebarRightExpand,
    IconLink,
    IconList,
    IconListDetails,
    IconMap,
    IconMenu2,
    IconMessageCircle,
    IconMusic,
    IconNumber123,
    IconPhoto,
    IconQuote,
    IconSearch,
    IconSlideshow,
    IconTable,
    IconTemplate,
    IconVideo,
    IconWorldWww,
    IconX,
  } from "@tabler/icons-svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import { readInsertCatalog } from "$lib/blocks/io";
  import type {
    InsertCatalogCategory,
    InsertCatalogContext,
    InsertCatalogItem,
    InsertCatalogSnapshot,
  } from "$lib/blocks/contracts";
  import { errorMessage } from "$lib/util";

  export let projectRoot = "";
  export let runtimeSessionId = "";
  export let workspaceRevision = 0;
  export let context: InsertCatalogContext;
  export let closeLabel = "Închide catalogul";
  export let close: () => void;
  export let startDrag: (
    item: InsertCatalogItem,
    snapshot: InsertCatalogSnapshot,
    event: PointerEvent,
  ) => void;

  type CatalogSectionIcon = "structure" | "text" | "list" | "form" | "table" | "block" | "component" | "tera" | "dynamic";
  type CatalogItemIcon = "box" | "layout" | "article" | "heading" | "text" | "quote" | "code" | "list" | "link" | "action" | "form" | "table" | "photo" | "video" | "audio" | "web" | "canvas" | "map" | "template" | "block" | "accordion" | "counter" | "dialog" | "nav-menu" | "offcanvas" | "slider" | "tabs" | "component" | "tera" | "dynamic";
  type CatalogSection = {
    id: string;
    label: string;
    description: string;
    icon: CatalogSectionIcon;
    items: InsertCatalogItem[];
  };

  const categories: Array<{ id: InsertCatalogCategory; label: string }> = [
    { id: "html", label: "HTML" },
    { id: "block", label: "Blocuri" },
    { id: "component", label: "Componente" },
    { id: "tera", label: "Tera" },
    { id: "dynamicWidget", label: "Dinamic" },
  ];

  const htmlSectionDefinitions: Array<Omit<CatalogSection, "items"> & { tags: string[] }> = [
    {
      id: "structure",
      label: "Structură",
      description: "Containerele și reperele semantice ale paginii.",
      icon: "structure",
      tags: ["div", "section", "article", "main", "header", "footer", "nav", "aside", "address", "hgroup", "figure", "figcaption", "search", "hr"],
    },
    {
      id: "text",
      label: "Text și titluri",
      description: "Conținut editorial, titluri și marcaje inline.",
      icon: "text",
      tags: ["p", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "pre", "code", "strong", "em", "small", "mark"],
    },
    {
      id: "inline",
      label: "Text în linie",
      description: "Marcaje semantice folosite în interiorul textului.",
      icon: "text",
      tags: ["span", "q", "cite", "abbr", "dfn", "time", "data", "sub", "sup", "kbd", "samp", "var", "b", "i", "u", "s", "bdi", "bdo", "br", "wbr", "ins", "del", "ruby", "rt", "rp"],
    },
    {
      id: "lists",
      label: "Liste",
      description: "Liste ordonate, neordonate și descriptive.",
      icon: "list",
      tags: ["ul", "ol", "li", "dl", "dt", "dd", "menu"],
    },
    {
      id: "media",
      label: "Media",
      description: "Imagini, conținut audio-video și sursele lor.",
      icon: "structure",
      tags: ["img", "picture", "video", "audio", "source", "track"],
    },
    {
      id: "embedded",
      label: "Conținut încorporat",
      description: "Documente, suprafețe și resurse externe încorporate.",
      icon: "structure",
      tags: ["iframe", "canvas", "object", "embed", "map", "area"],
    },
    {
      id: "interactive",
      label: "Interactiv",
      description: "Linkuri, acțiuni și elemente interactive native.",
      icon: "form",
      tags: ["a", "button", "details", "summary", "dialog"],
    },
    {
      id: "forms",
      label: "Formulare",
      description: "Câmpuri, controale și grupuri de formular.",
      icon: "form",
      tags: ["form", "label", "input", "textarea", "select", "option", "optgroup", "datalist", "fieldset", "legend", "output", "progress", "meter"],
    },
    {
      id: "tables",
      label: "Tabele",
      description: "Structuri semantice pentru date tabelare.",
      icon: "table",
      tags: ["table", "caption", "colgroup", "col", "thead", "tbody", "tfoot", "tr", "th", "td"],
    },
    {
      id: "advanced",
      label: "Avansat",
      description: "Primitive HTML pentru compoziție și componente web.",
      icon: "structure",
      tags: ["template", "slot"],
    },
  ];

  let activeCategory: InsertCatalogCategory = "html";
  let activeHtmlSection = "structure";
  let query = "";
  let snapshot: InsertCatalogSnapshot | null = null;
  let loading = false;
  let loadError = "";
  let requestedKey = "";

  $: requestKey = projectRoot && runtimeSessionId
    ? [
        projectRoot,
        runtimeSessionId,
        workspaceRevision,
        context.activeDocumentPath ?? "",
        context.activeTemplatePath ?? "",
        context.activePagePath ?? "",
        context.canvasPreviewRevision ?? "",
        context.canvasAvailable ? "1" : "0",
        context.targetSourceId ?? "",
        context.targetTag ?? "",
      ].join("\u0000")
    : "";
  $: if (requestKey && requestKey !== requestedKey) {
    requestedKey = requestKey;
    void loadCatalog(requestKey);
  }

  $: stale = Boolean(snapshot) && (
    snapshot?.projectRoot !== projectRoot
    || snapshot?.runtimeSessionId !== runtimeSessionId
    || snapshot?.workspaceRevision !== workspaceRevision
    || !sameContext(snapshot?.context, context)
  );
  $: activeGroup = snapshot?.groups.find((group) => group.category === activeCategory) ?? null;
  $: normalizedQuery = query.trim().toLocaleLowerCase();
  $: visibleItems = (activeGroup?.items ?? []).filter((item) => {
    if (!normalizedQuery) return true;
    return [item.label, item.description, item.origin, item.category]
      .join(" ")
      .toLocaleLowerCase()
      .includes(normalizedQuery);
  });
  $: presentedSections = buildPresentedSections(activeCategory, activeGroup?.label ?? "", visibleItems);
  $: if (
    activeCategory === "html"
    && presentedSections.length > 0
    && !presentedSections.some((section) => section.id === activeHtmlSection)
  ) activeHtmlSection = presentedSections[0].id;
  $: renderedSections = activeCategory === "html"
    ? presentedSections.filter((section) => section.id === activeHtmlSection)
    : presentedSections;
  $: blockedCount = visibleItems.filter((item) => !item.capabilities.canDrag).length;

  async function loadCatalog(key: string) {
    loading = true;
    loadError = "";
    try {
      const next = await readInsertCatalog(
        {
          expectedProjectRoot: projectRoot,
          expectedSessionId: runtimeSessionId,
        },
        workspaceRevision,
        context,
      );
      if (requestedKey !== key) return;
      snapshot = next;
      const hasActiveCategory = next.groups.some(
        (group) => group.category === activeCategory && group.items.length > 0,
      );
      if (!hasActiveCategory) {
        activeCategory = next.groups.find((group) => group.items.length > 0)?.category ?? "html";
      }
    } catch (cause) {
      if (requestedKey === key) loadError = errorMessage(cause);
    } finally {
      if (requestedKey === key) loading = false;
    }
  }

  function itemCode(item: InsertCatalogItem) {
    const payload = item.payload;
    if (payload.kind === "html") return `<${payload.tag}>`;
    if (payload.kind === "block") return payload.blockId;
    if (payload.kind === "component") return payload.teraKind;
    if (payload.kind === "dynamicWidget") return payload.providerId;
    return payload.teraKind;
  }

  function buildPresentedSections(
    category: InsertCatalogCategory,
    groupLabel: string,
    items: InsertCatalogItem[],
  ): CatalogSection[] {
    if (category === "html") {
      const knownTags = new Set(htmlSectionDefinitions.flatMap((section) => section.tags));
      const sections = htmlSectionDefinitions
        .map((section) => ({
          id: section.id,
          label: section.label,
          description: section.description,
          icon: section.icon,
          items: items.filter((item) => item.payload.kind === "html" && section.tags.includes(item.payload.tag)),
        }))
        .filter((section) => section.items.length > 0);
      const remaining = items.filter((item) => item.payload.kind !== "html" || !knownTags.has(item.payload.tag));
      if (remaining.length > 0) {
        sections.push({
          id: "other",
          label: "Alte elemente",
          description: "Elemente HTML disponibile în catalogul proiectului.",
          icon: "structure",
          items: remaining,
        });
      }
      return sections;
    }

    const icon: CatalogSectionIcon = category === "block"
      ? "block"
      : category === "component"
        ? "component"
        : category === "tera"
          ? "tera"
          : "dynamic";
    return items.length > 0
      ? [{ id: category, label: groupLabel, description: sectionDescription(category), icon, items }]
      : [];
  }

  function sectionDescription(category: InsertCatalogCategory) {
    if (category === "block") return "Piese native gata de inserat, cu structură și comportament păstrate de Rust.";
    if (category === "component") return "Componente Tera descoperite în proiect și în tema activă.";
    if (category === "tera") return "Construcții Tera pentru compoziție, logică și reutilizare.";
    return "Widgeturi dinamice conectate la conținutul și modelele proiectului.";
  }

  function itemIcon(item: InsertCatalogItem, sectionIcon: CatalogSectionIcon): CatalogItemIcon {
    if (item.payload.kind !== "html") {
      if (item.payload.kind === "block") {
        if (["accordion", "counter", "dialog", "nav-menu", "offcanvas", "slider", "tabs"].includes(item.payload.blockId)) {
          return item.payload.blockId as CatalogItemIcon;
        }
        return "block";
      }
      if (sectionIcon === "component") return "component";
      if (sectionIcon === "dynamic") return "dynamic";
      return "tera";
    }
    const tag = item.payload.tag;
    if (tag === "div") return "box";
    if (["section", "main", "header", "footer", "nav", "aside", "address", "hgroup", "figure", "figcaption", "search", "hr"].includes(tag)) return "layout";
    if (tag === "article") return "article";
    if (/^h[1-6]$/.test(tag)) return "heading";
    if (["blockquote", "q", "cite"].includes(tag)) return "quote";
    if (["pre", "code", "kbd", "samp", "var"].includes(tag)) return "code";
    if (["ul", "ol", "li", "dl", "dt", "dd", "menu"].includes(tag)) return "list";
    if (["img", "picture"].includes(tag)) return "photo";
    if (tag === "video") return "video";
    if (["audio", "source", "track"].includes(tag)) return "audio";
    if (["iframe", "object", "embed"].includes(tag)) return "web";
    if (tag === "canvas") return "canvas";
    if (["map", "area"].includes(tag)) return "map";
    if (["template", "slot"].includes(tag)) return "template";
    if (tag === "a") return "link";
    if (["button", "details", "summary", "dialog"].includes(tag)) return "action";
    if (["form", "label", "input", "textarea", "select", "option", "optgroup", "datalist", "fieldset", "legend", "output", "progress", "meter"].includes(tag)) return "form";
    if (["table", "caption", "colgroup", "col", "thead", "tbody", "tfoot", "tr", "th", "td"].includes(tag)) return "table";
    return "text";
  }

  function reasonLabel(code: string | null) {
    if (code === "insert_catalog_canvas_unavailable") return "Canvas-ul activ nu este disponibil.";
    if (code === "insert_catalog_document_not_insertable") return "Documentul activ nu acceptă inserare vizuală.";
    if (code === "insert_catalog_block_not_insertable") return "Blocul nu permite inserare.";
    if (code === "insert_catalog_component_inactive") return "Componenta nu este activă sau este umbrită.";
    if (code === "insert_catalog_macro_requires_arguments") return "Macrocomanda cere argumente explicite.";
    if (code === "insert_catalog_listing_nested_loop") return "Listing-ul nu poate fi inserat în interiorul altei bucle.";
    return code ? `Inserare blocată: ${code}` : "Inserare indisponibilă.";
  }

  function sameContext(left: InsertCatalogContext | undefined, right: InsertCatalogContext) {
    return Boolean(left)
      && left?.activeDocumentPath === right.activeDocumentPath
      && left?.activeTemplatePath === right.activeTemplatePath
      && left?.activePagePath === right.activePagePath
      && left?.canvasPreviewRevision === right.canvasPreviewRevision
      && left?.canvasAvailable === right.canvasAvailable
      && left?.targetSourceId === right.targetSourceId
      && left?.targetTag === right.targetTag;
  }
</script>

<section class="insert-catalog" aria-label="Catalog de inserare">
  <div class="catalog-toolbar">
    <div class="catalog-primary-actions">
      <label class="catalog-search">
        <IconSearch size={14} stroke={1.9} />
        <span class="sr-only">Caută în catalog</span>
        <input bind:value={query} type="search" placeholder="Caută în catalog…" />
      </label>
      <button
        type="button"
        class="ui-icon-button compact catalog-close"
        title={closeLabel}
        aria-label={closeLabel}
        onclick={close}
      ><IconX size={15} stroke={1.9} /></button>
    </div>

    <div class="catalog-tabs" role="tablist" aria-label="Categorii de inserare">
      {#each categories as category (category.id)}
        <button
          type="button"
          role="tab"
          class:active={activeCategory === category.id}
          aria-selected={activeCategory === category.id}
          onclick={() => { activeCategory = category.id; }}
        >
          <span class="category-icon" aria-hidden="true">
            {#if category.id === "html"}<IconCode size={14} />
            {:else if category.id === "block"}<IconBox size={14} />
            {:else if category.id === "component"}<IconComponents size={14} />
            {:else if category.id === "tera"}<IconBraces size={14} />
            {:else}<IconDatabase size={14} />{/if}
          </span>
          <span>{category.label}</span>
        </button>
      {/each}
    </div>

    {#if activeCategory === "html" && presentedSections.length > 0}
      <div class="html-category-filter">
        <IconLayoutGrid size={14} stroke={1.9} />
        <span class="sr-only">Categorie de elemente HTML</span>
        <SelectControl size="toolbar" value={activeHtmlSection} options={presentedSections.map((section) => ({ value: section.id, label: `${section.label} · ${section.items.length}` }))} ariaLabel="Categorie de elemente HTML" onchange={(value) => { activeHtmlSection = value; }} />
      </div>
    {/if}
  </div>

  {#if loading && !snapshot}
    <div class="catalog-scroll"><div class="catalog-state" role="status">Se proiectează catalogul din ProjectModel…</div></div>
  {:else if loadError}
    <div class="catalog-scroll">
      <div class="catalog-state error" role="alert">
        <IconAlertTriangle size={15} />
        <span><strong>Catalog indisponibil</strong>{loadError}</span>
      </div>
    </div>
  {:else if stale}
    <div class="catalog-scroll"><div class="catalog-state" role="status">Catalogul vechi a fost invalidat. Se încarcă revizia {workspaceRevision}…</div></div>
  {:else if snapshot && activeGroup}
    <div class="catalog-scroll">
      {#if visibleItems.length === 0}
        <div class="catalog-state">{query.trim() ? "Niciun rezultat pentru această căutare." : "Nu există resurse compatibile în contextul activ."}</div>
      {:else}
        <div class="catalog-sections">
          {#each renderedSections as section (section.id)}
            <div class="catalog-grid" aria-label={section.label} title={section.label}>
                {#each section.items as item (item.id)}
                  <button
                    type="button"
                    class="catalog-item ui-entity-selectable"
                    class:blocked={!item.capabilities.canDrag}
                    disabled={!item.capabilities.canDrag}
                    aria-label={`Trage ${item.label} în Canvas`}
                    title={item.capabilities.canDrag ? item.description : reasonLabel(item.capabilities.reasonCode)}
                    onpointerdown={(event) => snapshot && startDrag(item, snapshot, event)}
                  >
                    <span class="item-icon" aria-hidden="true">
                      {#if itemIcon(item, section.icon) === "box"}<IconBox size={16} />
                      {:else if itemIcon(item, section.icon) === "layout"}<IconLayout size={16} />
                      {:else if itemIcon(item, section.icon) === "article"}<IconArticle size={16} />
                      {:else if itemIcon(item, section.icon) === "heading"}<IconHeading size={16} />
                      {:else if itemIcon(item, section.icon) === "text"}<IconAlignLeft size={16} />
                      {:else if itemIcon(item, section.icon) === "quote"}<IconQuote size={16} />
                      {:else if itemIcon(item, section.icon) === "code"}<IconCode size={16} />
                      {:else if itemIcon(item, section.icon) === "list"}<IconList size={16} />
                      {:else if itemIcon(item, section.icon) === "link"}<IconLink size={16} />
                      {:else if itemIcon(item, section.icon) === "action"}<IconCursorText size={16} />
                      {:else if itemIcon(item, section.icon) === "form"}<IconForms size={16} />
                      {:else if itemIcon(item, section.icon) === "table"}<IconTable size={16} />
                      {:else if itemIcon(item, section.icon) === "photo"}<IconPhoto size={16} />
                      {:else if itemIcon(item, section.icon) === "video"}<IconVideo size={16} />
                      {:else if itemIcon(item, section.icon) === "audio"}<IconMusic size={16} />
                      {:else if itemIcon(item, section.icon) === "web"}<IconWorldWww size={16} />
                      {:else if itemIcon(item, section.icon) === "canvas"}<IconBrush size={16} />
                      {:else if itemIcon(item, section.icon) === "map"}<IconMap size={16} />
                      {:else if itemIcon(item, section.icon) === "template"}<IconTemplate size={16} />
                      {:else if itemIcon(item, section.icon) === "accordion"}<IconListDetails size={16} />
                      {:else if itemIcon(item, section.icon) === "counter"}<IconNumber123 size={16} />
                      {:else if itemIcon(item, section.icon) === "dialog"}<IconMessageCircle size={16} />
                      {:else if itemIcon(item, section.icon) === "nav-menu"}<IconMenu2 size={16} />
                      {:else if itemIcon(item, section.icon) === "offcanvas"}<IconLayoutSidebarRightExpand size={16} />
                      {:else if itemIcon(item, section.icon) === "slider"}<IconSlideshow size={16} />
                      {:else if itemIcon(item, section.icon) === "tabs"}<IconLayoutNavbar size={16} />
                      {:else if itemIcon(item, section.icon) === "block"}<IconBox size={16} />
                      {:else if itemIcon(item, section.icon) === "component"}<IconComponents size={16} />
                      {:else if itemIcon(item, section.icon) === "dynamic"}<IconDatabase size={16} />
                      {:else}<IconBraces size={16} />{/if}
                    </span>
                    <span class="item-copy">
                      <strong>{item.label}</strong>
                      <small>{item.description}</small>
                      <span class="item-meta"><code>{itemCode(item)}</code></span>
                    </span>
                    <span class="drag-handle" aria-hidden="true"><IconGripVertical size={15} /></span>
                  </button>
                {/each}
              </div>
          {/each}
        </div>
        {#if blockedCount > 0}
          <p class="blocked-note">{blockedCount} {blockedCount === 1 ? "resursă incompatibilă are" : "resurse incompatibile au"} motivul disponibil la hover.</p>
        {/if}
      {/if}
    </div>
  {:else}
    <div class="catalog-scroll"><div class="catalog-state">Catalogul nu conține această categorie.</div></div>
  {/if}
</section>

<style>
  .insert-catalog { display: flex; flex: 1 1 auto; flex-direction: column; gap: 9px; width: 100%; height: 100%; min-height: 0; overflow: hidden; }
  .catalog-toolbar { display: grid; flex: 0 0 auto; gap: 7px; }
  .catalog-primary-actions { display: grid; grid-template-columns: minmax(0, 1fr) 30px; align-items: center; gap: 5px; }
  .catalog-close { width: 30px; min-width: 30px; height: 30px; min-height: 30px; border-color: var(--border-subtle); color: var(--text-muted); background: var(--material-control); box-shadow: var(--shadow-control); }
  .catalog-close:hover:not(:disabled) { border-color: color-mix(in srgb, var(--danger) 28%, var(--border-strong)); color: var(--danger); background: color-mix(in srgb, var(--danger) 7%, var(--material-control-hover)); box-shadow: var(--shadow-control-hover); }
  .catalog-search { display: flex; align-items: center; gap: 7px; min-height: 32px; padding: 0 9px; border: 1px solid var(--border); border-radius: var(--radius-control); background: var(--material-inset); color: var(--text-muted); box-shadow: var(--shadow-inset); }
  .catalog-search:focus-within { border-color: var(--brand); box-shadow: 0 0 0 2px color-mix(in srgb, var(--brand) 14%, transparent); }
  .catalog-search input { min-width: 0; width: 100%; border: 0; outline: 0; background: transparent; color: var(--text); font: inherit; }
  .catalog-tabs { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 4px; }
  .catalog-tabs button { display: grid; grid-template-columns: 18px minmax(0, 1fr); align-items: center; gap: 5px; min-width: 0; min-height: 29px; padding: 4px 7px; border: 1px solid var(--border); border-radius: var(--radius-control); background: var(--material-control); color: var(--text-muted); box-shadow: var(--shadow-control); font: inherit; font-size: 11px; text-align: left; cursor: pointer; }
  .catalog-tabs button:hover:not(:disabled):not(.active) { border-color: var(--border-strong); background: var(--material-control-hover); box-shadow: var(--shadow-control-hover); }
  .catalog-tabs button.active { border-color: color-mix(in srgb, var(--brand) 52%, var(--border)); background: color-mix(in srgb, var(--brand) 8%, var(--material-control-selected)); color: var(--brand-strong); box-shadow: var(--shadow-pressed), inset 0 1px 0 color-mix(in srgb, var(--brand) 10%, var(--skeuo-edge-highlight)); }
  .category-icon { display: inline-flex; align-items: center; justify-content: center; }
  .catalog-tabs button > span:nth-child(2) { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .html-category-filter { display: grid; grid-template-columns: 18px minmax(0, 1fr); align-items: center; gap: 5px; min-height: 30px; padding: 0 7px; border: 1px solid var(--border); border-radius: var(--radius-control); background: var(--material-control); color: var(--brand-strong); box-shadow: var(--shadow-control); }
  .html-category-filter :global(.select-control-root) { min-width: 0; width: 100%; }
  .html-category-filter:focus-within { border-color: color-mix(in srgb, var(--brand) 52%, var(--border)); box-shadow: 0 0 0 2px color-mix(in srgb, var(--brand) 14%, transparent), var(--shadow-control); }
  .catalog-scroll { flex: 1 1 auto; min-height: 0; padding: 6px; border: 1px solid var(--border); border-radius: var(--radius-control); overflow: auto; overscroll-behavior: contain; scrollbar-gutter: stable; background: var(--material-inset); box-shadow: var(--shadow-inset); }
  .catalog-sections { display: grid; gap: 5px; }
  .catalog-grid { display: grid; grid-template-columns: minmax(0, 1fr); gap: 5px; }
  .catalog-item { --ui-entity-background: var(--material-control); --ui-entity-border-color: var(--border); --ui-entity-color: var(--text); --ui-entity-shadow: var(--shadow-control); display: grid; grid-template-columns: 30px minmax(0, 1fr) 16px; align-items: center; gap: 8px; min-width: 0; min-height: 62px; padding: 7px; border: 1px solid var(--border); border-radius: var(--radius-control); background: var(--material-control); color: var(--text); box-shadow: var(--shadow-control); text-align: left; cursor: grab; }
  .catalog-item:focus:not(:focus-visible),
  .catalog-item:active:not(:disabled) { outline-color: var(--ui-entity-outline-color, var(--entity-selection-outline)); outline-style: solid; background: var(--material-control); box-shadow: var(--shadow-control); }
  .catalog-item:active:not(:disabled) { cursor: grabbing; }
  .catalog-item.blocked { opacity: 0.58; cursor: not-allowed; }
  .item-icon { display: inline-flex; align-items: center; justify-content: center; width: 30px; height: 30px; border: 1px solid color-mix(in srgb, var(--brand) 18%, var(--border)); border-radius: 7px; background: color-mix(in srgb, var(--brand) 8%, var(--control-bg)); color: var(--brand-strong); }
  .item-copy { display: grid; align-content: start; gap: 2px; min-width: 0; }
  .item-copy strong { overflow: hidden; color: var(--text-strong); font-size: 11px; font-weight: 750; text-overflow: ellipsis; white-space: nowrap; }
  .item-copy small { overflow: hidden; color: var(--text-muted); font-size: 11px; line-height: 1.3; text-overflow: ellipsis; white-space: nowrap; }
  .item-meta { display: flex; align-items: center; gap: 5px; min-width: 0; }
  .item-meta code { overflow: hidden; color: var(--brand-strong); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .drag-handle { display: inline-flex; align-items: center; justify-content: center; color: var(--text-soft); opacity: 0.58; }
  .catalog-item:hover:not(:disabled) .drag-handle { color: var(--brand-strong); opacity: 1; }
  .catalog-state { display: flex; align-items: flex-start; gap: 7px; padding: 12px; border: 1px dashed var(--border); border-radius: var(--radius-control); color: var(--text-muted); font-size: 11px; line-height: 1.45; }
  .catalog-state.error { border-color: color-mix(in srgb, var(--danger) 42%, var(--border)); color: var(--danger); }
  .catalog-state span { display: grid; gap: 3px; }
  .blocked-note { margin: 9px 1px 1px; color: var(--text-soft); font-size: 11px; line-height: 1.4; }
</style>
