import { type FluentVariable } from "@fluent/bundle";
import { writable } from "svelte/store";
import {
  BASE_LOCALE,
  availableLocales,
  localeCatalogs,
  messageIds,
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
  nativeName: localeCatalogs[locale].manifest.nativeName,
  direction: localeCatalogs[locale].manifest.direction,
  contributors: localeCatalogs[locale].manifest.contributors,
}));

class LocalizationRuntime {
  locale = $state<AvailableLocale>(BASE_LOCALE);
  direction = $state<"ltr" | "rtl">("ltr");
  revision = $state(0);
  private core = new FluentCatalogRuntime(
    localeCatalogs,
    BASE_LOCALE,
    (locale, id, errors) => console.error(`[i18n] ${locale}/${id}`, errors),
  );

  setLocale(locale: string) {
    const { locale: effective, direction } = this.core.setLocale(locale);
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
    return this.core.format(id, arguments_);
  }

  nativeName(locale: string) {
    return this.core.nativeName(locale);
  }

  formatNumber(value: number, options?: Intl.NumberFormatOptions) {
    this.revision;
    return this.core.formatNumber(value, options);
  }

  formatDate(value: Date | number, options?: Intl.DateTimeFormatOptions) {
    this.revision;
    return this.core.formatDate(value, options);
  }

  compare(left: string, right: string, options?: Intl.CollatorOptions) {
    this.revision;
    return this.core.compare(left, right, options);
  }
}

export function isMessageId(id: string): id is MessageId {
  return (messageIds as readonly string[]).includes(id);
}

export const l10n = new LocalizationRuntime();

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
