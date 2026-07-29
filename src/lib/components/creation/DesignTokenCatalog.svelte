<script lang="ts">
  import {
    IconActivity,
    IconAlertTriangle,
    IconBorderRadius,
    IconDeviceDesktop,
    IconLayoutGrid,
    IconPalette,
    IconRuler2,
    IconShadow,
    IconStack2,
    IconTypography,
  } from "@tabler/icons-svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type {
    DesignTokenCatalogSnapshot,
    DesignTokenSnapshot,
  } from "$lib/types";

  let {
    catalog,
    loading = false,
    error = "",
    query = "",
    category = "all",
    selectedId = "",
    selectToken,
  }: {
    catalog: DesignTokenCatalogSnapshot | null;
    loading?: boolean;
    error?: string;
    query?: string;
    category?: string;
    selectedId?: string;
    selectToken: (token: DesignTokenSnapshot) => void;
  } = $props();

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const visibleTokens = $derived(
    (catalog?.tokens ?? []).filter((token) => (
      (category === "all" || token.categoryId === category)
      && (
        !normalizedQuery
        || `${token.name} ${token.rawValue} ${token.resolvedValue ?? ""} ${token.sourcePath} ${token.groupLabel}`
          .toLocaleLowerCase(l10n.locale)
          .includes(normalizedQuery)
      )
    )),
  );
  const visibleCategories = $derived(
    (catalog?.categories ?? [])
      .map((entry) => ({
        ...entry,
        tokens: visibleTokens.filter((token) => token.categoryId === entry.id),
      }))
      .filter((entry) => entry.tokens.length > 0),
  );

  function previewStyle(token: DesignTokenSnapshot) {
    const value = token.resolvedValue;
    if (!value || token.diagnostic) return "";
    return `--design-token-preview: ${value}`;
  }

  function kindLabel(token: DesignTokenSnapshot) {
    if (token.visualKind === "font_family") return t("design-token-kind-family");
    if (token.visualKind === "font_size") return t("design-token-kind-size");
    if (token.visualKind === "font_weight") return t("design-token-kind-weight");
    if (token.visualKind === "line_height") return t("design-token-kind-line-height");
    if (token.visualKind === "letter_spacing") return t("design-token-kind-letter-spacing");
    if (token.visualKind === "breakpoint") return t("design-token-kind-viewport");
    if (token.visualKind === "layer") return t("design-token-kind-layer");
    return token.groupLabel;
  }

  function categoryKicker(categoryId: string) {
    if (categoryId === "color") return t("design-token-kicker-color");
    if (categoryId === "typography") return t("design-token-kicker-typography");
    if (categoryId === "spacing") return t("design-token-kicker-spacing");
    if (categoryId === "radius") return t("design-token-kicker-radius");
    if (categoryId === "shadow") return t("design-token-kicker-shadow");
    if (categoryId === "transition") return t("design-token-kicker-transition");
    if (categoryId === "breakpoint") return t("design-token-kicker-breakpoint");
    if (categoryId === "layout") return t("design-token-kicker-layout");
    if (categoryId === "layer") return t("design-token-kicker-layer");
    return t("design-token-kicker-default");
  }

  function categoryTitle(categoryId: string, fallback: string) {
    if (categoryId === "color") return t("design-token-title-color");
    if (categoryId === "typography") return t("design-token-title-typography");
    if (categoryId === "spacing") return t("design-token-title-spacing");
    if (categoryId === "radius") return t("design-token-title-radius");
    if (categoryId === "shadow") return t("design-token-title-shadow");
    if (categoryId === "transition") return t("design-token-title-transition");
    if (categoryId === "breakpoint") return t("design-token-title-breakpoint");
    if (categoryId === "layout") return t("design-token-title-layout");
    if (categoryId === "layer") return t("design-token-title-layer");
    return fallback;
  }
</script>

