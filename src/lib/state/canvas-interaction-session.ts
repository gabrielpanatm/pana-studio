import {
  canvasRouteFromPreviewUrl,
  normalizeProjectDocumentPath,
  sameCanvasProjectionIdentity,
  sameProjectDocumentPath,
} from "$lib/contracts/canvas-identity";
import {
  CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
  createCanvasInteractionIdentity,
} from "$lib/preview/canvas-interaction";
import {
  bindCanvasInteractionAgent,
} from "$lib/canvas/interaction-io";
import type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";
import {
  canvasInteractionRuntimeFor,
  canvasRuntimeDocumentPathIsCurrent,
  clearCanvasActivationTimer,
  nextCanvasDocumentEpoch,
  refreshExpectedCanvasDocumentPath,
  resetCanvasInteractionRuntime,
  suspendCanvasInteractionRuntime,
  type CanvasInteractionFrontendRuntime,
} from "$lib/state/canvas-interaction-runtime";
import type {
  CanvasInteractionBindingReceipt,
  CanvasInteractionIdentity,
} from "$lib/canvas/contracts";
import { errorMessage } from "$lib/util";

const CANVAS_AGENT_ACTIVATION_TIMEOUT_MS = 2_000;

export function canvasInteractionBindingKey(
  canvas: CanvasInteractionIdentity["canvas"],
  route: string,
  agentInstanceId: string,
  activeDocumentPath: string | null,
) {
  return JSON.stringify([
    canvas.projectRoot,
    canvas.runtimeSessionId,
    canvas.workspaceRevision,
    canvas.transactionId,
    canvas.previewRevision,
    route,
    agentInstanceId,
    normalizeProjectDocumentPath(activeDocumentPath),
  ]);
}

export function deactivateCanvasAgent(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  agentInstanceId = runtime.agentInstanceId,
) {
  if (!agentInstanceId) return;
  app.commands.postPreviewMessage({
    type: "deactivate-canvas-interaction-agent",
    agentInstanceId,
  });
}

function previewRoute(app: CanvasInteractionControllerHost) {
  const source = app.session.activeCanvasUrl && app.session.activeCanvasUrl !== "about:blank"
    ? app.session.activeCanvasUrl
    : app.session.previewSrc;
  return canvasRouteFromPreviewUrl(source, app.session.browserPreviewRoute);
}

function confirmedCanvasNavigationBinding(
  app: CanvasInteractionControllerHost,
  canvas: CanvasInteractionIdentity["canvas"],
  route: string,
  activeDocumentPath: string | null,
) {
  const snapshot = app.selection.editorSelection.navigationSnapshot;
  const ready = Boolean(
    snapshot
    && sameCanvasProjectionIdentity(snapshot.identity, canvas)
    && snapshot.route === route
    && sameProjectDocumentPath(
      snapshot.focusedView?.activeDocumentPath,
      activeDocumentPath,
    ),
  );
  return { activeDocumentPath, ready };
}

export function currentCanvasInteractionBinding(
  app: CanvasInteractionControllerHost,
  runtime = canvasInteractionRuntimeFor(app),
) {
  const binding = runtime.binding;
  return canvasInteractionSurfaceActive(app)
    && runtime.phase === "active"
    && binding
    && runtime.agentInstanceId === binding.identity.agentInstanceId
    && runtime.documentEpoch === binding.identity.documentEpoch
    && sameCanvasProjectionIdentity(app.session.activeCanvasIdentity, binding.identity.canvas)
    && canvasRuntimeDocumentPathIsCurrent(app, runtime)
    && sameProjectDocumentPath(
      binding.activeDocumentPath,
      runtime.expectedDocumentPath,
    )
    ? binding
    : null;
}

export function canvasInteractionSurfaceActive(app: CanvasInteractionControllerHost) {
  return app.session.applicationSurface === "workbench"
    && (app.session.workbenchSnapshot?.activeActivity ?? "editor") === "editor"
    && app.session.centerView !== "kernel";
}

export async function retryCanvasInteractionBinding(app: CanvasInteractionControllerHost) {
  const runtime = canvasInteractionRuntimeFor(app);
  if (!runtime.agentInstanceId || currentCanvasInteractionBinding(app, runtime)) return;
  synchronizeCanvasInteractionBinding(app);
  if (runtime.phase === "binding" || runtime.phase === "activating") return;
  await bindCurrentCanvasAgent(app);
}

