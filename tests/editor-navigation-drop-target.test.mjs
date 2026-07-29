import assert from "node:assert/strict";
import { test } from "node:test";
import {
  editorNavigationDropTargetStatus,
} from "$lib/state/editor-navigation-controller";

function capabilities(overrides = {}) {
  return {
    canSelect: true,
    canInspect: true,
    canOpenInCode: true,
    canEnterBoundary: false,
    canMoveAtomic: false,
    canMove: true,
    canEditText: true,
    canEditAttributes: true,
    readOnly: false,
    requiresEditScopeId: null,
    reasonCode: null,
    ...overrides,
  };
}

function snapshot() {
  return {
    nodes: [
      {
        id: "editor_render:local",
        kind: "htmlElement",
        renderInstanceId: "render-local",
        origin: "project",
        capabilities: capabilities(),
        boundary: null,
      },
      {
        id: "editor_render:partial",
        kind: "htmlElement",
        renderInstanceId: "render-partial",
        origin: "project",
        capabilities: capabilities({
          readOnly: true,
          requiresEditScopeId: "editor_boundary:partial",
        }),
        boundary: null,
      },
      {
        id: "editor_boundary:empty",
        kind: "teraBoundary",
        renderInstanceId: null,
        origin: "project",
        capabilities: capabilities({
          readOnly: true,
          requiresEditScopeId: "editor_boundary:empty",
        }),
        boundary: {
          sourceNodeId: "source-empty",
          empty: true,
        },
      },
    ],
  };
}

test("palette drop uses Rust render identity and fails closed outside edit scope", () => {
  const locked = editorNavigationDropTargetStatus({
    editorNavigationSnapshot: snapshot(),
    editorEditScopeGrant: null,
  }, {
    targetRenderInstanceId: "render-partial",
  });
  assert.equal(locked.allowed, false);

  const authorized = editorNavigationDropTargetStatus({
    editorNavigationSnapshot: snapshot(),
    editorEditScopeGrant: { scopeId: "editor_boundary:partial" },
  }, {
    targetRenderInstanceId: "render-partial",
  });
  assert.deepEqual(authorized, {
    allowed: true,
    editorNodeId: "editor_render:partial",
  });
});

test("empty Tera slots resolve only through the Rust boundary identity", () => {
  const authorized = editorNavigationDropTargetStatus({
    editorNavigationSnapshot: snapshot(),
    editorEditScopeGrant: { scopeId: "editor_boundary:empty" },
  }, {
    targetBoundarySourceId: "source-empty",
  });
  assert.deepEqual(authorized, {
    allowed: true,
    editorNodeId: "editor_boundary:empty",
  });

  const stale = editorNavigationDropTargetStatus({
    editorNavigationSnapshot: snapshot(),
    editorEditScopeGrant: { scopeId: "editor_boundary:empty" },
  }, {
    targetBoundarySourceId: "missing",
  });
  assert.equal(stale.allowed, false);
});
