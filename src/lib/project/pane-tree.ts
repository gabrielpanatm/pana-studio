import { zolaPathWithoutThemeRoot, zolaRelativePath } from "$lib/project/files";
import type { PageSection, ProjectFile } from "$lib/types";

export type LayerNode = PageSection & { hasChildren: boolean };

export type ProjectTreeNode = {
  name: string;
  path: string;
  commandPath: string;
  isDir: boolean;
  file?: ProjectFile;
  children: ProjectTreeNode[];
};

export type FlatProjectFileNode = {
  type: "dir" | "file";
  name: string;
  path: string;
  commandPath: string;
  depth: number;
  file?: ProjectFile;
  hasChildren: boolean;
};

export type PendingCreate = {
  parentPath: string;
  commandParentPath: string;
  kind: "file" | "dir";
  depth: number;
  name: string;
};

export type CreateTarget = {
  filePath: string;
  content: string;
};

export function computeVisibleSections(sections: PageSection[], collapsed: Set<string>): LayerNode[] {
  const result: LayerNode[] = [];
  let skipUntilDepth: number | null = null;

  for (let i = 0; i < sections.length; i++) {
    const section = sections[i];
    const nextSection = sections[i + 1];
    if (skipUntilDepth !== null && section.depth <= skipUntilDepth) skipUntilDepth = null;
    if (skipUntilDepth !== null) continue;

    const hasChildren = nextSection !== undefined && nextSection.depth > section.depth;
    result.push({ ...section, hasChildren });

    if (hasChildren && collapsed.has(section.selector)) {
      skipUntilDepth = section.depth;
    }
  }

  return result;
}

export function buildProjectFileTree(files: ProjectFile[]): ProjectTreeNode[] {
  const root: ProjectTreeNode[] = [];
  const dirs = new Map<string, ProjectTreeNode>();
  const sorted = [...files].sort((a, b) => a.relativePath.localeCompare(b.relativePath));

  for (const file of sorted) {
    const displayPath = file.relativePath;
    const parts = displayPath.split("/");
    let current = root;
    let currentPath = "";

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      currentPath = currentPath ? `${currentPath}/${part}` : part;
      const isLast = i === parts.length - 1;

      if (isLast && file.kind !== "DIR") break;

      if (!dirs.has(currentPath)) {
        const dir: ProjectTreeNode = {
          name: part,
          path: currentPath,
          commandPath: currentPath,
          isDir: true,
          children: [],
        };
        dirs.set(currentPath, dir);
        current.push(dir);
      }

      current = dirs.get(currentPath)!.children;
    }

    if (file.kind === "DIR") continue;

    current.push({
      name: parts[parts.length - 1],
      path: displayPath,
      commandPath: displayPath,
      isDir: false,
      file,
      children: [],
    });
  }

  return sortProjectTree(root);
}

export function flattenVisibleProjectFiles(
  nodes: ProjectTreeNode[],
  collapsed: Set<string>,
  depth = 0,
): FlatProjectFileNode[] {
  const result: FlatProjectFileNode[] = [];

  for (const node of nodes) {
    result.push({
      type: node.isDir ? "dir" : "file",
      name: node.name,
      path: node.path,
      commandPath: node.commandPath,
      depth,
      file: node.file,
      hasChildren: node.children.length > 0,
    });

    if (node.isDir && !collapsed.has(node.path) && node.children.length > 0) {
      result.push(...flattenVisibleProjectFiles(node.children, collapsed, depth + 1));
    }
  }

  return result;
}

export function allProjectPaneFiles({
  allProjectFiles,
  scannedPages,
  scannedStyles,
  scannedTemplates,
  scannedScripts,
  scannedAssets,
}: {
  allProjectFiles: ProjectFile[];
  scannedPages: ProjectFile[];
  scannedStyles: ProjectFile[];
  scannedTemplates: ProjectFile[];
  scannedScripts: ProjectFile[];
  scannedAssets: ProjectFile[];
}): ProjectFile[] {
  return allProjectFiles.length > 0
    ? allProjectFiles
    : [...scannedPages, ...scannedStyles, ...scannedTemplates, ...scannedScripts, ...scannedAssets];
}

export function projectFileExt(name: string): string {
  return name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
}

export function initialProjectFileContent(filePath: string): string {
  const name = filePath.split("/").pop() ?? "";
  const zolaPath = zolaRelativePath(filePath);
  const logicalPath = zolaPathWithoutThemeRoot(filePath);
  const fileStem = name.replace(/\.html$/i, "").replace(/^_+/, "") || "partial";

  if (zolaPath.startsWith("content/")) {
    if (name === "_index.md") return `+++\ntitle = ""\ntemplate = "section.html"\n+++\n`;
    if (name.endsWith(".md")) return `+++\ntitle = ""\n+++\n`;
  }

  if (logicalPath.startsWith("templates/partials/") && name.endsWith(".html")) {
    return `<section class="${fileStem}">\n  <h2>${fileStem}</h2>\n</section>\n`;
  }

  if (logicalPath.startsWith("templates/macros/") && name.endsWith(".html")) {
    return `{% macro ${fileStem}() %}\n{% endmacro %}\n`;
  }

  if (zolaPath.startsWith("templates/") && name.endsWith(".html")) {
    return `{% extends "base.html" %}\n\n{% block content %}\n{% endblock %}\n`;
  }

  return "";
}

export function resolveCreateTarget(pendingCreate: PendingCreate): CreateTarget {
  const rawName = pendingCreate.name.trim();
  const parentPath = pendingCreate.commandParentPath;
  const base = parentPath ? `${parentPath}/${rawName}` : rawName;

  if (pendingCreate.kind === "dir") {
    const zolaBase = zolaRelativePath(base);
    if (zolaBase.startsWith("content") || zolaBase === "content") {
      return {
        filePath: `${base}/_index.md`,
        content: `+++\ntitle = ""\ntemplate = "section.html"\n+++\n`,
      };
    }

    return { filePath: `${base}/.gitkeep`, content: "" };
  }

  return { filePath: base, content: initialProjectFileContent(base) };
}

function sortProjectTree(nodes: ProjectTreeNode[]): ProjectTreeNode[] {
  return nodes
    .sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      return a.name.localeCompare(b.name);
    })
    .map((node) => ({ ...node, children: sortProjectTree(node.children) }));
}
