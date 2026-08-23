import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type {
  PageFrontmatterField,
  PageFrontmatterMutationValue,
} from "$lib/markdown/frontmatter";
import {
  readProjectFile,
} from "$lib/project/io/workspace";
import { scannedCacheKey } from "$lib/project/files";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  updatePageFrontmatterField as updatePageFrontmatterFieldFromController,
  updatePageFrontmatterSource as updatePageFrontmatterSourceFromController,
  type PageSettingsControllerHost,
} from "$lib/state/page-settings-controller";
import { t } from "$lib/i18n/runtime.svelte";

export type PageSettingsServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  source: SourceWorkspaceState;
  status: GlobalStatusState;
  settleMutation: PageSettingsControllerHost["settleMutation"];
}>;

/** Owns Markdown frontmatter reads and Rust-authoritative mutations. */
export class PageSettingsService {
  private readonly dependencies: PageSettingsServiceDependencies;
  private readonly controller: PageSettingsControllerHost;

  constructor(dependencies: PageSettingsServiceDependencies) {
    this.dependencies = dependencies;
    this.controller = {
      context: () => ({
        projectRoot: dependencies.project.root,
        runtimeSessionId: dependencies.project.runtimeSessionId,
        workspace: dependencies.project.workspace,
        activeScannedPath: dependencies.documents.activeScannedPath,
      }),
      source: dependencies.source,
      settleMutation: dependencies.settleMutation,
      setGlobalStatus: (text, kind) => dependencies.status.set(text, kind),
    };
  }

  updateSource(relativePath: string, nextSource: string) {
    updatePageFrontmatterSourceFromController(this.controller, relativePath, nextSource);
  }

  async updateField(
    relativePath: string,
    field: PageFrontmatterField,
    value: PageFrontmatterMutationValue,
  ) {
    return await updatePageFrontmatterFieldFromController(
      this.controller,
      relativePath,
      field,
      value,
    );
  }

  async readDocument(relativePath: string) {
    const projectRoot = this.dependencies.project.root;
    const runtimeSessionId = this.dependencies.project.runtimeSessionId;
    const cacheKey = scannedCacheKey({ relativePath });
    const cached = this.dependencies.source.sourceCache[cacheKey];
    if (typeof cached === "string") return cached;
    const source = await readProjectFile(relativePath);
    if (
      this.dependencies.project.root !== projectRoot
      || this.dependencies.project.runtimeSessionId !== runtimeSessionId
    ) throw new Error(t("workbench-metadata-session-stale"));
    this.dependencies.source.sourceCache = {
      ...this.dependencies.source.sourceCache,
      [cacheKey]: source,
    };
    return source;
  }
}