export function synchronizeCanvasInteractionBinding(app: CanvasInteractionControllerHost) {
  const runtime = canvasInteractionRuntimeFor(app);
  const activeDocumentPath = refreshExpectedCanvasDocumentPath(app, runtime);
  const canvas = app.session.activeCanvasIdentity;
  const agentInstanceId = runtime.agentInstanceId;
  if (!canvas || !agentInstanceId) {
    runtime.bindSerial += 1;
    clearCanvasActivationTimer(runtime);
    resetCanvasInteractionRuntime(runtime);
    runtime.desiredBindingKey = null;
    runtime.documentEpoch = 0;
    runtime.phase = "dormant";
    deactivateCanvasAgent(app, runtime);
    return;
  }

  const route = previewRoute(app);
  const navigation = confirmedCanvasNavigationBinding(
    app,
    canvas,
    route,
    activeDocumentPath,
  );
  const desiredBindingKey = canvasInteractionBindingKey(
    canvas,
    route,
    agentInstanceId,
    navigation.activeDocumentPath,
  );
  if (!canvasInteractionSurfaceActive(app)) {
    const retainedBinding = runtime.binding ?? runtime.pendingBinding;
    if (
      runtime.desiredBindingKey === desiredBindingKey
      && retainedBinding
      && retainedBinding.identity.agentInstanceId === agentInstanceId
      && retainedBinding.identity.route === route
      && sameCanvasProjectionIdentity(retainedBinding.identity.canvas, canvas)
    ) {
      runtime.bindSerial += 1;
      clearCanvasActivationTimer(runtime);
      suspendCanvasInteractionRuntime(runtime);
      runtime.binding = retainedBinding;
      runtime.pendingBinding = null;
      runtime.phase = "suspended";
      deactivateCanvasAgent(app, runtime);
      return;
    }
    runtime.bindSerial += 1;
    clearCanvasActivationTimer(runtime);
    resetCanvasInteractionRuntime(runtime);
    runtime.desiredBindingKey = null;
    runtime.documentEpoch = 0;
    runtime.phase = "dormant";
    deactivateCanvasAgent(app, runtime);
    return;
  }

  if (
    runtime.phase === "suspended"
    && runtime.desiredBindingKey === desiredBindingKey
    && runtime.binding
    && runtime.binding.identity.agentInstanceId === agentInstanceId
    && runtime.binding.identity.route === route
    && sameCanvasProjectionIdentity(runtime.binding.identity.canvas, canvas)
  ) {
    reactivateRetainedCanvasAgent(app, runtime, runtime.binding);
    return;
  }

  if (runtime.desiredBindingKey !== desiredBindingKey) {
    runtime.bindSerial += 1;
    clearCanvasActivationTimer(runtime);
    resetCanvasInteractionRuntime(runtime);
    deactivateCanvasAgent(app, runtime);
    runtime.desiredBindingKey = desiredBindingKey;
    runtime.documentEpoch = nextCanvasDocumentEpoch(runtime);
    if (!navigation.ready) {
      runtime.phase = "waitingNavigation";
      return;
    }
    runtime.phase = "binding";
    void bindCurrentCanvasAgent(app);
    return;
  }
  if (currentCanvasInteractionBinding(app, runtime)) return;
  if (runtime.phase === "binding" || runtime.phase === "activating") return;
  if (!navigation.ready) {
    runtime.phase = "waitingNavigation";
    return;
  }
  runtime.phase = "binding";
  void bindCurrentCanvasAgent(app);
}

function reactivateRetainedCanvasAgent(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  retainedBinding: CanvasInteractionBindingReceipt,
) {
  const serial = ++runtime.bindSerial;
  const lastAcceptedSequence = Math.max(
    retainedBinding.lastAcceptedSequence,
    runtime.lastObservedAgentSequence,
  );
  const binding = {
    ...retainedBinding,
    lastAcceptedSequence,
  };
  runtime.binding = null;
  runtime.pendingBinding = binding;
  runtime.phase = "activating";
  app.commands.postPreviewMessage({
    type: "activate-canvas-interaction-agent",
    schemaVersion: CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
    agentInstanceId: binding.identity.agentInstanceId,
    documentEpoch: binding.identity.documentEpoch,
    lastAcceptedSequence,
    selection: true,
    authoringSurfaces: binding.authoringSurfaces,
  });
  clearCanvasActivationTimer(runtime);
  runtime.activationTimer = window.setTimeout(() => {
    if (
      runtime.phase !== "activating"
      || runtime.pendingBinding !== binding
      || runtime.bindSerial !== serial
    ) return;
    failCanvasInteractionBinding(
      app,
      runtime,
      new Error("CanvasAgent nu a confirmat reactivarea lease-ului păstrat."),
    );
  }, CANVAS_AGENT_ACTIVATION_TIMEOUT_MS);
}

