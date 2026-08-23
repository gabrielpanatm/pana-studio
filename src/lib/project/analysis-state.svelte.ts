import type { ScssVariable } from "$lib/css/contracts";
import type { WorkspaceDerivedProjectionStatus } from "$lib/session/workspace-mutation-coordinator";
import type { SourceGraph } from "$lib/source-graph/graph-contract";

/** Owns derived project-analysis projections and their exact workspace revision. */
export class ProjectAnalysisState {
  sourceGraph = $state<SourceGraph | null>(null);
  sourceGraphProjectionStatus = $state<WorkspaceDerivedProjectionStatus>("deferred");
  sourceGraphWorkspaceRevision = $state<number | null>(null);
  scssVariables = $state<ScssVariable[]>([]);
  sourceGraphLoadSerial = 0;

  reset() {
    this.sourceGraph = null;
    this.sourceGraphProjectionStatus = "deferred";
    this.sourceGraphWorkspaceRevision = null;
    this.scssVariables = [];
    this.sourceGraphLoadSerial += 1;
  }
}
