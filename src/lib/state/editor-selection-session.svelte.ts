import {
  acceptSelectionObservation as acceptSelectionObservationInRust,
  applySelectionIntent as applySelectionIntentInRust,
  readSelectionSnapshot,
} from "$lib/editor/selection-io";
import {
  readEditorNavigationSnapshot,
} from "$lib/editor/navigation-io";
import { captureCanvasElementObservation } from "$lib/editor-runtime/commands";
import {
  primarySelectionEntry,
  primarySelectionRenderInstanceId,
  selectionResolution,
} from "$lib/kernel/selection-read-model";
import { errorMessage } from "$lib/util";
import type { CanvasProjectionIdentity } from "$lib/contracts/canvas-projection";
import {
  canvasRouteFromPreviewUrl,
  sameCanvasProjectionIdentity as canvasIdentityEquals,
} from "$lib/contracts/canvas-identity";
import type {
  AcceptedCanvasElementObservation,
  CanvasElementObservation,
  CoordinatedElementSelection,
} from "$lib/canvas/contracts";
import type { EditScopeGrant } from "$lib/editor/contracts";
import type { EditorNavigationSnapshot } from "$lib/editor/contracts";
import type {
  HoverSnapshot,
  InspectorSelectionSummarySnapshot,
  SelectionIntent,
  SelectionObservationInput,
  SelectionSnapshot,
} from "$lib/editor/contracts";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";

export type EditorSelectionSessionDiagnostics = Readonly<{
  resets: number;
  navigationRequests: number;
  navigationKeyChanges: number;
  navigationRefreshesDeduplicated: number;
  selectionRequests: number;
  selectionIntentRequests: number;
  selectionRebaseRequests: number;
  hoverRequests: number;
  hoverIntentRequests: number;
  hoverProjectionRequests: number;
  observationRequests: number;
  staleNavigationResponses: number;
  staleSelectionResponses: number;
  staleHoverResponses: number;
  staleObservationResponses: number;
}>;

type MutableEditorSelectionSessionDiagnostics = {
  -readonly [Key in keyof EditorSelectionSessionDiagnostics]:
    EditorSelectionSessionDiagnostics[Key];
};

export type EditorSelectionSessionHost = {
  activeCanvasIdentity: CanvasProjectionIdentity | null;
  activeCanvasUrl: string;
  activeScannedPath: string | null;
  browserPreviewRoute: string;
  coordinatedElementSelection: CoordinatedElementSelection | null;
  previewSrc: string;
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  applySelectionState: (observation: CanvasElementObservation) => void;
  projectSelectionSnapshotOnCanvas: (selection: SelectionSnapshot) => void;
};

export const editorSelectionRoute = canvasRouteFromPreviewUrl;

function navigationRefreshKey(
  identity: CanvasProjectionIdentity,
  route: string,
  activeDocumentPath: string | null,
  previewContextRenderInstanceId: string | null,
) {
  return JSON.stringify([
    identity.projectRoot,
    identity.runtimeSessionId,
    identity.workspaceRevision,
    identity.transactionId,
    identity.previewRevision,
    route,
    activeDocumentPath,
    previewContextRenderInstanceId,
  ]);
}

function emptyDiagnostics(): MutableEditorSelectionSessionDiagnostics {
  return {
    resets: 0,
    navigationRequests: 0,
    navigationKeyChanges: 0,
    navigationRefreshesDeduplicated: 0,
    selectionRequests: 0,
    selectionIntentRequests: 0,
    selectionRebaseRequests: 0,
    hoverRequests: 0,
    hoverIntentRequests: 0,
    hoverProjectionRequests: 0,
    observationRequests: 0,
    staleNavigationResponses: 0,
    staleSelectionResponses: 0,
    staleHoverResponses: 0,
    staleObservationResponses: 0,
  };
}

/**
 * Owns the frontend projection of Rust EditorNavigation and SelectionCoordinator.
 * Every async settlement is guarded by both a request generation and the exact
 * Canvas identity which issued it.
 */
export class EditorSelectionSessionController {
  acceptedObservation = $state<AcceptedCanvasElementObservation | null>(null);
  navigationSnapshot = $state<EditorNavigationSnapshot | null>(null);
  navigationLoading = $state(false);
  navigationError = $state("");
  editScopeGrant = $state<EditScopeGrant | null>(null);
  editScopeId = $state<string | null>(null);
  selectionSnapshot = $state<SelectionSnapshot | null>(null);
  inspectorSummary = $state<InspectorSelectionSummarySnapshot | null>(null);
  hoverSnapshot = $state<HoverSnapshot | null>(null);
  readonly diagnostics = emptyDiagnostics();

