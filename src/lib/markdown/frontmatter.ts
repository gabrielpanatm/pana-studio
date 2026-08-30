export type MarkdownParts = {
  marker: "---" | "+++" | "";
  frontmatter: string;
  body: string;
};

export type PageFrontmatterField =
  | "title"
  | "description"
  | "date"
  | "template"
  | "slug"
  | "weight"
  | "paginateBy"
  | "draft"
  | "hidden"
  | "includeInFeeds"
  | "seoTitle"
  | "seoDescription"
  | "canonicalUrl"
  | "robots"
  | "ogTitle"
  | "ogDescription"
  | "ogImage"
  | "ogType";

export type PageFrontmatterValues = Omit<Record<PageFrontmatterField, string>, "draft" | "hidden" | "includeInFeeds"> & {
  draft: boolean;
  hidden: "inherit" | "hidden" | "visible";
  includeInFeeds: boolean;
};

export type PageFrontmatterMutationValue =
  | { kind: "string"; value: string }
  | { kind: "integer"; value: number }
  | { kind: "boolean"; value: boolean }
  | { kind: "empty" };

export type PageFrontmatterParseResult = {
  kind: "toml" | "yaml" | "none";
  values: PageFrontmatterValues;
};

const defaultPageFrontmatterValues: PageFrontmatterValues = {
  title: "",
  description: "",
  date: "",
  template: "",
  slug: "",
  weight: "",
  paginateBy: "",
  draft: false,
  hidden: "inherit",
  includeInFeeds: true,
  seoTitle: "",
  seoDescription: "",
  canonicalUrl: "",
  robots: "",
  ogTitle: "",
  ogDescription: "",
  ogImage: "",
  ogType: "",
};

const fieldToTomlKey: Record<PageFrontmatterField, string> = {
  title: "title",
  description: "description",
  date: "date",
  template: "template",
  slug: "slug",
  weight: "weight",
  paginateBy: "paginate_by",
  draft: "draft",
  hidden: "hidden",
  includeInFeeds: "include_in_feeds",
  seoTitle: "extra.seo_title",
  seoDescription: "extra.seo_description",
  canonicalUrl: "extra.canonical_url",
  robots: "extra.robots",
  ogTitle: "extra.og_title",
  ogDescription: "extra.og_description",
  ogImage: "extra.og_image",
  ogType: "extra.og_type",
};

