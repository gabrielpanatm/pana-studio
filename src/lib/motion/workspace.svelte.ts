import {
  applyMotionMutation,
  getPageJsWorkspaceState,
} from "$lib/js/io";
import { emptyPageJsConfig, normalizePageJsConfig } from "$lib/js/page-config";
import { normalizePageJsTemplatePath } from "$lib/js/page-path";
import {
  createPageJsRequestIdentity,
  isPageJsRequestIdentityCurrent,
  pageJsCommandPayload,
} from "$lib/session/page-js-command-session";
import type {
  MotionAction,
  MotionInteraction,
  MotionMutation,
  MotionPageMutationReceipt,
  MotionRuntimeContract,
  PageJsConfig,
} from "$lib/js/contracts";
import { t } from "$lib/i18n/runtime.svelte";
import { errorMessage as localizedErrorMessage } from "$lib/util";

export type MotionWorkspaceLoadState = "idle" | "loading" | "ready" | "error";
export type MotionOwner = {
  kind: "template";
  templatePath: string;
};
export type MotionPreviewMode = "design" | "motion" | "interactive";
export type MotionPreviewCommand =
  | "prepare"
  | "seek"
  | "play"
  | "pause"
  | "reverse"
  | "restart";

export type MotionPreviewRequest = {
  serial: number;
  interactionId: string;
  command: MotionPreviewCommand;
  value?: number;
};

export type MotionPreviewStatus = {
  interactionId: string;
  value: number;
  duration: number;
  progress: number;
  paused: boolean;
  reversed: boolean;
};

type MotionContext = {
  templatePath: string;
  projectRoot: string;
  runtimeSessionId: string;
  refreshToken: number;
};

function contextKey(context: MotionContext): string {
  return `${context.projectRoot}\u0000${context.runtimeSessionId}\u0000${context.templatePath}`;
}

export class MotionWorkspaceState {
  owner = $state<MotionOwner | null>(null);
  config = $state<PageJsConfig>(emptyPageJsConfig());
  accepted = $state<PageJsConfig>(emptyPageJsConfig());
  runtimeContract = $state<MotionRuntimeContract | null>(null);
  loadState = $state<MotionWorkspaceLoadState>("idle");
  error = $state("");
  entryRevision = $state<number | null>(null);
  workspaceRevision = $state(0);
  pendingCount = $state(0);

  selectedInteractionId = $state<string | null>(null);
  selectedActionId = $state<string | null>(null);
  timelineOpen = $state(false);
  timelineCollapsed = $state(false);
  timelineHeight = $state(300);
  timelineZoom = $state(1);
  timelineSnap = $state(true);
  previewMode = $state<MotionPreviewMode>("design");
  previewRequest = $state<MotionPreviewRequest | null>(null);
  previewStatus = $state<MotionPreviewStatus | null>(null);

  private context: MotionContext | null = null;
  private loadSerial = 0;
  private previewSerial = 0;
  private mutationTail: Promise<void> = Promise.resolve();

  get interactions() {
    return this.config.motion?.interactions ?? [];
  }

  get selectedInteraction(): MotionInteraction | null {
    return this.interactions.find((item) => item.id === this.selectedInteractionId) ?? null;
  }

  get selectedAction(): MotionAction | null {
    return this.selectedInteraction?.actions.find((item) => item.id === this.selectedActionId) ?? null;
  }

  get ready(): boolean {
    return this.loadState === "ready" && Boolean(this.context);
  }

  bind(
    templatePath: string | null | undefined,
    projectRoot: string,
    runtimeSessionId: string,
    refreshToken: number,
  ) {
    const canonicalTemplatePath = normalizePageJsTemplatePath(templatePath);
    if (!canonicalTemplatePath || !projectRoot || !runtimeSessionId) {
      this.clear(canonicalTemplatePath ? t("motion-workspace-session-unavailable") : "");
      return;
    }
    const next: MotionContext = {
      templatePath: canonicalTemplatePath,
      projectRoot,
      runtimeSessionId,
      refreshToken,
    };
    const sameIdentity = this.context && contextKey(this.context) === contextKey(next);
    if (sameIdentity && this.context?.refreshToken === refreshToken) return;
    this.owner = { kind: "template", templatePath: canonicalTemplatePath };
    this.context = next;
    void this.load(next);
  }

  async reload() {
    if (!this.context) return;
    const next = { ...this.context };
    await this.load(next);
  }

  private clear(error = "") {
    this.loadSerial += 1;
    this.context = null;
    this.owner = null;
    this.config = emptyPageJsConfig();
    this.accepted = emptyPageJsConfig();
    this.runtimeContract = null;
    this.entryRevision = null;
    this.loadState = error ? "error" : "idle";
    this.error = error;
    this.selectedInteractionId = null;
    this.selectedActionId = null;
    this.timelineOpen = false;
    this.previewMode = "design";
    this.previewStatus = null;
  }