  private navigationRequestSerial = 0;
  private currentNavigationRefreshKey = "";
  private navigationRefreshPromise: Promise<void> | null = null;
  private selectionRequestSerial = 0;
  private hoverRequestSerial = 0;
  private observationRequestSerial = 0;
  private readonly host: () => EditorSelectionSessionHost;

  constructor(host: () => EditorSelectionSessionHost) {
    this.host = host;
  }

  reset() {
    this.diagnostics.resets += 1;
    this.navigationRequestSerial += 1;
    this.selectionRequestSerial += 1;
    this.hoverRequestSerial += 1;
    this.observationRequestSerial += 1;
    this.currentNavigationRefreshKey = "";
    this.navigationRefreshPromise = null;
    this.acceptedObservation = null;
    this.navigationSnapshot = null;
    this.navigationLoading = false;
    this.navigationError = "";
    this.editScopeGrant = null;
    this.editScopeId = null;
    this.selectionSnapshot = null;
    this.inspectorSummary = null;
    this.hoverSnapshot = null;
  }

  clearSelectionProjection() {
    this.acceptedObservation = null;
    this.inspectorSummary = null;
  }

  diagnosticSnapshot() {
    let navigationKeyParts: unknown[] = [];
    try {
      navigationKeyParts = this.currentNavigationRefreshKey
        ? JSON.parse(this.currentNavigationRefreshKey) as unknown[]
        : [];
    } catch {
      navigationKeyParts = [];
    }
    return Object.freeze({
      counters: Object.freeze({ ...this.diagnostics }),
      navigation: Object.freeze({
        inFlight: this.navigationRefreshPromise !== null,
        loading: this.navigationLoading,
        error: this.navigationError,
        keyParts: Object.freeze(navigationKeyParts),
        modelRevision: this.navigationSnapshot?.modelRevision ?? null,
      }),
      selectionRevision: this.selectionSnapshot?.selectionRevision ?? null,
      selectionFocus: this.selectionSnapshot?.focus ?? null,
      hoverRevision: this.hoverSnapshot?.hoverRevision ?? null,
      acceptedObservationRevision: this.acceptedObservation?.selectionRevision ?? null,
    });
  }

  async refreshNavigationSnapshot(
    identity = this.host().activeCanvasIdentity ?? undefined,
    previewUrl = this.host().activeCanvasUrl || this.host().previewSrc,
    options: { strict?: boolean } = {},
  ) {
    const host = this.host();
    if (!identity) {
      this.reset();
      return;
    }
    // ProjectWorkspace advances before its Canvas during rapid edits. Never
    // ask Rust to reinterpret the retained, intentionally stale Canvas.
    if (host.projectWorkspaceSnapshot?.revision !== identity.workspaceRevision) return;

    const route = editorSelectionRoute(previewUrl, host.browserPreviewRoute);
    const activeDocumentPath = host.activeScannedPath;
    const previewContextRenderInstanceId =
      host.coordinatedElementSelection?.renderInstanceId ?? null;
    const refreshKey = navigationRefreshKey(
      identity,
      route,
      activeDocumentPath,
      previewContextRenderInstanceId,
    );
    if (this.currentNavigationRefreshKey === refreshKey) {
      if (this.navigationRefreshPromise) {
        this.diagnostics.navigationRefreshesDeduplicated += 1;
        await this.navigationRefreshPromise;
        if (options.strict && this.navigationError) throw new Error(this.navigationError);
        return;
      }
      if (this.navigationSnapshot && !this.navigationError) {
        this.diagnostics.navigationRefreshesDeduplicated += 1;
        return;
      }
    }

    let finishRefresh = () => {};
    const refreshPromise = new Promise<void>((resolve) => {
      finishRefresh = resolve;
    });
    if (
      this.currentNavigationRefreshKey
      && this.currentNavigationRefreshKey !== refreshKey
    ) {
      this.diagnostics.navigationKeyChanges += 1;
    }
    this.currentNavigationRefreshKey = refreshKey;
    this.navigationRefreshPromise = refreshPromise;
    const serial = ++this.navigationRequestSerial;
    this.diagnostics.navigationRequests += 1;
    this.navigationLoading = true;
    this.navigationError = "";
    try {
      const snapshot = await readEditorNavigationSnapshot(
        identity,
        route,
        activeDocumentPath,
        previewContextRenderInstanceId,
      );
      if (
        serial !== this.navigationRequestSerial
        || !canvasIdentityEquals(this.host().activeCanvasIdentity, identity)
      ) {
        this.diagnostics.staleNavigationResponses += 1;
        return;
      }
      this.navigationSnapshot = snapshot;
      if (
        this.editScopeGrant
        && (
          this.editScopeGrant.projectRoot !== identity.projectRoot
          || this.editScopeGrant.runtimeSessionId !== identity.runtimeSessionId
          || this.editScopeGrant.workspaceRevision !== identity.workspaceRevision
          || this.editScopeGrant.previewRevision !== identity.previewRevision
          || this.editScopeGrant.canvasTransactionId !== identity.transactionId
          || this.editScopeGrant.activeDocumentPath
            !== snapshot.focusedView?.activeDocumentPath
        )
      ) {
        this.editScopeGrant = null;
        this.editScopeId = null;
      }
      await this.rebaseSelection(identity, route);
    } catch (error) {
      if (
        serial !== this.navigationRequestSerial
        || !canvasIdentityEquals(this.host().activeCanvasIdentity, identity)
      ) {
        this.diagnostics.staleNavigationResponses += 1;
        return;
      }
      if (this.host().projectWorkspaceSnapshot?.revision !== identity.workspaceRevision) return;
      this.navigationSnapshot = null;
      this.navigationError = errorMessage(error);
      this.editScopeGrant = null;
      this.editScopeId = null;
      if (options.strict) throw error;
    } finally {
      if (serial === this.navigationRequestSerial) this.navigationLoading = false;
      finishRefresh();
      if (this.navigationRefreshPromise === refreshPromise) {
        this.navigationRefreshPromise = null;
      }
    }
  }

