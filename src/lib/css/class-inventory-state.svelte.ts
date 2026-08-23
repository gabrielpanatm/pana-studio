import type { DesignClassInventorySnapshot } from "$lib/css/design-system-contract";
import { readDesignClassInventory } from "$lib/css/io";
import { t } from "$lib/i18n/runtime.svelte";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import { errorMessage } from "$lib/util";

export type DesignClassInventoryAuthority = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  workspace: ProjectWorkspaceSnapshot | null;
}>;

/** Owns deduplication and stale-response rejection for the design-class inventory. */
export class DesignClassInventoryState {
  snapshot = $state<DesignClassInventorySnapshot | null>(null);
  loading = $state(false);
  error = $state("");

  private request: Promise<DesignClassInventorySnapshot | null> | null = null;
  private requestKey = "";
  private serial = 0;
  private readonly authority: () => DesignClassInventoryAuthority;

  constructor(authority: () => DesignClassInventoryAuthority) {
    this.authority = authority;
  }

  reset() {
    this.serial += 1;
    this.request = null;
    this.requestKey = "";
    this.snapshot = null;
    this.loading = false;
    this.error = "";
  }

  async refresh(force = false): Promise<DesignClassInventorySnapshot | null> {
    const authority = this.authority();
    const projectRoot = authority.projectRoot.trim();
    const runtimeSessionId = authority.runtimeSessionId.trim();
    const workspaceRevision = authority.workspace?.revision ?? null;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === null) {
      this.snapshot = null;
      this.error = "";
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
        const snapshot = await readDesignClassInventory();
        const latest = this.authority();
        if (
          serial !== this.serial
          || latest.projectRoot !== projectRoot
          || latest.runtimeSessionId !== runtimeSessionId
          || latest.workspace?.revision !== workspaceRevision
        ) return null;
        if (
          snapshot.projectRoot !== projectRoot
          || snapshot.runtimeSessionId !== runtimeSessionId
          || snapshot.workspaceRevision !== workspaceRevision
        ) throw new Error(t("workbench-class-inventory-revision-mismatch"));
        this.snapshot = snapshot;
        return snapshot;
      } catch (error) {
        if (serial !== this.serial) return null;
        this.error = errorMessage(error);
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
}
