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
  | "draft"
  | "seoTitle"
  | "seoDescription"
  | "canonicalUrl"
  | "robots"
  | "ogTitle"
  | "ogDescription"
  | "ogImage"
  | "ogType"
  | "tags"
  | "categories";

export type PageFrontmatterValues = Omit<Record<PageFrontmatterField, string>, "draft"> & {
  draft: boolean;
};

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
  draft: false,
  seoTitle: "",
  seoDescription: "",
  canonicalUrl: "",
  robots: "",
  ogTitle: "",
  ogDescription: "",
  ogImage: "",
  ogType: "",
  tags: "",
  categories: "",
};

const fieldToTomlKey: Record<PageFrontmatterField, string> = {
  title: "title",
  description: "description",
  date: "date",
  template: "template",
  slug: "slug",
  weight: "weight",
  draft: "draft",
  seoTitle: "extra.seo_title",
  seoDescription: "extra.seo_description",
  canonicalUrl: "extra.canonical_url",
  robots: "extra.robots",
  ogTitle: "extra.og_title",
  ogDescription: "extra.og_description",
  ogImage: "extra.og_image",
  ogType: "extra.og_type",
  tags: "taxonomies.tags",
  categories: "taxonomies.categories",
};

const tomlArrayKeys = new Set(["taxonomies.tags", "taxonomies.categories"]);

function tomlString(value: string) {
  return `"${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
}

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

function readTomlValue(frontmatter: string, key: string) {
  const escaped = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = frontmatter.match(new RegExp(`(?:^|\\s)${escaped}\\s*=\\s*(\\[[^\\]]*\\]|"[^"]*"|'[^']*'|[^\\s]+)`, "m"));
  return match ? parseTomlScalar(match[1]) : undefined;
}

function replaceOrAppendTomlValue(frontmatter: string, key: string, value: string | boolean) {
  const rendered = typeof value === "boolean"
    ? String(value)
    : tomlArrayKeys.has(key)
      ? `[${value.split(",").map((entry) => entry.trim()).filter(Boolean).map(tomlString).join(", ")}]`
      : tomlString(value);
  const line = `${key} = ${rendered}`;
  const escaped = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp(`^\\s*${escaped}\\s*=\\s*.+$`, "m");
  if (pattern.test(frontmatter)) return frontmatter.replace(pattern, line);
  return `${frontmatter.trimEnd()}\n${line}`.trimStart();
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
    if (typeof values[field] === "boolean") {
      values[field] = Boolean(value) as never;
    } else if (typeof value === "string") {
      values[field] = value as never;
    }
  }
  return { kind: "toml", values };
}

export function updatePageFrontmatter(source: string, values: PageFrontmatterValues): string {
  const parts = splitMarkdownFrontmatter(source);
  const marker = parts.marker || "+++";
  let frontmatter = marker === "---" ? "" : parts.frontmatter;

  for (const [field, key] of Object.entries(fieldToTomlKey) as Array<[PageFrontmatterField, string]>) {
    const value = values[field];
    if (typeof value === "boolean" || String(value).trim()) {
      frontmatter = replaceOrAppendTomlValue(frontmatter, key, value);
    }
  }

  return joinMarkdownFrontmatter({
    marker: "+++",
    frontmatter,
    body: parts.marker ? parts.body : source,
  });
}
