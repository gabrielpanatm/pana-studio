<script lang="ts">
  import {
    IconBraces,
    IconCode,
    IconFileDescription,
    IconLayoutBottombar,
    IconLayoutNavbar,
    IconLayoutRows,
    IconPlus,
    IconSettings,
    IconStack2,
  } from "@tabler/icons-svelte";
  import type { ReusablePart } from "$lib/source-graph/architecture";
  import { reusablePartRoleLabel } from "$lib/source-graph/architecture";
  import {
    firstSourceNodeInOutline,
    outlineItemContainsNode,
    type SemanticOutlineItem,
  } from "$lib/source-graph/outline";
  import type { SourceGraphPage, SourceGraphStyle, SourceGraphTemplate } from "$lib/types";
  import SitePreview from "./SitePreview.svelte";
  import { sourceDisplayPath, styleScopeLabel } from "./workspace-model";

  let {
    page = null,
    zones = [],
    selectedNodeId = null,
    templateChain = [],
    styles = [],
    reusableParts = [],
    previewUrl = "",
    previewMarkup = null,
    onSelectZone = () => {},
    onSelectNode = () => {},
    onOpenSource = () => {},
    onCreateComponent = () => {},
    onCreateLoop = () => {},
    onOpenExternal = () => {},
  }: {
    page?: SourceGraphPage | null;
    zones?: SemanticOutlineItem[];
    selectedNodeId?: string | null;
    templateChain?: SourceGraphTemplate[];
    styles?: SourceGraphStyle[];
    reusableParts?: ReusablePart[];
    previewUrl?: string;
    previewMarkup?: string | null;
    onSelectZone?: (zone: SemanticOutlineItem) => void;
    onSelectNode?: (nodeId: string) => void;
    onOpenSource?: (path: string) => void | Promise<void>;
    onCreateComponent?: () => void;
    onCreateLoop?: () => void;
    onOpenExternal?: () => void | Promise<void>;
  } = $props();

  function zoneRole(item: SemanticOutlineItem) {
    const text = `${item.id} ${item.label} ${zoneFile(item)}`.toLowerCase();
    if (text.includes("header") || text.includes("nav")) return "header";
    if (text.includes("footer")) return "footer";
    if (text.includes("head")) return "technical";
    return "main";
  }

  function zoneTitle(item: SemanticOutlineItem) {
    const role = zoneRole(item);
    if (role === "header") return "Antetul site-ului";
    if (role === "footer") return "Subsolul site-ului";
    if (role === "technical") return "Setările documentului";
    return item.id === "semantic:main" ? "Conținutul paginii" : item.label;
  }

  function zoneDescription(item: SemanticOutlineItem) {
    const role = zoneRole(item);
    if (role === "header") return "Logo, meniu și elementele afișate în partea de sus.";
    if (role === "footer") return "Informațiile și navigația comune de la finalul paginii.";
    if (role === "technical") return "Titlu, metadate, stiluri și scripturi ale documentului.";
    return "Secțiunile și elementele care apar numai în pagina deschisă.";
  }

  function zoneScope(item: SemanticOutlineItem) {
    const role = zoneRole(item);
    if (role === "header" || role === "footer") return "Comun în site";
    if (role === "technical") return "Tehnic";
    return "Doar această pagină";
  }

  function zoneFile(item: SemanticOutlineItem) {
    return (item.node ?? firstSourceNodeInOutline(item))?.file ?? "";
  }

  function zoneCount(item: SemanticOutlineItem): number {
    return (item.node ? 1 : 0) + item.children.reduce((total, child) => total + zoneCount(child), 0);
  }
</script>

