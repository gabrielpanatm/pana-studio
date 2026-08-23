import { type FluentVariable } from "@fluent/bundle";
import { writable } from "svelte/store";
import {
  BASE_LOCALE,
  availableLocales,
  isAvailableLocale,
  loadLocaleCatalog,
  localeManifests,
  type AvailableLocale,
  type MessageId,
} from "$lib/i18n/generated/catalog";
import { FluentCatalogRuntime } from "$lib/i18n/runtime-core";
import { registerDiagnosticFormatter } from "$lib/util";

export const localeRevision = writable(0);

export type LocaleOption = {
  locale: AvailableLocale;
  nativeName: string;
  direction: "ltr" | "rtl";
  contributors: readonly string[];
};

export const localeOptions: LocaleOption[] = availableLocales.map((locale) => ({
  locale,
  nativeName: localeManifests[locale].nativeName,
  direction: localeManifests[locale].direction,
  contributors: localeManifests[locale].contributors,
}));

function bootLocale(): AvailableLocale {
  if (typeof document === "undefined") return BASE_LOCALE;
  const locale = document.documentElement.dataset.panaLocale;
  return locale && isAvailableLocale(locale) ? locale : BASE_LOCALE;
}

class LocalizationRuntime {
  locale = $state<AvailableLocale>(bootLocale());
  direction = $state<"ltr" | "rtl">(localeManifests[this.locale].direction);
  revision = $state(0);
  private core: FluentCatalogRuntime | null = null;
  private requestedLocale = this.locale;

  async setLocale(locale: string) {
    const requested = isAvailableLocale(locale) ? locale : BASE_LOCALE;
    this.requestedLocale = requested;
    if (!this.core?.hasLocale(requested)) {
      const catalog = await loadLocaleCatalog(requested);
      if (this.requestedLocale !== requested) return;
      if (this.core) {
        this.core.installCatalog(catalog);
      } else {
        this.core = new FluentCatalogRuntime(
          { [requested]: catalog },
          BASE_LOCALE,
          (activeLocale, id, errors) =>
            console.error(`[i18n] ${activeLocale}/${id}`, errors),
        );
      }
    }
    if (this.requestedLocale !== requested) return;
    const { locale: effective, direction } = this.core.setLocale(requested);
    if (this.locale === effective && this.direction === direction) return;
    this.locale = effective as AvailableLocale;
    this.direction = direction;
    this.revision += 1;
    localeRevision.set(this.revision);
  }

  format(
    id: MessageId,
    arguments_: Record<string, FluentVariable> | null = null,
  ): string {
    this.revision;
    return this.core?.format(id, arguments_) ?? "[translation unavailable]";
  }

  nativeName(locale: string) {
    return isAvailableLocale(locale) ? localeManifests[locale].nativeName : locale;
  }

  formatNumber(value: number, options?: Intl.NumberFormatOptions) {
    this.revision;
    return new Intl.NumberFormat(this.locale, options).format(value);
  }

  formatDate(value: Date | number, options?: Intl.DateTimeFormatOptions) {
    this.revision;
    return new Intl.DateTimeFormat(this.locale, options).format(value);
  }

  compare(left: string, right: string, options?: Intl.CollatorOptions) {
    this.revision;
    return new Intl.Collator(this.locale, options).compare(left, right);
  }

  hasMessage(id: string) {
    return this.core?.hasMessage(id) ?? false;
  }
}

export function isMessageId(id: string): id is MessageId {
  return l10n.hasMessage(id);
}

export const l10n = new LocalizationRuntime();

let initialization: Promise<void> | null = null;

export function initializeLocalization(locale: string = bootLocale()) {
  initialization ??= l10n.setLocale(locale);
  return initialization;
}

export type TranslationFunction = (
  id: MessageId,
  arguments_?: Record<string, FluentVariable> | null,
) => string;

export function legacyTranslator(_revision: number): TranslationFunction {
  return (id, arguments_ = null) => l10n.format(id, arguments_);
}

registerDiagnosticFormatter((code, arguments_) =>
  isMessageId(code) ? l10n.format(code, arguments_) : null
);

export function t(
  id: MessageId,
  arguments_: Record<string, FluentVariable> | null = null,
) {
  return l10n.format(id, arguments_);
}
