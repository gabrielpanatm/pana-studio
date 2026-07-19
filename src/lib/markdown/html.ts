function escapeHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function inlineMarkdownToHtml(value: string) {
  return escapeHtml(value)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\*([^*]+)\*/g, "<em>$1</em>")
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');
}

export function markdownBodyToHtml(markdown: string): string {
  const lines = markdown.replace(/\r\n/g, "\n").split("\n");
  const html: string[] = [];
  let paragraph: string[] = [];
  let list: { type: "ul" | "ol"; items: string[] } | null = null;
  let code: { lang: string; lines: string[] } | null = null;

  function flushParagraph() {
    if (paragraph.length === 0) return;
    html.push(`<p>${inlineMarkdownToHtml(paragraph.join(" "))}</p>`);
    paragraph = [];
  }

  function flushList() {
    if (!list) return;
    html.push(`<${list.type}>${list.items.map((item) => `<li>${inlineMarkdownToHtml(item)}</li>`).join("")}</${list.type}>`);
    list = null;
  }

  for (const line of lines) {
    const codeFence = line.match(/^```(.*)$/);
    if (codeFence) {
      if (code) {
        html.push(`<pre><code data-lang="${escapeHtml(code.lang)}">${escapeHtml(code.lines.join("\n"))}</code></pre>`);
        code = null;
      } else {
        flushParagraph();
        flushList();
        code = { lang: codeFence[1].trim(), lines: [] };
      }
      continue;
    }

    if (code) {
      code.lines.push(line);
      continue;
    }

    if (!line.trim()) {
      flushParagraph();
      flushList();
      continue;
    }

    const heading = line.match(/^(#{1,3})\s+(.+)$/);
    if (heading) {
      flushParagraph();
      flushList();
      html.push(`<h${heading[1].length}>${inlineMarkdownToHtml(heading[2])}</h${heading[1].length}>`);
      continue;
    }

    if (/^---+$/.test(line.trim())) {
      flushParagraph();
      flushList();
      html.push("<hr>");
      continue;
    }

    const quote = line.match(/^>\s+(.+)$/);
    if (quote) {
      flushParagraph();
      flushList();
      html.push(`<blockquote>${inlineMarkdownToHtml(quote[1])}</blockquote>`);
      continue;
    }

    const unordered = line.match(/^[-*]\s+(.+)$/);
    if (unordered) {
      flushParagraph();
      if (list?.type !== "ul") flushList();
      list ??= { type: "ul", items: [] };
      list.items.push(unordered[1]);
      continue;
    }

    const ordered = line.match(/^\d+\.\s+(.+)$/);
    if (ordered) {
      flushParagraph();
      if (list?.type !== "ol") flushList();
      list ??= { type: "ol", items: [] };
      list.items.push(ordered[1]);
      continue;
    }

    paragraph.push(line.trim());
  }

  flushParagraph();
  flushList();
  if (code) html.push(`<pre><code data-lang="${escapeHtml(code.lang)}">${escapeHtml(code.lines.join("\n"))}</code></pre>`);
  return html.join("\n") || "<p><br></p>";
}

function inlineHtmlToMarkdown(value: string) {
  const wrapper = document.createElement("div");
  wrapper.innerHTML = value;

  function walk(node: Node): string {
    if (node.nodeType === Node.TEXT_NODE) return node.textContent ?? "";
    if (!(node instanceof HTMLElement)) return "";
    const content = Array.from(node.childNodes).map(walk).join("");
    if (node.tagName === "STRONG" || node.tagName === "B") return `**${content}**`;
    if (node.tagName === "EM" || node.tagName === "I") return `*${content}*`;
    if (node.tagName === "CODE") return `\`${content}\``;
    if (node.tagName === "A") return `[${content}](${node.getAttribute("href") ?? ""})`;
    if (node.tagName === "BR") return "\n";
    return content;
  }

  return Array.from(wrapper.childNodes).map(walk).join("").trim();
}

export function htmlToMarkdownBody(html: string): string {
  const wrapper = document.createElement("div");
  wrapper.innerHTML = html;
  const blocks: string[] = [];

  for (const node of Array.from(wrapper.childNodes)) {
    if (!(node instanceof HTMLElement)) {
      const text = (node.textContent ?? "").trim();
      if (text) blocks.push(text);
      continue;
    }

    const tag = node.tagName;
    if (/^H[1-6]$/.test(tag)) {
      const level = Math.min(3, Number(tag.slice(1)));
      blocks.push(`${"#".repeat(level)} ${inlineHtmlToMarkdown(node.innerHTML)}`);
    } else if (tag === "P" || tag === "DIV") {
      const text = inlineHtmlToMarkdown(node.innerHTML);
      if (text) blocks.push(text);
    } else if (tag === "BLOCKQUOTE") {
      const text = inlineHtmlToMarkdown(node.innerHTML);
      if (text) blocks.push(`> ${text}`);
    } else if (tag === "UL" || tag === "OL") {
      const ordered = tag === "OL";
      const items = Array.from(node.querySelectorAll(":scope > li"))
        .map((item, index) => `${ordered ? `${index + 1}.` : "-"} ${inlineHtmlToMarkdown(item.innerHTML)}`);
      if (items.length) blocks.push(items.join("\n"));
    } else if (tag === "PRE") {
      const code = node.querySelector("code");
      const lang = code?.getAttribute("data-lang") ?? "";
      blocks.push(`\`\`\`${lang}\n${code?.textContent ?? node.textContent ?? ""}\n\`\`\``);
    } else if (tag === "HR") {
      blocks.push("---");
    } else {
      const text = inlineHtmlToMarkdown(node.innerHTML);
      if (text) blocks.push(text);
    }
  }

  return `${blocks.join("\n\n").trim()}\n`;
}
