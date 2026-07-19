import type { SourceGraphTemplate } from "$lib/types";
import { runInPreviewStructuralLane } from "$lib/kernel/preview-structural-lane";
import {
  createArchiveStructure,
  createPageStructure,
  createReusablePartial,
  createSingleStructure,
  includePartialInTemplate,
  partialTemplateName,
  templateWriteBaseForTarget,
  type PartialPreset,
  type SiteStructureSessionHost,
} from "$lib/source-graph/template-actions";

export type { PartialPreset };

export type WorkspaceTemplateActionResult = {
  message: string;
  singleSectionSlug?: string;
};

export type WorkspaceTemplateActionContext = {
  targetTemplate?: SourceGraphTemplate | null;
  activeTheme?: string | null;
};

type IncludeWriteResult = {
  changed: boolean;
  includeChanged: boolean;
  partialCreated: boolean;
  templateFile: string;
  reason: string;
};

export async function createPartialWorkspaceAction(
  host: SiteStructureSessionHost,
  options: {
    partialName: string;
    partialPreset: PartialPreset;
    context: WorkspaceTemplateActionContext;
  },
): Promise<WorkspaceTemplateActionResult | undefined> {
  const input = {
    partialName: options.partialName,
    partialPreset: options.partialPreset,
    context: captureWorkspaceContext(options.context),
  };
  return runInPreviewStructuralLane(host, async (lease) => {
    const result = await createReusablePartial(
      host,
      lease,
      input.partialName,
      input.partialPreset,
      input.context.targetTemplate,
      input.context.activeTheme,
    );

    return {
      message: result.created
        ? `Partial creat în ${result.origin === "theme" ? `theme ${result.themeName}` : "local"}: ${result.path}`
        : `Partialul există deja: ${result.path}`,
    };
  });
}

export async function createPageWorkspaceAction(
  host: SiteStructureSessionHost,
  options: {
    pageTitle: string;
    pageSlug: string;
    pageTemplateName: string;
    pageDraft: boolean;
    context: WorkspaceTemplateActionContext;
  },
): Promise<WorkspaceTemplateActionResult | undefined> {
  const input = {
    pageTitle: options.pageTitle,
    pageSlug: options.pageSlug,
    pageTemplateName: options.pageTemplateName,
    pageDraft: options.pageDraft,
    context: captureWorkspaceContext(options.context),
  };
  return runInPreviewStructuralLane(host, async (lease) => {
    const result = await createPageStructure(host, lease, {
      title: input.pageTitle,
      slug: input.pageSlug,
      pageTemplateName: input.pageTemplateName,
      draft: input.pageDraft,
      targetTemplate: input.context.targetTemplate,
      activeTheme: input.context.activeTheme,
    });

    const templateCreated = result.created.includes(result.templatePath);
    const suffix = templateCreated ? ` + ${result.templatePath}` : "";

    return {
      message: result.created.length
        ? `Pagină creată: ${result.contentPath}${suffix}`
        : `Pagina exista deja: ${result.contentPath}`,
    };
  });
}

export async function createArchiveWorkspaceAction(
  host: SiteStructureSessionHost,
  options: {
    archiveTitle: string;
    archiveSlug: string;
    archiveTemplateName: string;
    context: WorkspaceTemplateActionContext;
  },
): Promise<WorkspaceTemplateActionResult | undefined> {
  const input = {
    archiveTitle: options.archiveTitle,
    archiveSlug: options.archiveSlug,
    archiveTemplateName: options.archiveTemplateName,
    context: captureWorkspaceContext(options.context),
  };
  return runInPreviewStructuralLane(host, async (lease) => {
    const result = await createArchiveStructure(host, lease, {
      title: input.archiveTitle,
      slug: input.archiveSlug,
      archiveTemplateName: input.archiveTemplateName,
      targetTemplate: input.context.targetTemplate,
      activeTheme: input.context.activeTheme,
    });

    return {
      singleSectionSlug: result.slug,
      message: result.created.length
        ? `Arhivă creată: ${result.contentPath} + ${result.templatePath}`
        : `Arhiva exista deja: ${result.contentPath}`,
    };
  });
}