  async applySelectionIntent(intent: SelectionIntent): Promise<SelectionSnapshot | null> {
    const host = this.host();
    const identity = host.activeCanvasIdentity;
    if (!identity) return null;
    const route = editorSelectionRoute(
      host.activeCanvasUrl || host.previewSrc,
      host.browserPreviewRoute,
    );
    const serial = ++this.selectionRequestSerial;
    this.diagnostics.selectionRequests += 1;
    this.diagnostics.selectionIntentRequests += 1;
    const receipt = await applySelectionIntentInRust(
      identity,
      route,
      host.activeScannedPath,
      primarySelectionRenderInstanceId(this.selectionSnapshot)
        ?? host.coordinatedElementSelection?.renderInstanceId
        ?? null,
      intent,
      this.editScopeGrant,
    );
    if (
      serial !== this.selectionRequestSerial
      || !canvasIdentityEquals(this.host().activeCanvasIdentity, identity)
    ) {
      this.diagnostics.staleSelectionResponses += 1;
      return null;
    }
    this.projectCoordinatorSnapshot(
      receipt.selection,
      receipt.hover,
      receipt.inspectorSummary,
    );
    if (intent.kind === "setFocus") {
      this.host().projectSelectionSnapshotOnCanvas(receipt.selection);
    }
    return receipt.selection;
  }

  async applyHoverIntent(intent: Extract<SelectionIntent, {
    kind: "setHover" | "clearHover";
  }>): Promise<HoverSnapshot | null> {
    const host = this.host();
    const identity = host.activeCanvasIdentity;
    if (!identity) return null;
    const route = editorSelectionRoute(
      host.activeCanvasUrl || host.previewSrc,
      host.browserPreviewRoute,
    );
    const serial = ++this.hoverRequestSerial;
    this.diagnostics.hoverRequests += 1;
    this.diagnostics.hoverIntentRequests += 1;
    const receipt = await applySelectionIntentInRust(
      identity,
      route,
      host.activeScannedPath,
      primarySelectionRenderInstanceId(this.selectionSnapshot)
        ?? host.coordinatedElementSelection?.renderInstanceId
        ?? null,
      intent,
      this.editScopeGrant,
    );
    if (
      serial !== this.hoverRequestSerial
      || !canvasIdentityEquals(this.host().activeCanvasIdentity, identity)
    ) {
      this.diagnostics.staleHoverResponses += 1;
      return null;
    }
    this.hoverSnapshot = receipt.hover;
    this.inspectorSummary = receipt.inspectorSummary;
    return receipt.hover;
  }

  beginCanvasHoverProjection() {
    this.diagnostics.hoverRequests += 1;
    this.diagnostics.hoverProjectionRequests += 1;
    return ++this.hoverRequestSerial;
  }