  private async load(context: MotionContext) {
    const serial = ++this.loadSerial;
    this.loadState = "loading";
    this.error = "";
    try {
      await this.flush();
      const identity = createPageJsRequestIdentity(
        context.projectRoot,
        context.runtimeSessionId,
      );
      const receipt = await getPageJsWorkspaceState(context.templatePath, identity);
      if (
        serial !== this.loadSerial
        || !this.context
        || contextKey(this.context) !== contextKey(context)
        || !isPageJsRequestIdentityCurrent(
          identity,
          this.context.projectRoot,
          this.context.runtimeSessionId,
        )
      ) return;
      const workspace = pageJsCommandPayload(
        receipt,
        identity,
        t("motion-workspace-read-operation"),
      );
      const receiptTemplatePath = normalizePageJsTemplatePath(workspace.templatePath);
      if (receiptTemplatePath !== context.templatePath) {
        throw new Error(
          t("motion-workspace-load-owner-mismatch"),
        );
      }
      this.accepted = normalizePageJsConfig(workspace.accepted);
      this.config = normalizePageJsConfig(workspace.current);
      this.runtimeContract = workspace.motionRuntime;
      this.entryRevision = workspace.entryRevision;
      this.context.refreshToken = context.refreshToken;
      this.reconcileSelection();
      this.loadState = "ready";
    } catch (error) {
      if (serial !== this.loadSerial) return;
      this.error = localizedErrorMessage(error);
      this.loadState = "error";
    }
  }

  async mutate(mutation: MotionMutation): Promise<MotionPageMutationReceipt> {
    let resolveResult!: (value: MotionPageMutationReceipt) => void;
    let rejectResult!: (reason: unknown) => void;
    const result = new Promise<MotionPageMutationReceipt>((resolve, reject) => {
      resolveResult = resolve;
      rejectResult = reject;
    });
    this.pendingCount += 1;
    this.mutationTail = this.mutationTail
      .catch(() => {})
      .then(async () => {
        try {
          resolveResult(await this.executeMutation(mutation));
        } catch (error) {
          rejectResult(error);
        }
      })
      .finally(() => {
        this.pendingCount = Math.max(0, this.pendingCount - 1);
      });
    return result;
  }

  private async executeMutation(mutation: MotionMutation): Promise<MotionPageMutationReceipt> {
    const context = this.context ? { ...this.context } : null;
    if (!context || this.loadState !== "ready") {
      throw new Error(t("motion-workspace-not-ready"));
    }
    this.error = "";
    try {
      const receipt = await applyMotionMutation({
        templatePath: context.templatePath,
        expectedProjectRoot: context.projectRoot,
        expectedSessionId: context.runtimeSessionId,
        expectedEntryRevision: this.entryRevision,
        mutation,
      });
      const receiptTemplatePath = normalizePageJsTemplatePath(receipt.pageJs.templatePath);
      if (receiptTemplatePath !== context.templatePath) {
        throw new Error(
          t("motion-workspace-owner-mismatch"),
        );
      }
      if (!this.context || contextKey(this.context) !== contextKey(context)) {
        throw new Error(t("motion-workspace-template-inactive"));
      }
      this.config = normalizePageJsConfig(receipt.mutation.config);
      this.entryRevision = receipt.pageJs.entryRevision;
      this.workspaceRevision = receipt.workspaceRevision;
      this.reconcileSelection();
      return receipt;
    } catch (error) {
      this.error = localizedErrorMessage(error);
      if (/stale|entryRevision/i.test(this.error)) {
        queueMicrotask(() => {
          void this.load(context);
        });
      }
      throw error;
    }
  }

  async flush() {
    await this.mutationTail;
  }

  selectInteraction(interactionId: string | null, actionId: string | null = null) {
    if (this.selectedInteractionId !== interactionId) this.previewStatus = null;
    this.selectedInteractionId = interactionId;
    const interaction = this.interactions.find((item) => item.id === interactionId);
    this.selectedActionId = actionId && interaction?.actions.some((item) => item.id === actionId)
      ? actionId
      : interaction?.actions[0]?.id ?? null;
  }

  openTimeline(interactionId?: string | null, actionId?: string | null) {
    if (interactionId) this.selectInteraction(interactionId, actionId ?? null);
    this.timelineOpen = true;
    this.timelineCollapsed = false;
  }

  closeTimeline() {
    this.timelineOpen = false;
    this.timelineCollapsed = false;
  }

  requestPreview(
    command: MotionPreviewCommand,
    interactionId = this.selectedInteractionId,
    value?: number,
  ) {
    if (!interactionId) return;
    this.previewMode = "motion";
    this.previewRequest = {
      serial: ++this.previewSerial,
      interactionId,
      command,
      value,
    };
  }

  acceptPreviewStatus(status: MotionPreviewStatus) {
    if (
      this.previewMode !== "motion"
      || status.interactionId !== this.selectedInteractionId
    ) return;
    this.previewStatus = status;
  }

  private reconcileSelection() {
    const interactions = this.interactions;
    if (!interactions.some((item) => item.id === this.selectedInteractionId)) {
      this.selectedInteractionId = interactions[0]?.id ?? null;
    }
    const interaction = interactions.find((item) => item.id === this.selectedInteractionId);
    if (!interaction?.actions.some((item) => item.id === this.selectedActionId)) {
      this.selectedActionId = interaction?.actions[0]?.id ?? null;
    }
    if (!interaction && this.timelineOpen) this.timelineOpen = false;
  }
}
