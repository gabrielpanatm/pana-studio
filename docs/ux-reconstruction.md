# Pană Studio UX reconstruction

## Product goal

Pană Studio becomes a Rust-first, code-native visual IDE for Zola. Tera, SCSS,
Markdown, JavaScript and project assets remain the source of truth. The UI is a
projection of validated Rust state and command receipts, not a second project
model.

## Architectural invariants

1. Rust owns restorable product state and semantic operations.
2. `ProjectWorkspace` remains the authority for editable project content,
   history, save and recovery.
3. `WorkbenchRuntime` owns editor navigation state: activities, open documents,
   groups, split layout and the bottom panel. Workbench changes never dirty the
   project or enter file undo/redo history.
4. Every state-changing frontend action is expressed as an intent bound to the
   expected project root, runtime session and revision.
5. Rust validates an intent atomically and returns a typed receipt containing
   the complete new projection.
6. Svelte owns only ephemeral interaction state such as hover, popover position
   and an unfinished drag gesture.
7. New features are introduced as vertical slices. The existing editor stays
   usable until the replacement flow reaches parity.

## Target information architecture

- Activity rail: Editor, Site, Components, Design System, Assets, Content,
  Version Control, Audit and Publish.
- Primary sidebar: navigation belonging to the selected activity.
- Document workbench: tabs, breadcrumbs and one or two editor groups.
- Context toolbar: controls belonging to the current canvas or editor.
- Context inspector: properties of the selected resource or element.
- Bottom panel: Problems, Output, Terminal and Timeline.
- Status bar: session, save, validation, source and background operation state.

## Implementation status

| Phase | Status | Current boundary |
| --- | --- | --- |
| 0–2. Rust contract and persistence | Complete | `WorkbenchRuntime`, session identity, WriteAuthority persistence and typed Tauri receipts |
| 3. Application shell | Complete | Activity Rail, simplified topbar, document bar, contextual sidebars/status and the unified Rust-owned Problems/Output/Terminal/Timeline panel are live |
| 4. Command Center | Complete | Rust-ranked commands, activities, files and Tera symbols with scoped search and `Ctrl+K` |
| 5. Documents and split | Complete | Visual + Code render simultaneously from synchronized Rust groups; orientation, divider ratio, tabs and session restoration persist through Workbench receipts |
| 6. Responsive canvas | Complete | Fit/fixed mode, exact width, free resize, zoom, ruler, SCSS breakpoint context and presets are persisted in the Rust Workbench projection |
| 7. Problems and Audit | Complete | One Rust audit projection feeds both the filterable Audit workspace and bottom Problems; Zola validation, exact source navigation and the preserved Runtime console are integrated |
| 8. Creation workspaces | Complete | Components, Design System, Assets and Content are dedicated workspaces backed by Rust/project projections; class inventory/rename, source usage, frontmatter and taxonomies are integrated |
| 9. Canvas authoring | Complete | Existing code-native visual editing, contextual inspector, responsive canvas and synchronized source remain the authoring path; Mood Board is preserved inside Design System |
| 10. Publish Center | Complete | Release gates, configuration, build/deploy actions, session-bound cancellation and durable Output events form one traceable Rust operation flow |
| 11. Migration completion | Complete | Duplicate deploy/shell paths were removed, controls use the shared workbench language and the complete frontend/Rust verification suite passes |
| 12. Version Control workspace | Complete | Git status, staging, commits, history, branches, remotes, explicit integration and recovery are exposed as a central Rust-owned Workbench activity rather than a local overlay |

## Delivery sequence

### 0. Rust-first migration map

- Record the current command, state and projection boundaries.
- Decide the owner of every new state field before implementation.
- Keep content mutations in `ProjectWorkspace`; keep navigation mutations in
  `WorkbenchRuntime`.

Acceptance: there is no new canonical TypeScript store and no workbench action
can make project content dirty.

### 1. Workbench contract and runtime

- Rust snapshot for activities, documents, groups, split and bottom panel.
- Session binding, monotonic revision and atomic command receipts.
- Bounded document count and project-relative path validation.
- Runtime unit tests for stale identities, no-op commands and group invariants.

Acceptance: the complete navigation state can be read and changed without a
frontend dependency.

### 2. Workbench persistence

- Persist the stable, project-scoped workbench projection through Rust.
- Restore it when a new runtime session opens the same project.
- Reconcile removed or renamed resources against the current project scan.

