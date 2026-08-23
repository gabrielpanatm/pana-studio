import type { FontManagerSnapshot } from "$lib/fonts/contracts";
import { getFontManager } from "$lib/fonts/io";
import type { ProjectWorkspaceIdentity } from "$lib/project/workspace-contract";
import { errorMessage } from "$lib/util";

type FontManagerLoader = (identity: ProjectWorkspaceIdentity) => Promise<FontManagerSnapshot>;

export class FontManagerState {
  snapshot = $state<FontManagerSnapshot | null>(null);
  loading = $state(false);
  error = $state("");

  private request: Promise<FontManagerSnapshot | null> | null = null;
  private requestKey = "";
  private serial = 0;
  private readonly identity: () => ProjectWorkspaceIdentity | null;
  private readonly load: FontManagerLoader;

  constructor(
    identity: () => ProjectWorkspaceIdentity | null,
    load: FontManagerLoader = getFontManager,
  ) {
    this.identity = identity;
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

  replace(snapshot: FontManagerSnapshot) {
    this.snapshot = snapshot;
    this.error = "";
  }

  async refresh(force = false): Promise<FontManagerSnapshot | null> {
    const currentIdentity = this.identity();
    if (!currentIdentity) {
      this.reset();
      return null;
    }
    const identity = { ...currentIdentity };
    const requestKey = `${identity.expectedProjectRoot}\u0000${identity.expectedSessionId}\u0000${identity.expectedRevision}`;
    if (!force && this.snapshot && this.requestKey === requestKey) return this.snapshot;
    if (!force && this.request && this.requestKey === requestKey) return await this.request;

    const serial = ++this.serial;
    this.requestKey = requestKey;
    this.loading = true;
    this.error = "";
    const request = (async () => {
      try {
        const snapshot = await this.load(identity);
        if (!this.matches(serial, identity)) return null;
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
        }
      }
    })();
    this.request = request;
    return await request;
  }

  private matches(serial: number, identity: ProjectWorkspaceIdentity) {
    const latest = this.identity();
    return serial === this.serial
      && latest?.expectedProjectRoot === identity.expectedProjectRoot
      && latest.expectedSessionId === identity.expectedSessionId
      && latest.expectedRevision === identity.expectedRevision;
  }
}
