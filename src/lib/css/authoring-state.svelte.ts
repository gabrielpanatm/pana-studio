import { createDefaultEditableStyles } from "$lib/editor/defaults";
import type { EditableStyles } from "$lib/css/contracts";
import type { InspectorLiveCssIdentity } from "$lib/state/preview-live-controller";

/** Owns local CSS Inspector drafts and live preview layers. */
export class CssAuthoringState {
  variableValues = $state<Record<string, string>>({});
  editableStyles = $state<EditableStyles>(createDefaultEditableStyles());
  overrideRules = $state<Record<string, EditableStyles>>({});
  variableOverrides = $state<Record<string, string>>({});
  targetFile = $state("styles.css");
  liveLayers = $state<Record<string, string>>({});
  liveEpoch = $state(0);
  liveIdentity = $state<InspectorLiveCssIdentity | null>(null);

  reset() {
    this.variableValues = {};
    this.editableStyles = createDefaultEditableStyles();
    this.overrideRules = {};
    this.variableOverrides = {};
    this.targetFile = "styles.css";
    this.liveLayers = {};
    this.liveEpoch = 0;
    this.liveIdentity = null;
  }
}