<div class="token-catalog" aria-label={t("design-token-catalog-label")}>
  {#if loading && !catalog}
    <div class="catalog-state">{t("design-token-loading")}</div>
  {:else if error}
    <div class="catalog-state error" role="alert">
      <IconAlertTriangle size={16} /> {error}
    </div>
  {:else if visibleCategories.length === 0}
    <div class="catalog-state">{t("design-token-empty")}</div>
  {:else}
    <div class="token-sections">
      {#each visibleCategories as section (section.id)}
        <section class:palette-section={section.id === "color"} class="token-section">
          <header>
            <span class={`section-icon ${section.id}`}>
              {#if section.id === "color"}
                <IconPalette size={17} stroke={1.8} />
              {:else if section.id === "typography"}
                <IconTypography size={17} stroke={1.8} />
              {:else if section.id === "spacing"}
                <IconRuler2 size={17} stroke={1.8} />
              {:else if section.id === "radius"}
                <IconBorderRadius size={17} stroke={1.8} />
              {:else if section.id === "shadow"}
                <IconShadow size={17} stroke={1.8} />
              {:else if section.id === "transition"}
                <IconActivity size={17} stroke={1.8} />
              {:else if section.id === "breakpoint"}
                <IconDeviceDesktop size={17} stroke={1.8} />
              {:else if section.id === "layer"}
                <IconStack2 size={17} stroke={1.8} />
              {:else}
                <IconLayoutGrid size={17} stroke={1.8} />
              {/if}
            </span>
            <div>
              <span class="section-kicker">{categoryKicker(section.id)}</span>
              <h2>{categoryTitle(section.id, section.label)}</h2>
            </div>
          </header>

          <div class="token-grid">
            {#each section.tokens as token (token.id)}
              <article
                class="token-card ui-entity-selectable"
                data-ui-selected={selectedId === token.id ? "true" : undefined}
                class:invalid={Boolean(token.diagnostic)}
              >
                <button
                  class="token-select ui-entity-trigger"
                  type="button"
                  aria-label={t("design-token-select", { name: token.name })}
                  aria-pressed={selectedId === token.id}
                  onclick={() => selectToken(token)}
                >
                  <span
                    class={`token-preview ${token.visualKind}`}
                    style={previewStyle(token)}
                    aria-hidden="true"
                  >
                    {#if token.diagnostic}
                      <IconAlertTriangle size={19} stroke={1.8} />
                    {:else if token.visualKind === "color"}
                      <span></span>
                    {:else if token.visualKind === "font_family"}
                      <span class="type-sample family">Aa</span>
                    {:else if token.visualKind === "font_size"}
                      <span class="type-sample size">Aa</span>
                    {:else if token.visualKind === "font_weight"}
                      <span class="type-sample weight">Aa</span>
                    {:else if token.visualKind === "line_height"}
                      <span class="type-sample leading">Aa<br />Aa</span>
                    {:else if token.visualKind === "letter_spacing"}
                      <span class="type-sample tracking">Ab</span>
                    {:else if token.visualKind === "spacing"}
                      <span class="spacing-track"><span></span></span>
                    {:else if token.visualKind === "radius"}
                      <span class="radius-sample"></span>
                    {:else if token.visualKind === "shadow"}
                      <span class="shadow-sample"></span>
                    {:else if token.visualKind === "transition"}
                      <span class="transition-sample"><span></span></span>
                    {:else if token.visualKind === "breakpoint"}
                      <span class="breakpoint-sample"></span>
                    {:else if token.visualKind === "layer"}
                      <span class="layer-sample"><i></i><i></i><i></i></span>
                    {:else}
                      <span class="value-sample">{token.resolvedValue ?? token.rawValue}</span>
                    {/if}
                  </span>

                  <span class="token-copy">
                    <strong>${token.name}</strong>
                    <small>{kindLabel(token)}</small>
                    <code>{token.rawValue}</code>
                    {#if token.resolvedValue && token.resolvedValue !== token.rawValue}
                      <span class="resolved-value">→ {token.resolvedValue}</span>
                    {/if}
                  </span>
                </button>
              </article>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {/if}
</div>

<style>
  .token-catalog { min-width: 0; }
  .token-sections { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; }
  .token-section { min-width: 0; padding: 7px 4px 16px; border-bottom: 1px solid var(--wb-border-subtle); }
  .token-section:last-child { border-bottom: 0; }
  .token-section > header { display: flex; align-items: flex-start; gap: 10px; margin-bottom: 11px; }
  .section-icon { display: grid; flex: 0 0 auto; width: 34px; height: 34px; place-items: center; border-radius: 8px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .section-kicker { color: var(--wb-accent-strong); font-size: 11px; font-weight: 800; letter-spacing: .05em; text-transform: uppercase; }
  .token-section h2 { margin: 2px 0 0; color: var(--text-strong); font-size: 15px; }
  .token-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 7px; }
  .palette-section .token-grid { grid-template-columns: repeat(auto-fill, minmax(178px, 1fr)); }
  .token-card { position: relative; min-width: 0; min-height: 64px; border: 1px solid var(--wb-border-subtle); border-radius: 8px; background: var(--wb-surface-chrome); }
  .token-card.invalid { border-color: color-mix(in srgb, var(--danger, #e44) 45%, var(--wb-border-subtle)); }
  .token-select { display: grid; grid-template-columns: 48px minmax(0, 1fr); align-items: center; gap: 9px; width: 100%; min-height: 62px; padding: 6px 8px 6px 7px; border: 0; color: inherit; background: transparent; text-align: left; }
  .token-preview { display: grid; width: 46px; height: 48px; overflow: hidden; place-items: center; border: 1px solid var(--wb-border-subtle); border-radius: 7px; color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .token-preview.color { background: var(--design-token-preview, var(--wb-surface-document)); }
  .type-sample { color: var(--text-strong); font-size: 25px; line-height: 1; }
  .type-sample.family { font-family: var(--design-token-preview, inherit); }
  .type-sample.size { font-size: var(--design-token-preview, 24px); }
  .type-sample.weight { font-weight: var(--design-token-preview, 600); }
  .type-sample.leading { font-size: 14px; line-height: var(--design-token-preview, 1.3); }
  .type-sample.tracking { letter-spacing: var(--design-token-preview, normal); }
  .spacing-track { width: 34px; height: 5px; overflow: hidden; border-radius: 99px; background: var(--wb-border-subtle); }
  .spacing-track > span { display: block; width: min(var(--design-token-preview, 50%), 100%); min-width: 5px; height: 100%; border-radius: inherit; background: var(--wb-accent); }
  .radius-sample { width: 31px; height: 31px; border: 2px solid var(--wb-accent); border-radius: var(--design-token-preview, 6px); background: var(--wb-accent-soft); }
  .shadow-sample { width: 29px; height: 29px; border-radius: 6px; background: var(--wb-surface-document); box-shadow: var(--design-token-preview, 0 3px 8px #0002); }
  .transition-sample { position: relative; width: 32px; height: 4px; border-radius: 99px; background: var(--wb-border-subtle); }
  .transition-sample > span { position: absolute; top: -4px; left: 0; width: 12px; height: 12px; border-radius: 50%; background: var(--wb-accent); }
  .breakpoint-sample { width: 31px; height: 23px; border: 2px solid var(--wb-accent); border-radius: 4px; }
  .layer-sample { position: relative; width: 31px; height: 29px; }
  .layer-sample i { position: absolute; width: 22px; height: 12px; border: 1px solid var(--wb-accent); border-radius: 3px; background: var(--wb-surface-document); }
  .layer-sample i:nth-child(1) { left: 0; bottom: 0; }
  .layer-sample i:nth-child(2) { left: 4px; bottom: 6px; }
  .layer-sample i:nth-child(3) { left: 8px; bottom: 12px; }
  .value-sample { max-width: 40px; overflow: hidden; font-family: var(--font-mono); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .token-copy { display: grid; min-width: 0; gap: 2px; }
  .token-copy strong, .token-copy small, .token-copy code, .resolved-value { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .token-copy strong { color: var(--text-strong); font-size: 11px; }
  .token-copy small { color: var(--wb-accent-strong); font-size: 11px; font-weight: 700; text-transform: uppercase; }
  .token-copy code, .resolved-value { color: var(--wb-text-muted); font-size: 11px; }
  .resolved-value { display: block; }
  .catalog-state { display: flex; min-height: 180px; align-items: center; justify-content: center; gap: 7px; padding: 20px; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .catalog-state.error { color: var(--danger-strong, #b42318); }
  button:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  button { cursor: pointer; }

</style>
