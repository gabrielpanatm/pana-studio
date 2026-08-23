import {
  FluentBundle,
  FluentResource,
  type FluentVariable,
} from "@fluent/bundle";

export type FluentLocaleCatalog = {
  readonly manifest: {
    readonly locale: string;
    readonly nativeName: string;
    readonly direction: "ltr" | "rtl";
    readonly contributors: readonly string[];
  };
  readonly resources: Readonly<Record<string, string>>;
};

export type FluentLocaleCatalogs = Readonly<Record<string, FluentLocaleCatalog>>;

export class FluentCatalogRuntime {
  locale: string;
  direction: "ltr" | "rtl";
  readonly baseLocale: string;
  private readonly catalogs: Record<string, FluentLocaleCatalog>;
  private readonly onFormatError: (
    locale: string,
    id: string,
    errors: Error[],
  ) => void;
  private readonly bundles = new Map<string, FluentBundle>();

  constructor(
    catalogs: FluentLocaleCatalogs,
    baseLocale: string,
    onFormatError: (
      locale: string,
      id: string,
      errors: Error[],
    ) => void = () => {},
  ) {
    this.catalogs = { ...catalogs };
    this.baseLocale = baseLocale;
    this.onFormatError = onFormatError;
    const initialLocale = this.hasLocale(baseLocale)
      ? baseLocale
      : Object.keys(this.catalogs)[0];
    if (!initialLocale) {
      throw new Error("Fluent runtime requires at least one locale catalog");
    }
    this.locale = initialLocale;
    this.direction = this.catalogs[initialLocale].manifest.direction;
  }

  setLocale(locale: string) {
    const effective = this.hasLocale(locale)
      ? locale
      : this.hasLocale(this.baseLocale)
      ? this.baseLocale
      : this.locale;
    this.locale = effective;
    this.direction = this.catalogs[effective].manifest.direction;
    return { locale: this.locale, direction: this.direction };
  }

  installCatalog(catalog: FluentLocaleCatalog) {
    const locale = catalog.manifest.locale;
    if (!locale.trim()) throw new Error("Fluent catalog locale is empty");
    this.catalogs[locale] = catalog;
    this.bundles.delete(locale);
  }

  format(
    id: string,
    arguments_: Record<string, FluentVariable> | null = null,
  ): string {
    const selected = this.formatFromBundle(this.locale, id, arguments_);
    if (selected !== null) return selected;
    if (this.locale !== this.baseLocale && this.hasLocale(this.baseLocale)) {
      const fallback = this.formatFromBundle(this.baseLocale, id, arguments_);
      if (fallback !== null) return fallback;
    }
    return "[translation unavailable]";
  }

  nativeName(locale: string) {
    return this.hasLocale(locale)
      ? this.catalogs[locale].manifest.nativeName
      : locale;
  }

  formatNumber(value: number, options?: Intl.NumberFormatOptions) {
    return new Intl.NumberFormat(this.locale, options).format(value);
  }

  formatDate(
    value: Date | number,
    options?: Intl.DateTimeFormatOptions,
  ) {
    return new Intl.DateTimeFormat(this.locale, options).format(value);
  }

  compare(left: string, right: string, options?: Intl.CollatorOptions) {
    return new Intl.Collator(this.locale, options).compare(left, right);
  }

  hasLocale(locale: string) {
    return Object.prototype.hasOwnProperty.call(this.catalogs, locale);
  }

  hasMessage(id: string) {
    return Boolean(this.bundle(this.locale).getMessage(id));
  }

  private formatFromBundle(
    locale: string,
    id: string,
    arguments_: Record<string, FluentVariable> | null,
  ) {
    const bundle = this.bundle(locale);
    const message = bundle.getMessage(id);
    if (!message?.value) return null;
    const errors: Error[] = [];
    const formatted = bundle.formatPattern(
      message.value,
      arguments_ ?? undefined,
      errors,
    );
    if (errors.length > 0) {
      this.onFormatError(locale, id, errors);
      return null;
    }
    return formatted || null;
  }

  private bundle(locale: string) {
    const cached = this.bundles.get(locale);
    if (cached) return cached;
    const catalog = this.catalogs[locale];
    if (!catalog) throw new Error(`Unknown Fluent locale ${locale}`);
    const bundle = new FluentBundle(locale, { useIsolating: true });
    for (const [domain, source] of Object.entries(catalog.resources)) {
      const errors = bundle.addResource(new FluentResource(source));
      if (errors.length > 0) {
        throw new Error(
          `Fluent catalog ${locale}/${domain} is invalid: ${errors.join("; ")}`,
        );
      }
    }
    this.bundles.set(locale, bundle);
    return bundle;
  }
}
