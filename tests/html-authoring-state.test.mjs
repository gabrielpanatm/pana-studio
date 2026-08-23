import assert from "node:assert/strict";
import { test } from "node:test";
import { HtmlAuthoringState } from "$lib/editor/html-authoring-state.svelte";

test("pending-ul Inspector agregă sursele fără mutații reactive redundante", () => {
  let mutations = 0;
  const state = new HtmlAuthoringState(() => { mutations += 1; });

  state.setInspectorPending("css", true, "inspector-pane");
  state.setInspectorPending("css", true, "motion-timeline");
  state.setInspectorPending("css", false, "inspector-pane");
  assert.equal(state.inspectorPending.css, true);
  state.setInspectorPending("css", false, "motion-timeline");

  assert.equal(state.inspectorPending.css, false);
  assert.equal(mutations, 2);
});

test("pending-ul HTML proiectează o singură stare agregată în Inspector", () => {
  let mutations = 0;
  const state = new HtmlAuthoringState(() => { mutations += 1; });

  state.setHtmlPending("attributes", true);
  state.setHtmlPending("classes", true);
  state.setHtmlPending("attributes", false);
  assert.equal(state.inspectorPending.html, true);
  state.setHtmlPending("classes", false);

  assert.equal(state.inspectorPending.html, false);
  assert.equal(Object.values(state.htmlPending).some(Boolean), false);
  assert.equal(mutations, 2);
});

test("reset elimină drafturile și toate sursele pending", () => {
  const state = new HtmlAuthoringState(() => {});
  state.classEditorValue = "hero active";
  state.imageSourceValue = "/images/hero.webp";
  state.pendingTag = "section";
  state.setInspectorPending("js", true, "motion-timeline");
  state.setHtmlPending("text", true);

  state.reset();

  assert.equal(state.classEditorValue, "");
  assert.equal(state.imageSourceValue, "");
  assert.equal(state.pendingTag, null);
  assert.equal(Object.values(state.htmlPending).some(Boolean), false);
  assert.equal(Object.values(state.inspectorPending).some(Boolean), false);
});
