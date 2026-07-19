<script lang="ts">
  import {
    IconBox,
    IconCode,
    IconDatabase,
    IconFileCode,
    IconFileDescription,
    IconFiles,
    IconPhoto,
    IconSearch,
    IconStack2,
  } from "@tabler/icons-svelte";
  import {
    sourceNodeKindLabel,
    sourceOriginLabel,
    sourceRelationKindLabel,
  } from "$lib/source-graph/view";
  import { sourceNodeSubtitle } from "$lib/source-graph/workspace-view";
  import type {
    SourceGraph,
    SourceGraphNode,
    SourceGraphPage,
    SourceGraphRelation,
  } from "$lib/types";
  import { sourceDisplayPath } from "./workspace-model";

  type SourceTab = "pages" | "templates" | "components" | "styles" | "resources";
  type SourceEntry = {
    id: string;
    nodeId: string;
    label: string;
    path: string;
    description: string;
    tab: SourceTab;
  };

  let {
    graph,
    page = null,
    selectedNode = null,
    outgoingRelations = [],
    incomingRelations = [],
    impactLabel = "",
    editabilityLabel = "",
    structureLabel = "",
    onSelectNode = () => {},
    onOpenSource = () => {},
  }: {
    graph: SourceGraph;
    page?: SourceGraphPage | null;
    selectedNode?: SourceGraphNode | null;
    outgoingRelations?: SourceGraphRelation[];
    incomingRelations?: SourceGraphRelation[];
    impactLabel?: string;
    editabilityLabel?: string;
    structureLabel?: string;
    onSelectNode?: (nodeId: string) => void;
    onOpenSource?: (path: string) => void | Promise<void>;
  } = $props();

  let activeTab = $state<SourceTab>("pages");
  let query = $state("");
  const nodesById = $derived(new Map(graph.nodes.map((node) => [node.id, node])));
  const entries = $derived(buildEntries(graph));
  const visibleEntries = $derived.by(() => {
    const normalized = query.trim().toLocaleLowerCase("ro");
    return entries.filter((entry) => entry.tab === activeTab && (
      !normalized
      || entry.label.toLocaleLowerCase("ro").includes(normalized)
      || entry.path.toLocaleLowerCase("ro").includes(normalized)
    ));
  });

  function buildEntries(sourceGraph: SourceGraph): SourceEntry[] {
    return [
      ...sourceGraph.pages.map((item) => ({
        id: item.id,
        nodeId: item.id,
        label: item.title,
        path: item.file,
        description: `${item.url} · conținut Zola`,
        tab: "pages" as const,
      })),
      ...sourceGraph.templates.filter((item) => !item.isPartial).map((item) => ({
        id: item.id,
        nodeId: item.nodeId,
        label: item.name,
        path: item.file,
        description: `${sourceOriginLabel(item.origin, item.themeName)} · ${item.blocks.length} blocuri`,
        tab: "templates" as const,
      })),
      ...sourceGraph.templates.filter((item) => item.isPartial).map((item) => ({
        id: item.id,
        nodeId: item.nodeId,
        label: item.name,
        path: item.file,
        description: `${sourceOriginLabel(item.origin, item.themeName)} · ${item.macros.length ? "macro" : "partial"}`,
        tab: "components" as const,
      })),
      ...sourceGraph.styles.map((item) => ({
        id: item.id,
        nodeId: item.nodeId,
        label: sourceDisplayPath(item.file),
        path: item.file,
        description: `${item.scope} · ${sourceOriginLabel(item.origin, item.themeName)}`,
        tab: "styles" as const,
      })),
      ...sourceGraph.scripts.map((item) => ({
        id: item.id,
        nodeId: item.nodeId,
        label: item.logicalPath,
        path: item.file,
        description: "Script JavaScript",
        tab: "resources" as const,
      })),
      ...sourceGraph.dataFiles.map((item) => ({
        id: item.id,
        nodeId: item.nodeId,
        label: item.logicalPath,
        path: item.file,
        description: "Fișier de date",
        tab: "resources" as const,
      })),
      ...sourceGraph.assets.map((item) => ({
        id: item.id,
        nodeId: item.nodeId,
        label: item.logicalPath,
        path: item.file,
        description: "Resursă statică",
        tab: "resources" as const,
      })),
    ];
  }

  function tabCount(tab: SourceTab) {
    return entries.filter((entry) => entry.tab === tab).length;
  }

  function relationNode(relation: SourceGraphRelation, direction: "in" | "out") {
    return nodesById.get(direction === "out" ? relation.to : relation.from) ?? null;
  }
</script>

