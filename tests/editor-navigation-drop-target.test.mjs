import assert from "node:assert/strict";
import { test } from "node:test";
import {
  captureEditorMoveNodeAnchor,
  editorNavigationDropTargetStatus,
  resolveEditorMoveNodeAnchor,
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
          boundaryInstanceId: "boundary-empty-locked",
          sourceNodeId: "source-empty",
          empty: true,
        },
      },
      {
        id: "editor_boundary:empty-active",
        kind: "teraBoundary",
        renderInstanceId: null,
        origin: "project",
        capabilities: capabilities(),
        boundary: {
          boundaryInstanceId: "boundary-empty-active",
          sourceNodeId: "source-empty",
          // Preprocessarea poate proiecta un helper DOM drept rădăcină; Rust
          // declară totuși această instanță drept suprafața activă de autor.
          empty: false,
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
    editorEditScopeGrant: null,
  }, {
    targetBoundarySourceId: "source-empty",
    targetBoundaryInstanceId: "boundary-empty-active",
  });
  assert.deepEqual(authorized, {
    allowed: true,
    editorNodeId: "editor_boundary:empty-active",
  });

  const syntheticRenderDoesNotShadowBoundary = editorNavigationDropTargetStatus({
    editorNavigationSnapshot: snapshot(),
    editorEditScopeGrant: null,
  }, {
    targetRenderInstanceId: "render-partial",
    targetBoundarySourceId: "source-empty",
    targetBoundaryInstanceId: "boundary-empty-active",
  });
  assert.deepEqual(syntheticRenderDoesNotShadowBoundary, authorized);

  const ambiguous = editorNavigationDropTargetStatus({
    editorNavigationSnapshot: snapshot(),
    editorEditScopeGrant: null,
  }, {
    targetBoundarySourceId: "source-empty",
  });
  assert.equal(ambiguous.allowed, false);

  const stale = editorNavigationDropTargetStatus({
    editorNavigationSnapshot: snapshot(),
    editorEditScopeGrant: { scopeId: "editor_boundary:empty" },
  }, {
    targetBoundarySourceId: "missing",
    targetBoundaryInstanceId: "boundary-missing",
  });
  assert.equal(stale.allowed, false);
});

test("a move target fails closed when a draft changes every exact identity", () => {
  const before = {
    rootNodeIds: ["root:before"],
    nodes: [
      {
        id: "root:before",
        parentId: null,
        children: ["heading:before", "paragraph:before"],
        kind: "htmlElement",
        tag: "section",
        file: "templates/index.html",
        origin: "project",
      },
      {
        id: "heading:before",
        parentId: "root:before",
        children: [],
        kind: "htmlElement",
        tag: "h1",
        file: "templates/index.html",
        origin: "project",
      },
      {
        id: "paragraph:before",
        parentId: "root:before",
        children: [],
        kind: "htmlElement",
        tag: "p",
        file: "templates/index.html",
        origin: "project",
      },
    ],
  };
  const after = {
    rootNodeIds: ["root:after"],
    nodes: [
      {
        ...before.nodes[0],
        id: "root:after",
        children: ["heading:after", "paragraph:after"],
      },
      {
        ...before.nodes[1],
        id: "heading:after",
        parentId: "root:after",
      },
      {
        ...before.nodes[2],
        id: "paragraph:after",
        parentId: "root:after",
      },
    ],
  };

  const heading = captureEditorMoveNodeAnchor(before, "heading:before");
  const paragraph = captureEditorMoveNodeAnchor(before, "paragraph:before");
  assert.ok(heading);
  assert.ok(paragraph);
  assert.equal(resolveEditorMoveNodeAnchor(after, heading), null);
  assert.equal(resolveEditorMoveNodeAnchor(after, paragraph), null);
});

test("structural move rebasing fails closed when the node shape changed", () => {
  const before = {
    rootNodeIds: ["root"],
    nodes: [{
      id: "root",
      parentId: null,
      children: [],
      kind: "htmlElement",
      tag: "h1",
      file: "templates/index.html",
      origin: "project",
    }],
  };
  const anchor = captureEditorMoveNodeAnchor(before, "root");
  const changed = {
    rootNodeIds: ["replacement"],
    nodes: [{
      ...before.nodes[0],
      id: "replacement",
      tag: "p",
    }],
  };

  assert.ok(anchor);
  assert.equal(resolveEditorMoveNodeAnchor(changed, anchor), null);
});
