# Performance audit — large Zola projects

Date: 2026-07-29

## Scope

The audit used the real Zola root from `studio.pana.tm.ro`:

- 84 scanned entries;
- 62 tracked text files, approximately 181 KB;
- 35 rendered routes;
- approximately 642 KB of generated preview resources.

Measurements were taken in the Tauri development build on a 12-core Linux
workstation. They are appropriate for before/after comparison, but not release
build memory targets.

## User-visible result

| Milestone | Before | First pass | Second pass | Total change |
| --- | ---: | ---: | ---: | ---: |
| ProjectSession opened → Canvas Prepared | 13.690 s | 6.756 s | 5.093–5.795 s | -57.7% to -62.8% |
| ProjectSession opened → Canvas CanonicalVerified | 14.691 s | 7.682 s | 6.127–6.745 s | -54.1% to -58.3% |
| Preview navigation/publication tail | approximately 1.0 s | approximately 1.0 s | 0.950–1.034 s | effectively unchanged |

The browser navigation was not the primary bottleneck. Most of the delay was
spent before navigation, in source projection, model construction and
preparing every rendered route.

## Root causes and implemented corrections

### 1. ProjectModel was built before Preview and then rebuilt inside Preview

The frontend synchronously requested SourceGraph before starting Preview.
Preview then rebuilt the same ProjectModel from the same workspace revision.

The initial Preview now owns the model build, runs it concurrently with the
derived workspace materialization, and publishes the exact model into the
ProjectWorkspace cache. The UI SourceGraph request runs after Preview and
reuses that cached revision.

In the measured run, ProjectModel completed entirely inside the 3.214 s
workspace materialization window, so Preview waited 0 ms for it.

### 2. Rendered routes were processed sequentially

Canvas annotation, HTML sanitization and identity binding are independent for
each route. They now run through a bounded worker set: half of the available
parallelism, capped at six workers. Results and error propagation remain
deterministic by route.

For 35 routes:

- content annotation/preparation fell from 2.457 s to 0.493 s;
- Canvas identity binding fell from 1.045 s to 0.201 s.

### 3. Four complete preview DOM variants were eagerly generated per route

Editor, Visitor, Motion and Interactive HTML were all parsed and serialized
during project open. Only Editor and the script-free Visitor representation
are required for initial Canvas publication.

Motion and Interactive are now generated from the retained exact source only
when requested. Equivalence tests prove that the deferred output is byte-for-
byte identical to the previous eager output.

This also reduced the measured main-process plus WebKit RSS by approximately
40 MB in the same development scenario.

### 4. SourceGraph construction contained quadratic lookups

Parent-node lookup, node-range update and relation deduplication scanned
growing vectors. The builder now maintains node and relation indexes, changing
those operations from linear to amortized constant time.

### 5. Derived workspace creation repeated directory transactions

Every file recreated all of its ancestor directory transactions even when the
directories already existed. Safe, non-symlink directories inside the
validated cache boundary are now recognized before opening a new
WriteAuthority transaction.

For the audited project, baseline Preview materialization fell from 205
filesystem transactions to 100:

- directory transactions: 127 → 22;
- text writes: 72;
- copies: 5;
- derived-tree cleanup: 1.

### 6. Disk polling rewrote the AI/MCP context while nothing changed

The 2-second disk heartbeat changed `checking` and `lastCheckedAt`, which made
Context Hub publish two files and four observability events on every poll.

Heartbeat fields no longer trigger the Svelte publication effect or a semantic
Context Hub revision. They remain fresh in memory when a publication occurs,
while meaningful disk reconciliation state still publishes normally.

After the initial project-state transitions settled, the live audit observed
no periodic MCP context writes.

### 7. Template source annotation rescanned and shifted large strings

For every source node, line and column lookup started again at byte zero.
Annotation then inserted every marker into the middle of a growing `String`.
Both paths become quadratic as a template grows.

Each template now builds one line-start index, and all markers are emitted in
one forward, capacity-planned pass. Unicode columns retain their prior
character-based semantics. In the live project, the observed template
annotation write window fell from approximately 0.55 s to 0.21–0.29 s.

### 8. Canvas rebuilt the same semantic index for every route

The immutable semantic index derived from `ProjectModel` was rebuilt before
annotating each of the 35 rendered documents. A single index is now built once
per Preview candidate and shared read-only across the bounded route workers.

Together with the template correction, this reduced Canvas Prepared from
6.756 s to 5.09–5.80 s across two cold second-pass runs.

### 9. Pointer movement crossed the Canvas/Rust boundary every frame

CanvasAgent published a `pointerMove` on every animation frame. The frontend
then performed a Rust intent resolution followed by a second Rust hover
coordination call, even while the pointer remained on the same semantic
element.

CanvasAgent now emits only when the complete physical hit path changes. A
trusted X11 sweep across the real Canvas generated approximately 2,800 pointer
positions but only 37 CanvasAgent messages: 98.7% of redundant hover traffic
was suppressed. Leave/re-entry and drag transitions remain represented because
their hit paths or gesture kinds differ.

### 10. The external-disk monitor performed blocking recursive work too often

The clean heartbeat replaced the full reactive state object, and the Rust
manifest command recursively scanned the project from a synchronous Tauri
command every two seconds.

Clean heartbeats now mutate only fields whose semantic values changed. The
recursive manifest scan runs on the blocking worker pool, outside the UI
thread. Active/background intervals are 5/15 seconds rather than 2/8 seconds,
reducing idle disk scans by 60% while retaining automatic external-change
detection.

## First-pass Preview phase profile

| Phase | Time |
| --- | ---: |
| Derived source workspace sync | 3.214 s |
| Zola render/build | 1.062 s |
| Route annotation and initial surfaces | 0.493 s |
| Site-wide CanvasGraph | 0.740 s |
| Resource manifest | 0.054 s |
| Canvas identity binding | 0.201 s |
| Complete Preview candidate | 5.814 s |

The remaining time before Canvas Prepared is frontend/session setup and the
Canvas publication boundary.

## Runtime observations

Idle CPU sample in the same development build:

| Process | Before | First pass | Second pass |
| --- | ---: | ---: | ---: |
| Tauri main process | 5.59% | 5.70% | 5.47% |
| WebKit web process | 3.30% | 1.90% | 1.67% |
| Combined | 8.89% | 7.60% | 7.14% |

The WebKit reduction is material. Thread sampling attributed nearly all of the
remaining development main-process CPU to the GTK/UI thread rather than the
Preview, MCP or Tokio workers. Moving disk traversal to a worker removes a
periodic UI-thread blocking boundary even though it does not materially change
the aggregate debug-process average. A packaged release profile is still
required because Vite, WebKit inspection and debug assertions were active.

## Remaining high-priority work

1. Replace the per-file atomic Preview writes with one planned,
   recoverable projection transaction and an atomic staged-tree publication.
   Under ordinary conditions source materialization still dominates the
   pre-render path, and per-file WAL/fsync fan-out creates I/O variance.
2. Build CanvasGraph lazily per visited route, or cache per-route graphs, rather
   than parsing all 35 routes before the first route can be shown.
3. Profile a packaged release build. The application shell currently contains
   approximately 1,341 DOM elements and 598 KB of serialized markup.
4. Add a repeatable large-project performance fixture in CI with budgets for
   candidate time, filesystem transaction count, pointer-message volume and
   idle context writes.