async function bindCurrentCanvasAgent(app: CanvasInteractionControllerHost) {
  const runtime = canvasInteractionRuntimeFor(app);
  const canvas = app.session.activeCanvasIdentity;
  const agentInstanceId = runtime.agentInstanceId;
  const documentEpoch = runtime.documentEpoch;
  if (!canvas || !agentInstanceId || documentEpoch <= 0) return;

  const route = previewRoute(app);
  if (!canvasRuntimeDocumentPathIsCurrent(app, runtime)) {
    synchronizeCanvasInteractionBinding(app);
    return;
  }
  const navigation = confirmedCanvasNavigationBinding(
    app,
    canvas,
    route,
    runtime.expectedDocumentPath,
  );
  const desiredBindingKey = canvasInteractionBindingKey(
    canvas,
    route,
    agentInstanceId,
    navigation.activeDocumentPath,
  );
  if (runtime.desiredBindingKey !== desiredBindingKey) return;
  if (!navigation.ready) {
    runtime.phase = "waitingNavigation";
    return;
  }
  runtime.phase = "binding";
  const serial = ++runtime.bindSerial;
  const identity = createCanvasInteractionIdentity(
    canvas,
    route,
    documentEpoch,
    agentInstanceId,
  );
  try {
    const receipt = await bindCanvasInteractionAgent(
      identity,
      navigation.activeDocumentPath,
      app.selection.coordinatedElementSelection?.renderInstanceId ?? null,
    );
    if (!sameProjectDocumentPath(receipt.activeDocumentPath, navigation.activeDocumentPath)) {
      throw new Error("CanvasAgent a întors binding-ul altui document activ.");
    }
    if (
      serial !== runtime.bindSerial
      || runtime.agentInstanceId !== agentInstanceId
      || runtime.documentEpoch !== documentEpoch
      || runtime.desiredBindingKey !== desiredBindingKey
      || !sameCanvasProjectionIdentity(app.session.activeCanvasIdentity, canvas)
      || !canvasRuntimeDocumentPathIsCurrent(app, runtime)
      || !confirmedCanvasNavigationBinding(
        app,
        canvas,
        route,
        runtime.expectedDocumentPath,
      ).ready
      || !sameProjectDocumentPath(
        runtime.expectedDocumentPath,
        navigation.activeDocumentPath,
      )
    ) return;
    runtime.pendingBinding = receipt;
    runtime.phase = "activating";
    runtime.pendingInspections.clear();
    runtime.dragSource = null;
    runtime.dragMovePreview = null;
    runtime.latestPointerMoveSequence = receipt.lastAcceptedSequence;
    runtime.latestDragOverSequence = receipt.lastAcceptedSequence;
    runtime.lastObservedAgentSequence = receipt.lastAcceptedSequence;
    app.commands.postPreviewMessage({
      type: "activate-canvas-interaction-agent",
      schemaVersion: CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
      agentInstanceId,
      documentEpoch,
      lastAcceptedSequence: receipt.lastAcceptedSequence,
      selection: true,
      authoringSurfaces: receipt.authoringSurfaces,
    });
    clearCanvasActivationTimer(runtime);
    runtime.activationTimer = window.setTimeout(() => {
      if (
        runtime.phase !== "activating"
        || runtime.pendingBinding !== receipt
        || runtime.bindSerial !== serial
      ) return;
      failCanvasInteractionBinding(
        app,
        runtime,
        new Error("CanvasAgent nu a confirmat activarea."),
      );
    }, CANVAS_AGENT_ACTIVATION_TIMEOUT_MS);
  } catch (error) {
    if (serial !== runtime.bindSerial) return;
    clearCanvasActivationTimer(runtime);
    resetCanvasInteractionRuntime(runtime);
    runtime.phase = "failed";
    deactivateCanvasAgent(app, runtime, agentInstanceId);
    app.commands.setGlobalStatus(
      `Canvas Interaction Rust nu a putut fi activat: ${errorMessage(error)}`,
      "error",
    );
  }
}

export function failCanvasInteractionBinding(
  app: CanvasInteractionControllerHost,
  runtime: CanvasInteractionFrontendRuntime,
  error: unknown,
) {
  const agentInstanceId = runtime.agentInstanceId;
  runtime.bindSerial += 1;
  clearCanvasActivationTimer(runtime);
  resetCanvasInteractionRuntime(runtime);
  runtime.phase = "failed";
  deactivateCanvasAgent(app, runtime, agentInstanceId);
  app.commands.setGlobalStatus(
    `Canvas Interaction Rust a fost oprit după o eroare: ${errorMessage(error)}`,
    "error",
  );
}