function parseTomlScalar(value: string): string | boolean {
  const trimmed = value.trim().replace(/,$/, "");
  if (trimmed === "true") return true;
  if (trimmed === "false") return false;
  if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
    return [...trimmed.matchAll(/["']((?:\\.|[^"'])*)["']/g)]
      .map((match) => match[1].replace(/\\"/g, '"').replace(/\\\\/g, "\\"))
      .join(", ");
  }
  const quoted = trimmed.match(/^["']([\s\S]*)["']$/);
  return quoted ? quoted[1].replace(/\\"/g, '"').replace(/\\\\/g, "\\") : trimmed;
}

function readTomlAssignment(frontmatter: string, key: string) {
  const escaped = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = frontmatter.match(new RegExp(`^\\s*${escaped}\\s*=\\s*(\\[[^\\]]*\\]|"[^"]*"|'[^']*'|[^\\s#]+)`, "m"));
  return match ? parseTomlScalar(match[1]) : undefined;
}

function readTomlValue(frontmatter: string, key: string) {
  const direct = readTomlAssignment(frontmatter, key);
  if (direct !== undefined || !key.includes(".")) return direct;

  const [tableName, ...nestedPath] = key.split(".");
  const nestedKey = nestedPath.join(".");
  const escapedTable = tableName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const tableMatch = new RegExp(`^\\s*\\[${escapedTable}\\]\\s*(?:#.*)?$`, "m").exec(frontmatter);
  if (!tableMatch || tableMatch.index === undefined) return undefined;
  const tableStart = tableMatch.index + tableMatch[0].length;
  const afterTable = frontmatter.slice(tableStart);
  const nextTableOffset = afterTable.search(/^\s*\[[^\]]+\]\s*(?:#.*)?$/m);
  const tableBody = nextTableOffset >= 0 ? afterTable.slice(0, nextTableOffset) : afterTable;
  return readTomlAssignment(tableBody, nestedKey);
}

export function pageFrontmatterMutationValue(
  field: PageFrontmatterField,
  value: string | boolean,
): PageFrontmatterMutationValue {
  if (field === "draft") {
    if (typeof value !== "boolean") {
      throw new Error("Starea draft trebuie să fie o valoare booleană.");
    }
    return { kind: "boolean", value };
  }
  if (field === "hidden") {
    if (value === "inherit") return { kind: "empty" };
    if (value === "hidden") return { kind: "boolean", value: true };
    if (value === "visible") return { kind: "boolean", value: false };
    throw new Error("Vizibilitatea trebuie să fie moștenită, ascunsă sau vizibilă.");
  }
  if (field === "includeInFeeds") {
    if (typeof value !== "boolean") {
      throw new Error("Includerea în feed trebuie să fie o valoare booleană.");
    }
    return value ? { kind: "empty" } : { kind: "boolean", value: false };
  }
  if (typeof value !== "string") {
    throw new Error("Valoarea acestui câmp trebuie să fie text.");
  }
  if (field === "weight") {
    if (!value.trim()) return { kind: "empty" };
    const number = Number(value);
    if (!Number.isSafeInteger(number) || number < 0) {
      throw new Error("Ordinea paginii trebuie să fie un număr întreg pozitiv sau zero.");
    }
    return { kind: "integer", value: number };
  }
  if (field === "paginateBy") {
    const number = Number(value);
    if (!value.trim() || !Number.isSafeInteger(number) || number <= 0) {
      throw new Error("Arhiva trebuie să conțină cel puțin un articol pe pagină.");
    }
    return { kind: "integer", value: number };
  }
  return value.trim() ? { kind: "string", value } : { kind: "empty" };
}

export function splitMarkdownFrontmatter(source: string): MarkdownParts {
  const normalized = source.replace(/\r\n/g, "\n");
  const inline = normalized.match(/^(\+\+\+|---)\s+([\s\S]*?)\s+\1(?:\n([\s\S]*))?$/);
  if (inline) {
    return {
      marker: inline[1] as "+++" | "---",
      frontmatter: inline[2],
      body: inline[3] ?? "",
    };
  }

  const firstLine = normalized.match(/^(\+\+\+|---)\n/);
  if (!firstLine) {
    return { marker: "", frontmatter: "", body: normalized };
  }

  const marker = firstLine[1] as "+++" | "---";
  const closing = `\n${marker}\n`;
  const closingIndex = normalized.indexOf(closing, marker.length + 1);
  if (closingIndex < 0) {
    return { marker: "", frontmatter: "", body: normalized };
  }

  return {
    marker,
    frontmatter: normalized.slice(marker.length + 1, closingIndex),
    body: normalized.slice(closingIndex + closing.length),
  };
}

export function joinMarkdownFrontmatter(parts: MarkdownParts): string {
  if (!parts.marker) return parts.body;
  return `${parts.marker}\n${parts.frontmatter.trimEnd()}\n${parts.marker}\n\n${parts.body.replace(/^\n+/, "")}`;
}

export function parsePageFrontmatter(source: string): PageFrontmatterParseResult {
  const parts = splitMarkdownFrontmatter(source);
  if (!parts.marker) return { kind: "none", values: { ...defaultPageFrontmatterValues } };
  if (parts.marker === "---") return { kind: "yaml", values: { ...defaultPageFrontmatterValues } };

  const values = { ...defaultPageFrontmatterValues };
  for (const [field, key] of Object.entries(fieldToTomlKey) as Array<[PageFrontmatterField, string]>) {
    const value = readTomlValue(parts.frontmatter, key);
    if (field === "hidden") {
      values.hidden = value === true ? "hidden" : value === false ? "visible" : "inherit";
      continue;
    }
    if (field === "includeInFeeds") {
      values.includeInFeeds = typeof value === "boolean" ? value : true;
      continue;
    }
    if (typeof values[field] === "boolean") {
      values[field] = Boolean(value) as never;
    } else if (typeof value === "string") {
      values[field] = value as never;
    }
  }
  return { kind: "toml", values };
}