Acceptance: reopening Pană Studio restores valid documents and layout without
using browser storage as the canonical source.

### 3. New application shell

- Introduce semantic design tokens and reusable controls.
- Build the activity rail, primary sidebar, document bar, contextual inspector,
  bottom panel and status bar around the Rust projection.
- Establish keyboard focus, visible focus rings, labels and minimum hit areas.

Acceptance: the old and new shell can be compared behind a migration boundary,
and every new control is keyboard reachable.

### 4. Command Center

- Build the searchable index in Rust from project scan, Source Graph, component
  registry, SCSS analysis, assets and commands.
- Expose ranked, typed results with executable intents.
- Add `Ctrl+K` and contextual scopes.

Acceptance: a user can navigate to or invoke every primary action without
searching through toolbars.

### 5. Documents and split workbench

- Connect project file opening to Rust-owned tabs.
- Add dirty markers derived from `ProjectWorkspace`, close/reopen and tab order.
- Render visual and code surfaces simultaneously in two synchronized groups.

Acceptance: preview and source can remain open together, and stale receipts can
never switch the visible document.

### 6. Responsive canvas

- Add exact width, free resize, fit, ruler and SCSS-derived breakpoint markers.
- Persist viewport state in the workbench contract.
- Add two-viewport comparison after the single viewport flow is stable.

Acceptance: responsive inspection does not depend on three fixed device icons.

### 7. Problems and Audit

- Aggregate Zola/Tera/SCSS diagnostics, broken references, HTML accessibility,
  SEO, overflow and unused resources in Rust.
- Deduplicate and sort diagnostics; expose locations and safe fixes.
- Navigate from a problem to the exact document, canvas element or setting.

Acceptance: validation has one source, one severity model and actionable
locations.

### 8. Creation workspaces

- Components: partials, macros and interactive components with usage and typed
  contracts.
- Design System: SCSS variables, classes, usage and safe rename/refactor.
- Assets: metadata, usage, replacement, optimization and unused detection.
- Pages and Content: semantic page operations, frontmatter, taxonomies and
  collections.

Acceptance: each workspace is backed by Rust analysis and mutation receipts,
not by direct filesystem writes from the frontend.

### 9. Canvas authoring

- Inline text editing, contextual actions, DOM breadcrumbs and measurements.
- Route every committed edit through existing or new Rust mutation commands.
- Keep preview changes provisional until a receipt is accepted.

Acceptance: the canvas never becomes an independent document model.

### 10. Publish Center

- Unify preflight, build output, changed resources, performance, deploy and
  recovery.
- Keep long-running work cancellable and session-bound.

Acceptance: publishing is a traceable Rust operation with a durable outcome.

### 11. Migration completion

- Remove duplicate shell paths and obsolete local state.
- Normalize terminology and shortcuts.
- Run Rust tests, frontend tests, Svelte checks, production build, accessibility
  review and regression scenarios.

Acceptance: no primary workflow depends on the legacy shell and all baseline
checks pass.

### 12. Version Control workspace

- Add Version Control to the Rust `WorkbenchActivity` contract and persistence.
- Route Activity Rail, Site overview and Command Center navigation through the
  same typed `SetActivity` intent.
- Present status, staging, commits, history, branches, remotes, explicit
  integration and recovery in the central work area.
- Remove the local `versionsPanelOpen` state, fixed drawer and redundant topbar
  entry.

Acceptance: Git is reachable as a primary activity, survives Workbench
restoration and no longer introduces a parallel navigation or overlay model.

## Final verification

- `npm run check`: 0 Svelte/TypeScript errors and 0 warnings.
- `npm run test:kernel`: 31/31 frontend contract and kernel-integration tests.
- `npm run build`: production static build completed.
- `cargo check` and `cargo fmt --check`: completed cleanly.
- `cargo test`: 1001 passed, 0 failed, 2 environment-dependent tests ignored.
- Live AT-SPI review: session restoration, document/split canvas, Publish and
  Design System tab semantics, Mood Board integration and the Rust class
  inventory were verified without writing to the active project.
- Live inspector review for Version Control: canonical activity selection,
  1120×915 central workspace, controls of at least 32 px, text of at least
  11 px and no large fixed overlay.

## Definition of done

The reconstruction is complete only when the workbench, responsive editing,
global navigation, audit and creation workspaces are implemented end to end;
canonical restorable state is Rust-owned; source files remain authoritative;
the legacy UI paths are removed; and the full verification suite passes.
