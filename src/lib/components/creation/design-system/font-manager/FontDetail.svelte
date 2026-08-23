<script lang="ts">
  import { IconAlertTriangle, IconCircleCheck, IconTrash, IconTypography } from "@tabler/icons-svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import { errorMessage } from "$lib/util";
  import type { FontManagerController } from "./controller.svelte";

  let { controller }: { controller: FontManagerController } = $props();
  const font = $derived(controller.selectedFont);
</script>

{#if font}
  <span class="detail-kicker">{t("design-font-inventory")}</span><h2>{font.family}</h2><p>{t("design-font-description")}</p>
  <div class="font-preview" aria-label={t("design-font-preview-label", { family: font.family })}><strong>{t("design-font-preview-text")}</strong><span>{controller.fontPreviewLoading ? t("design-font-preview-loading") : controller.selectedFontPreviewFile?.subfamily ?? t("design-font-preview-real")}</span></div>
  {#if controller.fontPreviewError}<p class="font-preview-error"><IconAlertTriangle size={13} /> {t("design-font-preview-error", { message: controller.fontPreviewError })}</p>{/if}
  <dl class="info-grid">
    <div><dt>{t("design-origin")}</dt><dd>{font.origin === "bundled" ? t("design-origin-bundled") : font.origin === "local" ? t("design-origin-local") : font.origin === "theme" ? font.themeName ?? t("design-origin-theme") : t("design-origin-external")}</dd></div>
    <div><dt>{t("design-files")}</dt><dd>{l10n.formatNumber(font.files.length)}</dd></div>
    <div><dt>{t("design-delivery")}</dt><dd>{font.delivery === "local" ? t("design-delivery-local") : font.delivery === "system" ? t("design-delivery-system") : font.delivery === "external" ? t("design-delivery-external") : t("design-delivery-missing")}</dd></div>
    <div><dt>{t("design-css-registration")}</dt><dd>{font.registration.registered ? font.registration.managed ? t("design-registration-managed") : t("design-registration-detected") : t("design-registration-missing")}</dd></div>
    <div><dt>{t("design-font-display-policy")}</dt><dd>{font.registration.displayModes.join(", ") || "—"}</dd></div>
    <div><dt>{t("design-font-variable")}</dt><dd>{font.files.some((file) => file.axes.length > 0) ? t("design-yes") : t("design-no")}</dd></div>
    <div><dt>{t("design-romanian-coverage")}</dt><dd>{font.romanianSupported === null ? "—" : font.romanianSupported ? t("design-yes") : t("design-no")}</dd></div>
    <div><dt>{t("design-license")}</dt><dd>{font.license.description || font.license.url ? t("design-license-metadata") : t("design-license-undeclared")}</dd></div>
  </dl>
  {#if font.directories.length}<div class="source-card"><span>{t("design-directory")}</span><code>{font.directories.join(", ")}</code></div>{/if}
  {#if font.registration.stylesheets.length}<div class="source-card"><span>{t("design-font-face-declarations")}</span><code>{font.registration.stylesheets.join(", ")}</code></div>{/if}
  <section class="font-delivery-actions" aria-labelledby="font-delivery-title"><div><span id="font-delivery-title">{t("design-browser-delivery")}</span><small>{t("design-browser-delivery-description")}</small></div><label><span>{t("design-font-display-policy")}</span><select value={font.registration.displayModes.length === 1 ? font.registration.displayModes[0] : ""} disabled={controller.mutating || !font.registration.managed} onchange={(event) => { const display = event.currentTarget.value as "auto" | "block" | "swap" | "fallback" | "optional"; if (display) void controller.changeSelectedFontDisplay(display); }}>{#if font.registration.displayModes.length !== 1}<option value="">{t("design-choose-policy")}</option>{/if}<option value="swap">{t("design-display-swap")}</option><option value="optional">{t("design-display-optional")}</option><option value="fallback">{t("design-display-fallback")}</option><option value="block">{t("design-display-block")}</option><option value="auto">{t("design-display-auto")}</option></select></label>{#if !font.registration.managed}<small>{t("design-display-managed-only")}</small>{/if}</section>
  {#if font.license.description || font.license.url}<div class="font-license"><span>{t("design-font-license-included")}</span>{#if font.license.description}<p>{font.license.description}</p>{/if}{#if font.license.url}<code>{font.license.url}</code>{/if}</div>{/if}
  <section class="font-role-actions" aria-labelledby="font-role-actions-title"><div><span id="font-role-actions-title">{t("design-use-family-for")}</span><small>{t("design-role-description")}</small></div><div>{#each controller.roles as role (role.id)}<button type="button" class:active={role.family === font.family} disabled={controller.mutating || !role.assignable || !font.registration.registered || font.delivery === "missing"} title={role.diagnostic ?? t("design-assign-role", { family: font.family, role: role.label })} onclick={() => controller.assignSelectedFontToRole(role.id)}><IconTypography size={14} /><span>{role.label}</span>{#if role.family === font.family}<IconCircleCheck size={14} />{/if}</button>{/each}</div></section>
  {#if controller.selectedFontDiagnostics.length}<div class="font-diagnostics" aria-label={t("design-font-diagnostics-label")}>{#each controller.selectedFontDiagnostics as diagnostic (`${diagnostic.code}:${diagnostic.file ?? diagnostic.family ?? "global"}`)}<p class:error={diagnostic.severity === "error"} class:warning={diagnostic.severity === "warning"}><IconAlertTriangle size={14} /><span>{errorMessage(diagnostic.messageDiagnostic)}</span></p>{/each}</div>{/if}
  {#if controller.formError}<p class="form-error font-action-error" role="alert"><IconAlertTriangle size={14} /> {controller.formError}</p>{/if}
  <div class="font-files" aria-label={t("design-family-variants-label")}>
    {#each font.files as file (file.file)}<div><span><strong>{file.declaredWeightRange ? `${file.declaredWeightRange.start}–${file.declaredWeightRange.end}` : file.declaredWeight ?? file.subfamily ?? (file.weightRange ? `${file.weightRange.start}–${file.weightRange.end}` : file.weight ?? 400)}</strong> {file.declaredStyle ?? file.style ?? "normal"}</span><small>{file.format.toUpperCase()} · {Math.max(1, Math.round(file.sizeBytes / 1024))} KB{file.textOptimized ? ` · ${t("design-exact-character-set")}` : ""}{file.axes.length ? ` · ${file.axes.map((axis) => `${axis.tag} ${axis.min}–${axis.max} (${t("design-axis-default", { value: axis.default })})`).join(" · ")}` : ""}</small><button type="button" class:active={file.preload.preloaded} disabled={controller.mutating || !font.registration.registered || (file.preload.preloaded && !file.preload.managed)} title={file.preload.templates.length ? t("design-preload-template", { templates: file.preload.templates.join(", ") }) : t("design-preload-add-help")} onclick={() => controller.toggleFontPreload(file.file, !file.preload.preloaded)}>{#if file.preload.preloaded}<IconCircleCheck size={13} />{/if}{file.preload.preloaded ? file.preload.managed ? t("design-preload-active") : t("design-preload-external") : t("design-preload")}</button></div>{/each}
  </div>
  {#if font.origin === "local"}
    <section class="font-removal" aria-labelledby="font-removal-title"><div><span id="font-removal-title">{t("design-controlled-removal")}</span><small>{t("design-removal-description")}</small></div>
      {#if controller.fontRemovalPlan}<dl><div><dt>{t("design-fonts")}</dt><dd>{l10n.formatNumber(controller.fontRemovalPlan.files.length)}</dd></div><div><dt>{t("design-stylesheets")}</dt><dd>{l10n.formatNumber(controller.fontRemovalPlan.stylesheetPaths.length)}</dd></div><div><dt>{t("design-licenses")}</dt><dd>{l10n.formatNumber(controller.fontRemovalPlan.licenseFiles.length)}</dd></div></dl>{#each controller.fontRemovalPlan.blockedReasons as reason}<p class="blocked"><IconAlertTriangle size={13} /> {reason}</p>{/each}{#each controller.fontRemovalPlan.warnings as warning}<p><IconAlertTriangle size={13} /> {warning}</p>{/each}<div class="font-removal-actions"><button type="button" disabled={controller.mutating} onclick={() => { controller.fontRemovalPlan = null; }}>{t("design-cancel")}</button><button class="danger" type="button" disabled={controller.mutating || !controller.fontRemovalPlan.changed || controller.fontRemovalPlan.blockedReasons.length > 0} onclick={() => controller.confirmSelectedFontRemoval()}><IconTrash size={13} />{controller.mutating ? t("design-removing-rust") : t("design-confirm-removal")}</button></div>
      {:else}<button type="button" disabled={controller.mutating || controller.fontRemovalPlanning} onclick={() => controller.planSelectedFontRemoval()}><IconTrash size={13} />{controller.fontRemovalPlanning ? t("design-analyzing-rust") : t("design-analyze-removal")}</button>{/if}
    </section>
  {/if}
{:else}<div class="workspace-state">{t("design-select-resource")}</div>{/if}

<style>
  .font-preview { display: grid; gap: 5px; margin-top: 11px; padding: 13px; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .font-preview strong { overflow: hidden; color: var(--text-strong); font-family: "Pana Studio Font Preview", system-ui, sans-serif; font-size: 27px; font-weight: 400; text-overflow: ellipsis; white-space: nowrap; }
  .font-preview span, .font-preview-error { color: var(--wb-text-muted); font-size: 11px; }
  .font-preview-error { display: flex; gap: 5px; color: var(--warning); }
  .source-card, .font-license, .font-delivery-actions, .font-role-actions, .font-removal { display: grid; gap: 6px; margin-top: 9px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .source-card span, .font-license span, .font-delivery-actions span, .font-role-actions span, .font-removal span { color: var(--text-strong); font-size: 12px; font-weight: 750; }
  .source-card code, .font-license code, .font-license p, .font-delivery-actions small, .font-role-actions small, .font-removal small { overflow: hidden; margin: 0; color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; text-overflow: ellipsis; }
  .font-delivery-actions label { display: grid; grid-template-columns: minmax(100px, 1fr) minmax(150px, 1.2fr); align-items: center; gap: 8px; }
  .font-delivery-actions select { height: 30px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); }
  .font-role-actions > div:last-child { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 5px; }
  .font-role-actions button { display: grid; grid-template-columns: 16px minmax(0, 1fr) 16px; align-items: center; gap: 5px; min-height: 31px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; text-align: left; }
  .font-role-actions button.active, .font-files button.active { border-color: var(--wb-accent); color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .font-diagnostics { display: grid; gap: 5px; margin-top: 9px; }
  .font-diagnostics p { display: flex; gap: 6px; margin: 0; padding: 7px 8px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-muted); font-size: 11px; }
  .font-diagnostics p.warning { color: var(--warning); } .font-diagnostics p.error { color: var(--danger); }
  .font-files { display: grid; margin-top: 10px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .font-files > div { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; column-gap: 8px; min-height: 46px; padding: 7px 9px; border-bottom: 1px solid var(--wb-border-subtle); }
  .font-files small { grid-column: 1; overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .font-files button { display: inline-flex; grid-column: 2; grid-row: 1 / span 2; align-items: center; gap: 4px; min-height: 27px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; }
  .font-removal { border-color: color-mix(in srgb, var(--danger) 30%, var(--wb-border-subtle)); }
  .font-removal dl, .font-removal-actions { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 5px; margin: 0; }
  .font-removal-actions { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .font-removal p { display: flex; gap: 5px; margin: 0; color: var(--warning); font-size: 11px; } .font-removal p.blocked { color: var(--danger); }
  .font-removal button { min-height: 30px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); }
  .font-removal button.danger { color: #fff; background: var(--danger); }
</style>