<div class="structure-stage">
  <header class="stage-heading">
    <div>
      <span class="section-kicker">Structura paginii</span>
      <h1>{page?.title ?? "Alege o pagină"}</h1>
      <p>Vezi din ce zone este alcătuită pagina și ce elemente sunt locale sau comune întregului site.</p>
    </div>
    <div class="heading-actions">
      <button type="button" onclick={onCreateLoop}><IconPlus size={16} /> Listă dinamică</button>
      <button type="button" onclick={onCreateComponent}><IconPlus size={16} /> Componentă comună</button>
    </div>
  </header>

  <div class="structure-workspace">
    <section class="canvas-column">
      <SitePreview
        title={page?.title ?? "Structura site-ului"}
        route={page?.url ?? "/"}
        src={previewUrl}
        srcdoc={previewMarkup}
        onOpenExternal={onOpenExternal}
      />
    </section>

    <aside class="structure-inspector">
      <section class="zone-map">
        <header>
          <span><IconLayoutRows size={17} stroke={1.8} /></span>
          <div><small>Harta vizuală</small><h2>Zonele paginii</h2></div>
        </header>
        <div class="zone-list">
          {#each zones as zone}
            {@const role = zoneRole(zone)}
            <button
              class:active={outlineItemContainsNode(zone, selectedNodeId)}
              class:shared={role === "header" || role === "footer"}
              class="zone-card"
              type="button"
              onclick={() => onSelectZone(zone)}
            >
              <span class="zone-icon">
                {#if role === "header"}<IconLayoutNavbar size={18} stroke={1.8} />
                {:else if role === "footer"}<IconLayoutBottombar size={18} stroke={1.8} />
                {:else if role === "technical"}<IconSettings size={18} stroke={1.8} />
                {:else}<IconFileDescription size={18} stroke={1.8} />{/if}
              </span>
              <span class="zone-copy">
                <small>{zoneScope(zone)}</small>
                <strong>{zoneTitle(zone)}</strong>
                <em>{zoneDescription(zone)}</em>
              </span>
              <span class="zone-count">{zoneCount(zone)}</span>
            </button>
          {:else}
            <div class="empty-zones"><IconBraces size={24} stroke={1.5} /><span>Nu au fost detectate zone în template-ul paginii.</span></div>
          {/each}
        </div>
      </section>

      <details class="technical-details">
        <summary>
          <span><IconStack2 size={17} stroke={1.8} /></span>
          <span><strong>Lanțul de randare</strong><small>Detalii tehnice despre Tera și SCSS</small></span>
        </summary>
        <div class="technical-list">
          {#if page}
            <button type="button" onclick={() => { void onOpenSource(page.file); }}>
              <span>Conținut</span><strong>{sourceDisplayPath(page.file)}</strong><IconCode size={14} />
            </button>
          {/if}
          {#each templateChain as template}
            <button class:active={selectedNodeId === template.nodeId} type="button" onclick={() => onSelectNode(template.nodeId)} ondblclick={() => { void onOpenSource(template.file); }}>
              <span>{template.isPartial ? "Componentă" : "Template"}</span><strong>{template.name}</strong><IconCode size={14} />
            </button>
          {/each}
          {#each styles as style}
            <button class:active={selectedNodeId === style.nodeId} type="button" onclick={() => onSelectNode(style.nodeId)} ondblclick={() => { void onOpenSource(style.file); }}>
              <span>{styleScopeLabel(style.scope)}</span><strong>{sourceDisplayPath(style.file)}</strong><IconCode size={14} />
            </button>
          {/each}
        </div>
      </details>

      <details class="technical-details">
        <summary>
          <span><IconBraces size={17} stroke={1.8} /></span>
          <span><strong>Componente comune</strong><small>Elemente reutilizate în mai multe pagini</small></span>
        </summary>
        <div class="technical-list">
          {#each reusableParts as part}
            <button class:active={selectedNodeId === part.nodeId} type="button" onclick={() => onSelectNode(part.nodeId)} ondblclick={() => { void onOpenSource(part.file); }}>
              <span>{reusablePartRoleLabel(part.role)}</span><strong>{part.label}</strong><em>{part.usedBy} utilizări</em>
            </button>
          {:else}
            <p>Nu există componente reutilizabile detectate.</p>
          {/each}
        </div>
      </details>
    </aside>
  </div>
</div>

<style>
  .structure-stage {
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

  .stage-heading > div:first-child {
    display: grid;
    gap: 4px;
  }

  .section-kicker,
  .zone-map header small,
  .zone-copy small,
  .technical-list button span {
    color: var(--brand);
    font-size: 10px;
    font-weight: 900;
    letter-spacing: .07em;
    text-transform: uppercase;
  }

  h1,
  h2,
  p {
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

  .heading-actions {
    display: flex;
    gap: 7px;
  }

  button {
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text-strong);
    background: var(--surface);
    font: inherit;
    cursor: pointer;
  }

  .heading-actions button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 34px;
    padding: 0 10px;
    font-size: 11px;
    font-weight: 850;
  }

  button:hover {
    border-color: color-mix(in srgb, var(--brand) 55%, var(--border));
  }

  .structure-workspace {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(300px, 350px);
    gap: 14px;
    min-width: 0;
    min-height: 0;
  }

  .canvas-column {
    min-width: 0;
    min-height: 0;
  }

  .structure-inspector {
    display: grid;
    align-content: start;
    gap: 9px;
    min-width: 0;
    min-height: 0;
    overflow: auto;
  }

  .zone-map,
  .technical-details {
    border: 1px solid var(--border-2);
    border-radius: 11px;
    background: var(--surface-2);
  }

  .zone-map {
    padding: 11px;
  }

  .zone-map header {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    padding: 2px 2px 10px;
  }

  .zone-map header > span,
  .technical-details summary > span:first-child {
    display: grid;
    width: 29px;
    height: 29px;
    place-items: center;
    border-radius: 8px;
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 10%, var(--surface));
  }

  .zone-map header div {
    display: grid;
    gap: 1px;
  }

  .zone-map h2 {
    color: var(--text-strong);
    font-size: 14px;
  }

  .zone-list {
    display: grid;
    gap: 6px;
  }

  .zone-card {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    width: 100%;
    min-height: 72px;
    padding: 8px;
    text-align: left;
  }

  .zone-card.active {
    border-color: var(--brand);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--brand) 18%, transparent);
  }

  .zone-card.shared .zone-icon {
    color: #7c3aed;
    background: color-mix(in srgb, #7c3aed 10%, var(--surface));
  }

  .zone-icon {
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    border-radius: 9px;
    color: #2563eb;
    background: color-mix(in srgb, #2563eb 9%, var(--surface));
  }

  .zone-copy {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .zone-copy strong {
    color: var(--text-strong);
    font-size: 12px;
  }

  .zone-copy em {
    overflow: hidden;
    color: var(--text-muted);
    font-size: 10px;
    font-style: normal;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .zone-count {
    display: grid;
    min-width: 24px;
    height: 24px;
    place-items: center;
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--surface-3);
    font-size: 10px;
    font-weight: 900;
  }

  .empty-zones {
    display: grid;
    justify-items: center;
    gap: 7px;
    padding: 18px 10px;
    color: var(--text-muted);
    font-size: 11px;
    text-align: center;
  }

  .technical-details {
    overflow: hidden;
  }

  .technical-details summary {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    padding: 10px;
    cursor: pointer;
    list-style: none;
  }

  .technical-details summary::-webkit-details-marker {
    display: none;
  }

  .technical-details summary > span:last-child {
    display: grid;
    gap: 1px;
  }

  .technical-details summary strong {
    color: var(--text-strong);
    font-size: 12px;
  }

  .technical-details summary small {
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 700;
  }

  .technical-list {
    display: grid;
    gap: 5px;
    padding: 0 9px 9px;
  }

  .technical-list button {
    display: grid;
    grid-template-columns: 88px minmax(0, 1fr) auto;
    gap: 7px;
    align-items: center;
    min-height: 38px;
    padding: 6px 8px;
    text-align: left;
  }

  .technical-list button.active {
    border-color: var(--brand);
  }

  .technical-list button strong {
    overflow: hidden;
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .technical-list button em {
    color: var(--text-muted);
    font-size: 10px;
    font-style: normal;
  }

  .technical-list p {
    padding: 8px;
    color: var(--text-muted);
    font-size: 11px;
  }

  @media (max-width: 1050px) {
    .structure-workspace {
      grid-template-columns: minmax(0, 1fr);
    }

    .structure-inspector {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      overflow: visible;
    }

    .zone-map {
      grid-row: span 2;
    }
  }
</style>