<div class="sources-stage">
  <header class="stage-heading">
    <div>
      <span class="section-kicker">Mod avansat</span>
      <h1>Fișiere și relații</h1>
      <p>Explorează implementarea tehnică a site-ului. Această zonă nu schimbă structura prin drag-and-drop.</p>
    </div>
    {#if selectedNode}
      <button class="open-code" type="button" onclick={() => { void onOpenSource(selectedNode.file); }}><IconCode size={16} /> Deschide în editor</button>
    {/if}
  </header>

  <div class="sources-workspace">
    <aside class="source-explorer">
      <label class="source-search"><IconSearch size={15} /><input bind:value={query} placeholder="Caută fișiere…" aria-label="Caută fișiere" /></label>
      <nav class="source-tabs" aria-label="Tipuri de fișiere">
        <button class:active={activeTab === "pages"} type="button" onclick={() => (activeTab = "pages")}><IconFileDescription size={15} /><span>Conținut</span><em>{tabCount("pages")}</em></button>
        <button class:active={activeTab === "templates"} type="button" onclick={() => (activeTab = "templates")}><IconFileCode size={15} /><span>Template-uri</span><em>{tabCount("templates")}</em></button>
        <button class:active={activeTab === "components"} type="button" onclick={() => (activeTab = "components")}><IconBox size={15} /><span>Componente</span><em>{tabCount("components")}</em></button>
        <button class:active={activeTab === "styles"} type="button" onclick={() => (activeTab = "styles")}><IconCode size={15} /><span>Stiluri</span><em>{tabCount("styles")}</em></button>
        <button class:active={activeTab === "resources"} type="button" onclick={() => (activeTab = "resources")}><IconDatabase size={15} /><span>Resurse</span><em>{tabCount("resources")}</em></button>
      </nav>
      <div class="source-list">
        {#each visibleEntries as entry}
          <button class:active={selectedNode?.id === entry.nodeId || selectedNode?.id === entry.id} type="button" onclick={() => onSelectNode(entry.nodeId)} ondblclick={() => { void onOpenSource(entry.path); }}>
            <span class="entry-icon">
              {#if entry.tab === "pages"}<IconFileDescription size={16} />
              {:else if entry.tab === "templates"}<IconFileCode size={16} />
              {:else if entry.tab === "components"}<IconBox size={16} />
              {:else if entry.tab === "styles"}<IconCode size={16} />
              {:else}<IconPhoto size={16} />{/if}
            </span>
            <span class="entry-copy"><strong>{entry.label}</strong><small>{entry.description}</small><code>{sourceDisplayPath(entry.path)}</code></span>
          </button>
        {:else}
          <div class="source-empty"><IconFiles size={25} stroke={1.5} /><span>{query ? "Niciun rezultat" : "Niciun fișier în această categorie"}</span></div>
        {/each}
      </div>
    </aside>

    <main class="source-detail">
      {#if selectedNode}
        <section class="selected-source">
          <div class="selected-icon"><IconFileCode size={22} stroke={1.7} /></div>
          <div>
            <span>{sourceNodeKindLabel(selectedNode.kind)}</span>
            <h2>{selectedNode.label}</h2>
            <code>{sourceDisplayPath(selectedNode.file)}</code>
          </div>
          <span class:theme={selectedNode.origin === "theme"} class="origin-badge">{sourceOriginLabel(selectedNode.origin, selectedNode.themeName)}</span>
        </section>

        <section class="human-context">
          <article><span>Ce controlează</span><strong>{structureLabel || sourceNodeSubtitle(selectedNode)}</strong><p>{impactLabel}</p></article>
          <article><span>Cum se editează</span><strong>{editabilityLabel || "În editorul de cod"}</strong><p>{selectedNode.capabilities.canOpenInCode ? "Fișier disponibil în editorul de cod." : "Sursă protejată sau derivată."}</p></article>
          <article><span>Pagina activă</span><strong>{page?.title ?? "Nicio pagină"}</strong><p>{page ? `${page.url} · selecția tehnică nu schimbă pagina deschisă` : "Selectează o pagină din navigator."}</p></article>
        </section>

        <div class="relationship-grid">
          <details open>
            <summary><IconStack2 size={16} /><span><strong>Folosește</strong><small>{outgoingRelations.length} relații către alte surse</small></span></summary>
            <div class="relation-list">
              {#each outgoingRelations as relation}
                {@const node = relationNode(relation, "out")}
                <button type="button" onclick={() => onSelectNode(relation.to)}>
                  <span>{sourceRelationKindLabel(relation.kind)}</span><strong>{node?.label ?? relation.label}</strong><code>{node ? sourceDisplayPath(node.file) : relation.to}</code>
                </button>
              {:else}<p>Acest element nu are relații ieșite detectate.</p>{/each}
            </div>
          </details>
          <details open>
            <summary><IconStack2 size={16} /><span><strong>Este folosit de</strong><small>{incomingRelations.length} relații din alte surse</small></span></summary>
            <div class="relation-list">
              {#each incomingRelations as relation}
                {@const node = relationNode(relation, "in")}
                <button type="button" onclick={() => onSelectNode(relation.from)}>
                  <span>{sourceRelationKindLabel(relation.kind)}</span><strong>{node?.label ?? relation.label}</strong><code>{node ? sourceDisplayPath(node.file) : relation.from}</code>
                </button>
              {:else}<p>Nicio altă sursă nu indică spre acest element.</p>{/each}
            </div>
          </details>
        </div>

        <details class="raw-details">
          <summary>Detalii tehnice complete</summary>
          <dl>
            <div><dt>ID Source Graph</dt><dd>{selectedNode.id}</dd></div>
            <div><dt>Fișier</dt><dd>{selectedNode.file}</dd></div>
            <div><dt>Origine</dt><dd>{sourceOriginLabel(selectedNode.origin, selectedNode.themeName)}</dd></div>
            <div><dt>Poziție</dt><dd>{selectedNode.range ? `${selectedNode.range.line}:${selectedNode.range.column}` : "Fișier întreg"}</dd></div>
            <div><dt>Copii în graph</dt><dd>{selectedNode.children.length}</dd></div>
          </dl>
        </details>
      {:else}
        <div class="detail-empty"><IconFileCode size={34} stroke={1.5} /><h2>Alege un fișier</h2><p>Detaliile și relațiile lui cu restul site-ului vor apărea aici.</p></div>
      {/if}
    </main>
  </div>
</div>

<style>
  .sources-stage {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 15px;
    min-width: 0;
    min-height: 0;
    padding: 20px;
  }

  .stage-heading {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 18px;
  }

  .stage-heading > div {
    display: grid;
    gap: 4px;
  }

  .section-kicker,
  .selected-source > div:nth-child(2) > span,
  .human-context article > span,
  dt,
  .relation-list button span {
    color: var(--brand);
    font-size: 10px;
    font-weight: 900;
    letter-spacing: .08em;
    text-transform: uppercase;
  }

  h1,
  h2,
  p,
  dl,
  dd {
    margin: 0;
  }

  h1 {
    color: var(--text-strong);
    font-size: clamp(24px, 2.2vw, 34px);
    line-height: 1.08;
  }

  .stage-heading p {
    color: var(--text-muted);
    font-size: 12px;
  }

  button {
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text-strong);
    background: var(--surface);
    font: inherit;
    cursor: pointer;
  }

  button:hover {
    border-color: color-mix(in srgb, var(--brand) 55%, var(--border));
  }

  .open-code {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 34px;
    padding: 0 11px;
    font-size: 11px;
    font-weight: 850;
  }

  .sources-workspace {
    display: grid;
    grid-template-columns: minmax(270px, 320px) minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border: 1px solid var(--border-2);
    border-radius: 12px;
    background: var(--surface-2);
  }

  .source-explorer {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    border-right: 1px solid var(--border-2);
    background: color-mix(in srgb, var(--surface-2) 72%, var(--surface));
  }

  .source-search {
    display: flex;
    align-items: center;
    gap: 7px;
    margin: 10px;
    padding: 0 9px;
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text-muted);
    background: var(--surface);
  }

  .source-search input {
    width: 100%;
    height: 33px;
    border: 0;
    outline: 0;
    color: var(--text);
    background: transparent;
    font: inherit;
    font-size: 11px;
  }

  .source-tabs {
    display: grid;
    gap: 2px;
    padding: 0 8px 9px;
    border-bottom: 1px solid var(--border-2);
  }

  .source-tabs button {
    display: grid;
    grid-template-columns: 21px minmax(0, 1fr) auto;
    gap: 7px;
    align-items: center;
    min-height: 32px;
    padding: 4px 7px;
    border-color: transparent;
    background: transparent;
    font-size: 11px;
    font-weight: 800;
    text-align: left;
  }

  .source-tabs button.active {
    border-color: color-mix(in srgb, var(--brand) 22%, var(--border));
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 8%, var(--surface));
  }

  .source-tabs em {
    color: var(--text-muted);
    font-size: 10px;
    font-style: normal;
  }

  .source-list {
    min-height: 0;
    overflow: auto;
    padding: 8px;
  }

  .source-list > button {
    display: grid;
    grid-template-columns: 27px minmax(0, 1fr);
    gap: 7px;
    width: 100%;
    min-height: 58px;
    margin-bottom: 5px;
    padding: 7px;
    border-color: transparent;
    background: transparent;
    text-align: left;
  }

  .source-list > button:hover,
  .source-list > button.active {
    border-color: var(--border);
    background: var(--surface);
  }

  .source-list > button.active {
    border-color: var(--brand);
  }

  .entry-icon {
    display: grid;
    width: 27px;
    height: 27px;
    place-items: center;
    border-radius: 7px;
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 9%, var(--surface));
  }

  .entry-copy {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .entry-copy strong,
  .entry-copy small,
  .entry-copy code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-copy strong {
    color: var(--text-strong);
    font-size: 11px;
  }

  .entry-copy small {
    color: var(--text-muted);
    font-size: 9px;
    font-weight: 750;
  }

  code {
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
    font-size: 9px;
  }

  .source-empty,
  .detail-empty {
    display: grid;
    justify-items: center;
    align-content: center;
    gap: 7px;
    min-height: 160px;
    color: var(--text-muted);
    font-size: 11px;
    text-align: center;
  }

  .source-detail {
    min-width: 0;
    min-height: 0;
    overflow: auto;
    padding: 15px;
  }

  .selected-source {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 11px;
    align-items: center;
    padding: 14px;
    border: 1px solid var(--border-2);
    border-radius: 11px;
    background: var(--surface);
  }

  .selected-icon {
    display: grid;
    width: 42px;
    height: 42px;
    place-items: center;
    border-radius: 11px;
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 10%, var(--surface));
  }

  .selected-source > div:nth-child(2) {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .selected-source h2 {
    overflow: hidden;
    color: var(--text-strong);
    font-size: 17px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .origin-badge {
    padding: 5px 8px;
    border-radius: 999px;
    color: #0f766e;
    background: color-mix(in srgb, #0f766e 10%, var(--surface));
    font-size: 9px;
    font-weight: 900;
  }

  .origin-badge.theme {
    color: #c2410c;
    background: color-mix(in srgb, #c2410c 10%, var(--surface));
  }

  .human-context {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
    margin-top: 10px;
  }

  .human-context article {
    display: grid;
    align-content: start;
    gap: 5px;
    min-height: 104px;
    padding: 12px;
    border: 1px solid var(--border-2);
    border-radius: 10px;
    background: var(--surface);
  }

  .human-context strong {
    color: var(--text-strong);
    font-size: 12px;
  }

  .human-context p {
    color: var(--text-muted);
    font-size: 10px;
    line-height: 1.45;
  }

  .relationship-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    margin-top: 10px;
  }

  .relationship-grid details,
  .raw-details {
    overflow: hidden;
    border: 1px solid var(--border-2);
    border-radius: 10px;
    background: var(--surface);
  }

  .relationship-grid summary {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    padding: 10px;
    cursor: pointer;
    list-style: none;
  }

  .relationship-grid summary {
    color: var(--brand);
  }

  .relationship-grid summary span {
    display: grid;
    gap: 1px;
  }

  .relationship-grid summary strong {
    color: var(--text-strong);
    font-size: 11px;
  }

  .relationship-grid summary small {
    color: var(--text-muted);
    font-size: 9px;
  }

  .relation-list {
    display: grid;
    gap: 5px;
    padding: 0 8px 8px;
  }

  .relation-list button {
    display: grid;
    grid-template-columns: 95px minmax(0, 1fr);
    gap: 3px 7px;
    padding: 7px;
    text-align: left;
  }

  .relation-list button code {
    grid-column: 2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .relation-list button strong {
    overflow: hidden;
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .relation-list p {
    padding: 8px;
    color: var(--text-muted);
    font-size: 10px;
  }

  .raw-details {
    margin-top: 10px;
  }

  .raw-details summary {
    padding: 10px 12px;
    color: var(--text-strong);
    font-size: 11px;
    font-weight: 850;
    cursor: pointer;
  }

  .raw-details dl {
    display: grid;
    padding: 0 10px 10px;
  }

  .raw-details dl div {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr);
    gap: 8px;
    padding: 7px 0;
    border-top: 1px solid var(--border-2);
  }

  .raw-details dd {
    overflow: hidden;
    color: var(--text);
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail-empty {
    min-height: 420px;
  }

  .detail-empty h2 {
    color: var(--text-strong);
    font-size: 17px;
  }

  @media (max-width: 1050px) {
    .human-context,
    .relationship-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