export async function createSingleWorkspaceAction(
  host: SiteStructureSessionHost,
  options: {
    singleSectionSlug: string;
    singleTitle: string;
    singleSlug: string;
    singleTemplateName: string;
    context: WorkspaceTemplateActionContext;
  },
): Promise<WorkspaceTemplateActionResult | undefined> {
  const input = {
    singleSectionSlug: options.singleSectionSlug,
    singleTitle: options.singleTitle,
    singleSlug: options.singleSlug,
    singleTemplateName: options.singleTemplateName,
    context: captureWorkspaceContext(options.context),
  };
  return runInPreviewStructuralLane(host, async (lease) => {
    const result = await createSingleStructure(host, lease, {
      sectionSlug: input.singleSectionSlug,
      title: input.singleTitle,
      slug: input.singleSlug,
      singleTemplateName: input.singleTemplateName,
      targetTemplate: input.context.targetTemplate,
      activeTheme: input.context.activeTheme,
    });

    return {
      message: result.created.length
        ? `Single creat: ${result.itemPath} + ${result.templatePath}`
        : `Single-ul exista deja: ${result.itemPath}`,
    };
  });
}

export async function includePartialWorkspaceAction(
  host: SiteStructureSessionHost,
  options: {
    partialName: string;
    partialPreset: PartialPreset;
    targetTemplate: SourceGraphTemplate;
    activeTheme?: string | null;
  },
): Promise<WorkspaceTemplateActionResult | undefined> {
  const input = {
    partialName: options.partialName,
    partialPreset: options.partialPreset,
    targetTemplate: captureSourceGraphTemplate(options.targetTemplate),
    activeTheme: options.activeTheme ?? null,
  };
  const templateName = partialTemplateName(input.partialName);
  if (!templateName) throw new Error("Numele partialului este invalid.");
  const base = templateWriteBaseForTarget(input.targetTemplate, input.activeTheme);

  return runInPreviewStructuralLane(host, async (lease) => {
    const result = await includePartialInTemplate(
      host,
      lease,
      input.targetTemplate,
      templateName,
      {
        name: input.partialName,
        preset: input.partialPreset,
        targetOrigin: base.origin,
        targetThemeName: base.themeName,
      },
    );
    return { message: includeResultMessage(templateName, result) };
  });
}

function includeResultMessage(templateName: string, result: IncludeWriteResult) {
  if (result.partialCreated && result.includeChanged) {
    return `${templateName} a fost creat și inclus atomic în ${result.templateFile}. ${result.reason}`;
  }
  if (result.partialCreated) {
    return `${templateName} a fost creat atomic pentru ${result.templateFile}. ${result.reason}`;
  }
  return result.changed
    ? `${templateName} inclus în ${result.templateFile}. ${result.reason}`
    : `${templateName} era deja inclus în ${result.templateFile}.`;
}

function captureWorkspaceContext(
  context: WorkspaceTemplateActionContext,
): WorkspaceTemplateActionContext {
  return Object.freeze({
    targetTemplate: context.targetTemplate
      ? captureSourceGraphTemplate(context.targetTemplate)
      : null,
    activeTheme: context.activeTheme ?? null,
  });
}

function captureSourceGraphTemplate(template: SourceGraphTemplate): SourceGraphTemplate {
  return Object.freeze({
    ...template,
    includes: Object.freeze([...template.includes]) as unknown as string[],
    imports: Object.freeze([...template.imports]) as unknown as string[],
    getPages: Object.freeze([...template.getPages]) as unknown as string[],
    getSections: Object.freeze([...template.getSections]) as unknown as string[],
    internalLinks: Object.freeze([...template.internalLinks]) as unknown as string[],
    assetUrls: Object.freeze([...template.assetUrls]) as unknown as string[],
    assetHashes: Object.freeze([...template.assetHashes]) as unknown as string[],
    dataLoads: Object.freeze([...template.dataLoads]) as unknown as string[],
    imageMetadata: Object.freeze([...template.imageMetadata]) as unknown as string[],
    imageResizes: Object.freeze([...template.imageResizes]) as unknown as string[],
    blocks: Object.freeze([...template.blocks]) as unknown as string[],
    macros: Object.freeze([...template.macros]) as unknown as string[],
  });
}
