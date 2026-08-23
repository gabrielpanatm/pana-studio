<script lang="ts">
  import type { ComponentProps } from "svelte";
  import InspectorPane from "$lib/components/InspectorPane.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import {
  getFontManager,
} from "$lib/fonts/io";
  import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
  import type { MotionWorkspaceState } from "$lib/motion/workspace.svelte";
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";
  import type { InstalledFontVariationAxis } from "$lib/fonts/contracts";

  type ForwardedInspectorPaneProps = Omit<
    ComponentProps<typeof InspectorPane>,
    | "motionWorkspace"
    | "fontFamilies"
    | "installedFontAxes"
    | "blockPropertiesHeight"
    | "blockPropertiesCollapsed"
    | "persistBlockPropertiesLayout"
  >;

  let {
    visible,
    sessionId,
    interactionLocked,
    paneProps,
    applicationPreferences,
    motionWorkspace,
    workspaceMutations,
    workspaceLayout,
  }: {
    visible: boolean;
    sessionId: string;
    interactionLocked: boolean;
    paneProps: ForwardedInspectorPaneProps;
    applicationPreferences: ApplicationPreferencesState;
    motionWorkspace: MotionWorkspaceState;
    workspaceMutations: ProjectWorkspaceMutationService;
    workspaceLayout: WorkspaceLayoutState;
  } = $props();

  const editorSidebarActive = $derived(visible);
  let installedFontFamilies = $state<string[]>([]);
  let installedFontAxes = $state<InstalledFontVariationAxis[]>([]);
  let fontLoadSequence = 0;

  $effect(() => {
    const snapshot = workspaceMutations.snapshot;
    if (!snapshot) {
      installedFontFamilies = [];
      installedFontAxes = [];
      return;
    }
    const requestId = ++fontLoadSequence;
    const expectedRevision = snapshot.revision;
    void getFontManager({
      expectedProjectRoot: snapshot.projectRoot,
      expectedSessionId: snapshot.runtimeSessionId,
      expectedRevision,
    }).then((manager) => {
      if (
        requestId !== fontLoadSequence
        || workspaceMutations.snapshot?.revision !== expectedRevision
      ) return;
      installedFontFamilies = manager.graph.families
        .filter((family) => family.registration.registered && family.delivery !== "missing")
        .map((family) => family.family);
      const axes = manager.graph.families
        .filter((family) => family.registration.registered && family.delivery !== "missing")
        .flatMap((family) => family.files.flatMap((file) => (
          file.axes.map((axis) => ({ family: family.family, ...axis }))
        )));
      installedFontAxes = axes.filter((axis, index) => axes.findIndex((candidate) => (
        candidate.family === axis.family
        && candidate.tag === axis.tag
        && candidate.min === axis.min
        && candidate.default === axis.default
        && candidate.max === axis.max
      )) === index);
    }).catch(() => {
      if (requestId === fontLoadSequence) {
        installedFontFamilies = [];
        installedFontAxes = [];
      }
    });
  });
</script>

{#if !workspaceLayout.rightPaneCollapsed && editorSidebarActive}
  <WorkspaceResizeHandle
    kind="right"
    active={workspaceLayout.activeResizeKind === "right"}
    ariaLabel={t("workbench-resize-right-panel")}
    onDrag={(event) => workspaceLayout.startResizeDrag("right", event)}
    onReset={() => workspaceLayout.resetResize("right")}
  />
{/if}

{#if workspaceMutations.snapshot && sessionId}
  {#key sessionId}
    <div
      class="inspector-pane-shell"
      hidden={workspaceLayout.rightPaneCollapsed}
      inert={!editorSidebarActive
        || workspaceLayout.rightPaneCollapsed
        || interactionLocked
        ? true
        : undefined}
      aria-hidden={!editorSidebarActive || workspaceLayout.rightPaneCollapsed}
      aria-busy={interactionLocked}
    >
      <InspectorPane
      {...paneProps}
      {motionWorkspace}
      blockPropertiesHeight={applicationPreferences.snapshot?.blockPropertiesHeight ?? 220}
      blockPropertiesCollapsed={applicationPreferences.snapshot?.blockPropertiesCollapsed ?? false}
      fontFamilies={installedFontFamilies}
      {installedFontAxes}
      persistBlockPropertiesLayout={(height, collapsed) => {
        void applicationPreferences.persistBlockPropertiesLayout(height, collapsed);
      }}
      />
    </div>
  {/key}
{/if}
