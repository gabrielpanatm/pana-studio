import type {
  DesignTokenCatalogSnapshot,
  ThemeStyleCatalogSnapshot,
} from "$lib/css/design-system-contract";
import {
  readDesignTokenCatalog,
  readThemeStyleCatalog,
} from "$lib/css/io";
import type { FileBufferRequestIdentity } from "$lib/project/workspace-contract";
import { t } from "$lib/i18n/runtime.svelte";
import { errorMessage } from "$lib/util";

export type DesignCatalogAuthority = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number | null;
}>;

type WorkspaceCatalogSnapshot = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
}>;

type CatalogLoader<T extends WorkspaceCatalogSnapshot> = (
  identity: FileBufferRequestIdentity,
  workspaceRevision: number,
) => Promise<T>;

class RevisionedCatalogState<T extends WorkspaceCatalogSnapshot> {
  snapshot = $state<T | null>(null);
  loading = $state(false);
  error = $state("");

  private request: Promise<T | null> | null = null;
  private requestKey = "";
  private serial = 0;
  private readonly authority: () => DesignCatalogAuthority;
  private readonly load: CatalogLoader<T>;

  constructor(
    authority: () => DesignCatalogAuthority,
    load: CatalogLoader<T>,
  ) {
    this.authority = authority;
    this.load = load;
  }

  reset() {
    this.serial += 1;
    this.request = null;
    this.requestKey = "";
    this.snapshot = null;
    this.loading = false;
    this.error = "";
  }

  async refresh(force = false): Promise<T | null> {
    const authority = this.authority();
    const projectRoot = authority.projectRoot.trim();
    const runtimeSessionId = authority.runtimeSessionId.trim();
    const workspaceRevision = authority.workspaceRevision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === null) {
      this.reset();
      return null;
    }

    const requestKey = `${projectRoot}\u0000${runtimeSessionId}\u0000${workspaceRevision}`;
    const current = this.snapshot;
    if (
      !force
      && current?.projectRoot === projectRoot
      && current.runtimeSessionId === runtimeSessionId
      && current.workspaceRevision === workspaceRevision
    ) return current;
    if (!force && this.request && this.requestKey === requestKey) return await this.request;

    const serial = ++this.serial;
    this.requestKey = requestKey;
    this.loading = true;
    this.error = "";
    const request = (async () => {
      try {
        const snapshot = await this.load(
          { expectedProjectRoot: projectRoot, expectedSessionId: runtimeSessionId },
          workspaceRevision,
        );
        if (!this.matches(serial, projectRoot, runtimeSessionId, workspaceRevision)) return null;
        if (
          snapshot.projectRoot !== projectRoot
          || snapshot.runtimeSessionId !== runtimeSessionId
          || snapshot.workspaceRevision !== workspaceRevision
        ) throw new Error(t("io-workspace-catalog-revision-mismatch", {
          resource: t("workbench-design-system"),
          actual: snapshot.workspaceRevision,
          expected: workspaceRevision,
        }));
        this.snapshot = snapshot;
        return snapshot;
      } catch (cause) {
        if (serial !== this.serial) return null;
        this.error = errorMessage(cause);
        return null;
      } finally {
        if (serial === this.serial) {
          this.loading = false;
          this.request = null;
          this.requestKey = "";
        }
      }
    })();
    this.request = request;
    return await request;
  }

  private matches(
    serial: number,
    projectRoot: string,
    runtimeSessionId: string,
    workspaceRevision: number,
  ) {
    const latest = this.authority();
    return serial === this.serial
      && latest.projectRoot === projectRoot
      && latest.runtimeSessionId === runtimeSessionId
      && latest.workspaceRevision === workspaceRevision;
  }
}

export class DesignTokenCatalogState extends RevisionedCatalogState<DesignTokenCatalogSnapshot> {
  constructor(
    authority: () => DesignCatalogAuthority,
    load: CatalogLoader<DesignTokenCatalogSnapshot> = readDesignTokenCatalog,
  ) {
    super(authority, load);
  }
}

export class ThemeStyleCatalogState extends RevisionedCatalogState<ThemeStyleCatalogSnapshot> {
  constructor(
    authority: () => DesignCatalogAuthority,
    load: CatalogLoader<ThemeStyleCatalogSnapshot> = readThemeStyleCatalog,
  ) {
    super(authority, load);
  }
}