  projectCanvasHoverReceipt(
    serial: number,
    identity: CanvasProjectionIdentity,
    hover: HoverSnapshot | null,
  ) {
    if (
      serial !== this.hoverRequestSerial
      || !canvasIdentityEquals(this.host().activeCanvasIdentity, identity)
    ) {
      this.diagnostics.staleHoverResponses += 1;
      return false;
    }
    this.hoverSnapshot = hover;
    return true;
  }

  async rebaseSelection(
    identity = this.host().activeCanvasIdentity ?? undefined,
    route = editorSelectionRoute(
      this.host().activeCanvasUrl || this.host().previewSrc,
      this.host().browserPreviewRoute,
    ),
  ): Promise<SelectionSnapshot | null> {
    if (!identity) return null;
    const serial = ++this.selectionRequestSerial;
    this.diagnostics.selectionRequests += 1;
    this.diagnostics.selectionRebaseRequests += 1;
    const receipt = await readSelectionSnapshot(
      identity,
      route,
      this.host().activeScannedPath,
      primarySelectionRenderInstanceId(this.selectionSnapshot)
        ?? this.host().coordinatedElementSelection?.renderInstanceId
        ?? null,
    );
    if (
      serial !== this.selectionRequestSerial
      || !canvasIdentityEquals(this.host().activeCanvasIdentity, identity)
    ) {
      this.diagnostics.staleSelectionResponses += 1;
      return null;
    }
    this.projectCoordinatorSnapshot(
      receipt.selection,
      receipt.hover,
      receipt.inspectorSummary,
    );
    this.host().projectSelectionSnapshotOnCanvas(receipt.selection);
    return receipt.selection;
  }

  async acceptObservation(
    input: SelectionObservationInput,
    observation: CanvasElementObservation,
  ): Promise<AcceptedCanvasElementObservation | null> {
    const selection = this.selectionSnapshot;
    if (
      !selection
      || selection.selectionRevision !== input.selectionRevision
      || !canvasIdentityEquals(selection.canvasIdentity, input.canvasIdentity)
    ) return null;
    const serial = ++this.observationRequestSerial;
    this.diagnostics.observationRequests += 1;
    const receipt = await acceptSelectionObservationInRust(input);
    if (
      serial !== this.observationRequestSerial
      || this.selectionSnapshot?.selectionRevision !== input.selectionRevision
      || !canvasIdentityEquals(this.selectionSnapshot.canvasIdentity, input.canvasIdentity)
    ) {
      this.diagnostics.staleObservationResponses += 1;
      return null;
    }
    const accepted = {
      selectionRevision: receipt.selectionRevision,
      canvasIdentity: receipt.canvasIdentity,
      documentEpoch: receipt.documentEpoch,
      renderInstanceId: receipt.renderInstanceId,
      observation: captureCanvasElementObservation(observation) ?? observation,
    };
    this.acceptedObservation = accepted;
    this.inspectorSummary = receipt.inspectorSummary;
    // Physical values and the Rust-resolved summary settle synchronously; an
    // Inspector render can never pair the new summary with the old element.
    this.host().applySelectionState(accepted.observation);
    return accepted;
  }

  private projectCoordinatorSnapshot(
    selection: SelectionSnapshot,
    hover: HoverSnapshot | null,
    inspectorSummary: InspectorSelectionSummarySnapshot,
  ) {
    const previousRenderInstanceId = primarySelectionRenderInstanceId(this.selectionSnapshot);
    this.selectionSnapshot = selection;
    this.inspectorSummary = inspectorSummary;
    this.hoverSnapshot = hover;

    const primary = primarySelectionEntry(selection);
    if (selectionResolution(selection) === "cleared" || !primary) {
      this.acceptedObservation = null;
      return;
    }
    if (primary.subject.kind === "boundary") {
      this.acceptedObservation = null;
    } else {
      const accepted = this.acceptedObservation;
      const renderInstanceId = primarySelectionRenderInstanceId(selection);
      if (
        !accepted
        || previousRenderInstanceId !== renderInstanceId
        || accepted.renderInstanceId !== renderInstanceId
        || inspectorSummary.state !== "resolved"
        || inspectorSummary.renderInstanceId !== renderInstanceId
      ) {
        this.acceptedObservation = null;
      } else if (accepted.selectionRevision !== selection.selectionRevision) {
        this.acceptedObservation = {
          ...accepted,
          selectionRevision: selection.selectionRevision,
          canvasIdentity: selection.canvasIdentity,
        };
      }
    }
  }
}
