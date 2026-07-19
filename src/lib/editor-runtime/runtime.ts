import {
  deleteSelectedHtmlElement,
  duplicateSelectedHtmlElement,
  type HtmlActionsControllerHost,
} from "$lib/state/html-actions-controller";
import type { SelectionControllerHost } from "$lib/state/selection-controller";
import {
  blockedAction,
  committedAction,
  editorActionSucceeded,
  failedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  canMutateHtmlTarget,
  captureEditorCommand,
  type EditorCommand,
  type EditorCommandResult,
  type EditorSurface,
  type EditorTeraTarget,
  type EditorTransaction,
} from "$lib/editor-runtime/commands";
import type { PageSection, SaveState } from "$lib/types";

export type EditorRuntimeHost = {
  centerView: string;
  setCenterView: (view: "code") => Promise<boolean>;
  templateHtmlEditSourceId: string | null;
  htmlActionsControllerHost: () => HtmlActionsControllerHost;
  selectionControllerHost: () => SelectionControllerHost;
  selectDomNode: (selector: string, options?: { revealCode?: boolean }) => void;
  selectTeraLayerSource: (section: PageSection, sourceId: string) => void;
  setPreviewTeraSelection: (
    gate: {
      selector: string;
      sourceId: string;
      origin: "current" | "local" | "theme" | "unknown";
      themeName: string | null;
      canSelectHtml?: boolean;
    },
    options?: { status?: string; showGate?: boolean; clearHtmlMarker?: boolean },
  ) => void;
  allowTemplateHtmlEdit: (sourceId: string | null, selector: string | null) => void | Promise<void>;
  openSelectedTeraSource: () => Promise<void>;
  deleteSelectedTeraNode: (target?: EditorTeraTarget | null) => Promise<EditorActionOutcome>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
};

function commandSurface(command: EditorCommand): EditorSurface {
  return command.surface ?? "runtime";
}

function transactionFor(revision: number, command: EditorCommand): EditorTransaction {
  const target = command.target;
  return {
    revision,
    command: command.type,
    surface: commandSurface(command),
    targetKind: target.kind,
    selector: target.selector ?? null,
    sourceId: target.kind === "html" ? target.sourceId ?? null : target.sourceId,
    startedAt: Date.now(),
  };
}

export class EditorRuntime {
  private revision = 0;
  private readonly host: EditorRuntimeHost;
  lastTransaction: EditorTransaction | null = null;

  constructor(host: EditorRuntimeHost) {
    this.host = host;
  }

  currentRevision() {
    return this.revision;
  }

  canDispatch(command: EditorCommand) {
    if (command.type === "delete-html" || command.type === "duplicate-html") {
      return canMutateHtmlTarget(command.target);
    }
    if ((command.type === "select-html" || command.type === "open-html-code") && !command.target.selector) {
      return { allowed: false, reason: "Elementul nu are selector stabil." };
    }
    if ((command.type === "edit-tera-html" || command.type === "select-tera") && !command.target.selector) {
      return { allowed: false, reason: "Nodul Tera nu are selector de preview." };
    }
    return { allowed: true, reason: "" };
  }

  async dispatch(command: EditorCommand): Promise<EditorCommandResult> {
    const capturedCommand = captureEditorCommand(command);
    const revision = ++this.revision;
    const transaction = transactionFor(revision, capturedCommand);
    this.lastTransaction = transaction;
    const verdict = this.canDispatch(capturedCommand);
    if (!verdict.allowed) {
      transaction.completedAt = Date.now();
      transaction.ok = false;
      transaction.status = "blocked";
      transaction.reason = verdict.reason;
      if (verdict.reason) this.host.setGlobalStatus(verdict.reason, "error");
      return { ok: false, status: "blocked", revision, command: capturedCommand.type, reason: verdict.reason };
    }

    try {
      const outcome = await this.execute(capturedCommand);
      const ok = editorActionSucceeded(outcome);
      transaction.completedAt = Date.now();
      transaction.ok = ok;
      transaction.status = outcome.status;
      transaction.reason = outcome.reason;
      if (!ok && outcome.reason) this.host.setGlobalStatus(outcome.reason, "error");
      return {
        ok,
        status: outcome.status,
        revision,
        command: capturedCommand.type,
        ...(outcome.reason ? { reason: outcome.reason } : {}),
      };
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      const outcome = failedAction(reason);
      transaction.completedAt = Date.now();
      transaction.ok = false;
      transaction.status = outcome.status;
      transaction.reason = reason;
      this.host.setGlobalStatus(reason, "error");
      return { ok: false, status: outcome.status, revision, command: capturedCommand.type, reason };
    }
  }

  private async execute(command: EditorCommand): Promise<EditorActionOutcome> {
    switch (command.type) {
      case "select-html":
        this.host.selectDomNode(command.target.selector, {
          revealCode: command.revealCode === true,
        });
        return committedAction();
      case "open-html-code":
        if (!await this.host.setCenterView("code")) {
          return blockedAction("Editorul de cod nu a acceptat schimbarea de suprafață.");
        }
        this.host.selectDomNode(command.target.selector, { revealCode: true });
        return committedAction();
      case "delete-html":
        return await deleteSelectedHtmlElement(this.host.htmlActionsControllerHost(), command.target);
      case "duplicate-html":
        return await duplicateSelectedHtmlElement(this.host.htmlActionsControllerHost(), command.target);
      case "select-tera":
        this.selectTera(command);
        return committedAction();
      case "edit-tera-html":
        this.selectTera(command);
        await this.host.allowTemplateHtmlEdit(command.target.sourceId, command.target.selector);
        return committedAction();
      case "open-tera-code":
        this.selectTera(command);
        await this.host.openSelectedTeraSource();
        if (!await this.host.setCenterView("code")) {
          return blockedAction("Editorul de cod nu a acceptat schimbarea de suprafață.");
        }
        return committedAction();
      case "delete-tera":
        return await this.host.deleteSelectedTeraNode(command.target);
    }
  }

  private selectTera(command: Extract<EditorCommand, { target: { kind: "tera" } }>) {
    const target = command.target;
    if (target.section) {
      this.host.selectTeraLayerSource(target.section, target.sourceId);
      return;
    }
    if (!target.selector) return;
    this.host.setPreviewTeraSelection({
      selector: target.selector,
      sourceId: target.sourceId,
      origin: target.origin ?? "unknown",
      themeName: target.themeName ?? null,
      canSelectHtml: target.canSelectHtml,
    });
  }
}

export function createEditorRuntime(host: EditorRuntimeHost) {
  return new EditorRuntime(host);
}
