# Release 0.1.2 performance audit

Date: 2026-07-29

Artifact:
`src-tauri/target/release/bundle/appimage/Pană Studio_0.1.2_amd64.AppImage`

This report continues the development-build audit in
`docs/performance-audit-2026-07-29.md`. No performance correction was applied
during this release audit.

## Executive conclusion

Release 0.1.2 is materially faster than the development build, but it is still
not production-usable for sustained work on the audited Zola project.

The dominant release problems are:

1. **P0 — returning to Editor destroys and recreates the Canvas iframe.**
   A warmed Editor return takes approximately 636–664 ms median and retains
   approximately 15–18 MB of WebKit PSS per return. WebKit PSS grew from
   approximately 295 MB to 678–700 MB over 40 Editor/Teme transitions.
2. **P0 — Canvas hover still has a serialized two-command Rust path.**
   Pointer messages are now deduplicated correctly, but every accepted target
   change performs intent resolution and then a second hover-coordination IPC.
   The currently executing stale hover is not cancelled after the first await,
   so it blocks the newest pointer position.
3. **P1 — initial Preview projection performs 100 independent durable
   filesystem transactions.** Their serialized commit latency alone consumes
   627–681 ms, approximately 30% of the session-to-Prepared interval.
4. **P1 — the application does not reach true idle.** In a clean Wayland
   release sample, the main process and WebKit consumed a combined 3.60% of one
   CPU core with no user action and no new observability events.
5. **P1 — the complete workspace catalog is eagerly loaded.** The main
   minified JavaScript chunk is 2.17 MB and all JavaScript is 3.70 MB. Fifteen
   workspace components are statically imported by the central workspace.

These are architectural costs. Further small local optimizations will not make
the application feel fast until the two P0 paths are corrected.

## Scope and method

The real Zola root was:
`/home/gabriel/Documente/studio.pana.tm.ro/sursa`

Project shape:

- 84 scanned entries;
- 62 tracked text files, approximately 181 KB;
- 35 rendered routes;
- 13 initial Preview resources, approximately 642 KB.

Environment:

- packaged AppImage release 0.1.2;
- Linux workstation with 12 logical CPUs;
- native Wayland for project-open, idle, memory and activity-switch tests;
- XWayland only for the synthetic trusted pointer sweep;
- isolated XDG config, data, cache and state roots under `/tmp`;
- two completely new profiles and one same-profile process restart.

`ProjectSession opened` is the first authoritative Rust session event. Native
file-picker time and application boot time are intentionally excluded from the
project-open numbers.

“New profile” means application caches are empty. The operating-system page
cache was not dropped, so the measurements are controlled application-cold
runs, not physical-disk cold boots.

CPU percentages reported by `pidstat` use one logical CPU as 100%.

## Release measurements

### Project open

| Scenario | Session → Prepared | Prepared → Canonical | Session → Canonical | styledReady |
| --- | ---: | ---: | ---: | ---: |
| New profile A | 2.127 s | 1.206 s | 3.333 s | 409 ms |
| New profile B | 2.280 s | 1.142 s | 3.422 s | 438 ms |
| Same-profile process restart | 2.319 s | 1.011 s | 3.330 s | 436 ms |

The two new-profile means are:

- Session → Prepared: 2.204 s;
- Session → Canonical: 3.378 s.

The process-restart result is within measurement noise of the new-profile
results. Release caches preserved between processes do not reduce project-open
latency. This is consistent with the Preview engine explicitly removing the
editor cache at every runtime start and beginning with no projection manifest.

Release is approximately twice as fast as the 5.09–5.80 s development
Session-to-Prepared result, but the user still waits approximately 3.3–3.4 s
for a canonical Canvas.

### Preview projection transaction cost

Each controlled open performed:

| Operation | Count | Total commit latency | Mean | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Create directory | 23 | 269 ms | 11.70 ms | 35 ms |
| Copy | 5 | 78 ms | 15.60 ms | 18 ms |
| Write text | 72 | 334 ms | 4.64 ms | 8 ms |
| **Total** | **100** | **681 ms** | **6.81 ms** | **35 ms** |

The other new-profile run recorded 627 ms total serialized commit latency for
the same 100 transactions. This is 29.5–29.9% of the complete
Session-to-Prepared interval.

Ten project templates are written once as workspace overlays and then written
again after annotation. The second write is avoidable if annotation is fused
with initial materialization.

### Clean idle

Forty-five-second native-Wayland sample:

| Process | Mean CPU | RSS | PSS |
| --- | ---: | ---: | ---: |
| Tauri main | 1.69% | approximately 315 MB | approximately 228 MB |
| WebKit network | 0.00% | approximately 53 MB | approximately 24 MB |
| WebKit web | 1.91% | approximately 376 MB | approximately 288 MB |
| **Total** | **3.60%** | **approximately 744 MB** | **approximately 540 MB** |

A second clean profile settled at approximately 775 MB aggregate RSS and
571 MB aggregate PSS.

The idle sample also produced approximately 34 minor faults/second in the main
process and 134 minor faults/second in WebKit. WebKit had idle spikes of
11–13% CPU. No new kernel observability event occurred during the sample.

Thread sampling attributed measurable idle work to:

- the GTK/application main thread;
- the persistent Preview listener;
- WebKit's main and receive-queue threads.

Code-level periodic sources include:

- AI coordination IPC every 500 ms;
- a complete recursive project manifest scan every 5 seconds while focused;
- a nonblocking Preview `accept()` loop that wakes every 15 ms.

### Activity-switch latency and memory retention

Two series of ten Editor → Teme cycles were executed through AT-SPI. Completion
was detected from the target activity's active accessibility state.

Warmed results:

| Activity | Typical median | p95 | Maximum |
| --- | ---: | ---: | ---: |
| Editor | approximately 636–664 ms | 793 ms | 824 ms |
| Teme | approximately 155–162 ms | 181–203 ms | 203 ms |

During the first transition sample:

- Tauri main mean CPU: 10.89%, peak 28%;
- WebKit mean CPU: 43.56%, peak 122%;
- WebKit RSS rose from approximately 428 MB to 541 MB inside the sample.

Memory progression:

| Point | WebKit PSS |
| --- | ---: |
| Clean project-open baseline | approximately 295 MB |
| After first 20 Editor/Teme transitions | approximately 515 MB |
| Immediately after the next 20 transitions | approximately 700 MB |
| Settled after the next 20 transitions | approximately 678 MB |

After the second series, WebKit RSS did not fall during a 30-second observation
window; it rose from approximately 789 MB to 791 MB.

Control: twenty transitions between Teme and Date, with no Editor/Canvas
mount, increased WebKit PSS by only 8.6 MB. After subtracting this general
workspace cost, the settled retained increase is approximately 15 MB per
return to Editor; the immediate increase is approximately 18 MB per return.

The code path explains the result:

- `WorkspaceCenterArea.svelte` selects exactly one activity through a single
  `{#if}` chain;
- `EditorShell` exists only in the final Editor branch, so leaving Editor
  destroys it;
- `EditorShell` owns the Preview iframe, so returning to Editor constructs and
  loads a new Canvas document;
- Interactive or Motion mode can add a second iframe.

This is the strongest release blocker because it combines visible latency,
CPU saturation and monotonic memory retention in an ordinary navigation path.

### Pointer movement over Canvas

The Canvas bridge already coalesces pointer movement to animation frames and
suppresses messages while the physical hit path is unchanged. That correction
is effective and must be retained.

The remaining frontend path is still serialized:

1. `resolve_canvas_interaction_intent`;
2. `apply_selection_intent` for `setHover` or `clearHover`;
3. overlay projection back into the iframe.

The gesture tail checks whether a pointer message is stale before starting
`resolveGesture`, but does not check it again after the first Rust await. An
old in-flight pointer position can therefore execute the second IPC and render
before the newest position is allowed through the serialized tail.

A synthetic sweep injected approximately 3,780 X11 pointer positions over
15.4 seconds across the Canvas area. A 30-second sample containing the sweep
measured:

| Process | Idle mean | Sample mean | Sample peak |
| --- | ---: | ---: | ---: |
| Tauri main | 1.67% | 9.33% | 19% |
| WebKit web | 1.33% | 20.97% | 84% |

The 30-second mean includes the idle ramp before and after the sweep, so active
Canvas cost is higher than the reported mean.

This XWayland test is directional rather than a native-Wayland absolute
benchmark, but it exercises the same Canvas bridge, Svelte queue and Rust
commands.

### Frontend payload

Release assets:

| Asset group | Raw | gzip |
| --- | ---: | ---: |
| Main JavaScript chunk | 2,167,415 bytes | 555,659 bytes |
| Main CSS asset | 377,907 bytes | 51,368 bytes |
| All JavaScript chunks | 3,701,720 bytes | not measured |

The main chunk is 58.6% of all JavaScript. `WorkspaceCenterArea.svelte`
statically imports Editor plus fourteen other workspaces. The source has 94
Svelte components, but only a small number of component-level dynamic imports.

Because Tauri serves local assets, network gzip size is less important than
the 2.17 MB that WebKit must decode, parse and compile for the main chunk. Eager
workspace imports also raise the baseline heap even when most activities are
never opened.

## Root causes and required corrections

### P0. Keep the Editor/Canvas surface stable across activity changes

Do not place `EditorShell` inside the mutually exclusive activity branch.
Hoist one Editor instance to a stable project-session owner and hide it with
`hidden`, `inert` and `aria-hidden` while another activity is visible.

The stable owner must:

- keep the canonical Preview iframe alive while the project session is alive;
- deactivate Canvas pointer/keyboard interaction while hidden;
- pause resize/measurement observers that do not need to run in the
  background;
- destroy the iframe exactly once on project close or session replacement;
- preserve the existing surface-generation and stale-message guards.

This change should remove both the 600–800 ms Editor return and the
15–18 MB-per-return retention slope.

If WebKit still retains memory after a stable owner is implemented, capture a
release WebKit heap snapshot and inspect detached frames and event-listener
roots. The current structural remount must be removed first because it creates
the retention stimulus on every ordinary navigation.

### P0. Make hover a dedicated latest-wins lane

Pointer hover must not share a serialized promise tail with click and drag
semantics.

Required shape:

- keep only one latest pending pointer target;
- after every await, compare gesture sequence and binding again before doing
  more work;
- never execute `applyHoverIntent` or overlay rendering for a stale result;
- combine Rust intent resolution and hover projection into one command/receipt
  where the security model permits it;
- keep click, context menu, drag start and drop in their ordered semantic lane.

At minimum, add a stale-sequence check immediately after
`resolveCanvasInteractionIntent` and immediately after `applyHoverIntent`.
That is a safe first reduction, but the final target is one Rust round trip per
accepted target change.

### P1. Replace per-file Preview WAL/fsync fan-out with staged publication

Preview is a derived, recoverable cache. It still needs strict path authority
and symlink protection, but it does not need one durable application WAL
transaction per derived file.

Build the complete projection in a new descriptor-bound generation directory:

1. validate every relative path and authority boundary;
2. write and annotate files directly inside the unpublished generation;
3. sync files/directories according to one generation policy;
4. atomically publish the prepared directory;
5. fsync the publication parent once;
6. retire the prior generation after readers release it.

Also preprocess each template before its initial write so the ten annotation
rewrites disappear.

Target: no more than one logical projection transaction and fewer than ten
durability syscalls for this fixture, instead of 100 full WriteAuthority
transactions.

### P1. Replace permanent polling with event-driven idle

- AI coordination: emit a Tauri event when authority changes and schedule only
  the exact lease-expiry deadline in Rust. Remove the unconditional 500 ms
  frontend IPC.
- External disk: use a native file watcher with debounce and rescan only
  affected subtrees. Keep a low-frequency full manifest audit as a safety
  backstop, not a five-second primary mechanism.
- Preview HTTP: use a blocking listener with a stop wakeup or an async listener
  instead of `WouldBlock` plus a 15 ms sleep loop.

Target: under 1% combined main-plus-WebKit CPU after 30 seconds of clean idle,
with no recurring project-sized allocations.

### P1. Lazy-load non-Editor workspaces

Replace static workspace imports with cached dynamic component imports.
Preload a workspace on activity-button hover/focus and keep only the current
workspace module resident by default. Terminal's existing lazy-loading pattern
is an appropriate starting point.

The stable Editor/Canvas owner is deliberately different: its DOM instance
should remain mounted after first use to avoid frame churn.

### P2. Reduce the cold Canvas publication tail

Prepared-to-Canonical remains 1.01–1.21 seconds. The styled-ready phase alone
is 409–438 ms. After the stable iframe work, profile stylesheet parsing,
font/resource readiness and bridge startup inside the Preview document.

Do not optimize this tail before removing Editor remounts: remounting currently
repays the entire navigation/style/bridge cost during normal activity changes.

### P2. Make safe cross-process Preview reuse possible

The Preview engine currently deletes all editor residue on every process
start. A safe cache could retain a content-addressed, immutable generation
whose manifest is verified against the newly accepted disk snapshot before it
is published into the new runtime session.

This is lower priority than batching because the current same-profile restart
shows no warm benefit, and verifying/reusing 100 independently journaled files
would preserve much of the existing complexity.

## Recommended implementation order

1. Stable Editor/Canvas lifetime.
2. Latest-wins hover lane and one-command hover receipt.
3. Staged Preview projection with fused template annotation.
4. Event-driven AI coordination, filesystem watch and blocking/async Preview
   listener.
5. Dynamic imports for non-Editor workspaces.
6. Cold Canvas styled-ready profile and optional verified cross-process cache.

The first two items should be implemented and measured together in a dedicated
release build before starting another broad optimization pass.

## Acceptance gates for the next release candidate

Use the same project fixture and automated scenarios.

| Gate | Release 0.1.2 | Required next candidate |
| --- | ---: | ---: |
| New-profile Session → Canonical p95 | approximately 3.42 s | at most 2.5 s |
| Editor return p95 | approximately 793 ms | at most 100 ms |
| WebKit PSS change after 50 Editor/Teme cycles | monotonic, hundreds of MB | at most 20 MB, no positive linear slope |
| Preview projection transactions | 100 | one logical publication, fewer than 10 durability operations |
| Clean idle main + WebKit CPU | 3.60% | below 1.0% |
| Pointer sweep main + WebKit sample mean | 30.30% | below 15%, with no stale hover projection |
| Aggregate clean-project PSS | approximately 540–571 MB | at most 450 MB |

CI should retain the fixture and fail on regression in:

- project-open milestones;
- Preview transaction count;
- Editor-return latency;
- memory slope across repeated activity changes;
- Canvas pointer/hover command volume;
- clean-idle CPU and manifest-poll volume.

## Implementation progress

### Stage 1 — stable Editor/Canvas lifetime: accepted

The first remediation stage was implemented and remeasured in a packaged
release build. Rust remains authoritative for the ProjectSession and Workbench
activity. The frontend now keeps one Editor, Canvas iframe, Project explorer
and Inspector owner per Rust `ProjectSession`; inactive surfaces are inert and
hidden without destroying the iframe.

The Workbench `SetActivity` receipt is applied immediately by the authoritative
Rust runtime. Persistence is scheduled in a serialized Rust write-behind lane
which refuses to overwrite a newer on-disk revision. Other Workbench intents
retain their synchronous persisted semantics.

The final activity layout keeps the Editor's grid geometry stable. A retained
non-Editor workspace is a direct absolute sibling of that grid, rather than a
child extended outside a clipping parent. An intermediate negative-inset
`overflow` prototype was rejected: although it initially produced
approximately 53 ms Editor p95, WebKit PSS grew linearly by tens of megabytes
per 50 cycles and latency degraded after repeated use.

Final measured artifact:

`src-tauri/target/release/bundle/appimage/Pană Studio_0.1.2_amd64.AppImage`

- SHA-256:
  `96edbdde0cd8b7b155b3a87634434deeaac59a68f18ce40cf3dd5b42e678a664`;
- size: 108,947,960 bytes;
- fixture: 84 scanned files;
- one `Prepared` and one `CanonicalVerified` Canvas transaction for the whole
  run; no new Preview transaction during activity cycles.

After 50 warm-up cycles, two independent 50-cycle measurement intervals with
500 ms settlement produced:

| Interval | Editor median | Editor p95 | Editor max | Total PSS |
| --- | ---: | ---: | ---: | ---: |
| Warm baseline | — | — | — | 631,644 KB |
| First 50 cycles | 79.583 ms | 94.803 ms | 103.068 ms | 648,267 KB |
| Second 50 cycles | 82.452 ms | 91.091 ms | 92.828 ms | 634,997 KB |

The first interval changed aggregate PSS by +16,623 KB, below the 20 MB gate.
The second interval released 13,270 KB and ended only +3,353 KB above the
warmed baseline, so the rejected linear retention slope is absent.

Validation:

- `npm run check`: 0 errors and 0 warnings;
- `npm run test:kernel`: 61/61 test files passed;
- `cargo fmt --check`: passed;
- Rust library suite from the implementation pass: 1,220 passed, 0 failed,
  2 ignored;
- dedicated Rust persistence concurrency regression: passed.

Stage 1 therefore passes its two release gates: Editor return p95 is at most
100 ms and aggregate PSS changes by at most 20 MB after 50 cycles without a
positive linear slope. This does not claim that the Editor itself is generally
fast. Pointer/hover, selection projection, Preview publication, idle work and
bundle loading remain separate open stages below.

### Stage 2 — latest-wins Canvas hover: accepted with a harness limitation

Pointer hover now has a dedicated Rust-first lane. It no longer enters the
ordered click/context/drag promise tail. The frontend retains at most one
in-flight hover and one newest pending physical target, and rechecks the
binding, generation and gesture sequence after every await before projecting
the receipt.

The new `resolve_canvas_hover_intent` command performs target resolution and
hover coordination in one Rust round trip. Rust serializes this lane through
its Canvas interaction runtime, rejects stale or non-monotonic sequences
before projection, and returns only the minimal hover projection
(`changed` plus the optional hover snapshot). It does not clone or serialize
the selection or Inspector summary. Click, context menu and drag remain in
their independent ordered lane.

The Canvas bridge also stopped doing continuous semantic hover work. A passive
physical pointer listener resets a 120 ms dwell, then emits only the final
resting target. The Rust-approved visual is an outline attached through a
temporary data attribute on the already identified element; normal hover no
longer performs `querySelector`, `getBoundingClientRect` or fixed-overlay
layout work. Pointer leave still clears hover immediately.

Live verification on the release artifact traversed approximately 3,800 X11
positions over 15.76 seconds. Runtime-only instrumentation observed exactly
two pointer gestures: the boundary clear and the final target after dwell.
Sequences were monotonic (`nonMonotonic = 0`), so no stale hover was projected.

Clean 30-second samples on the final artifact:

| Scenario | Tauri main | WebKit web | Combined |
| --- | ---: | ---: | ---: |
| Canvas agent active, pointer sweep | 8.53% | 9.80% | 18.33% |
| Same process/sweep, Canvas agent deactivated | 8.50% | 8.67% | 17.17% |

The Canvas hover path therefore adds approximately 1.16 percentage points in
this controlled X11 scenario. The absolute `< 15%` acceptance value cannot be
demonstrated with this harness because the same application, website and
synthetic sweep already consume 17.17% with the Canvas agent completely
deactivated. This is recorded as a measurement-floor limitation, not as an
assertion that the absolute gate passed. WebKit inspector DOM instrumentation
was excluded from these numbers because its invalidation event stream itself
raised WebKit CPU to 70–90%.

Final Stage 2 artifact:

`src-tauri/target/release/bundle/appimage/Pană Studio_0.1.2_amd64.AppImage`

- SHA-256:
  `052e226a2edd66c54bc0d13cc14aea2f7f17ffdf651b69ee298b036d4719882e`;
- size: 108,915,192 bytes.

Validation:

- `npm run check`: 0 errors and 0 warnings;
- `npm run test:kernel`: 61/61 test files passed;
- Rust library suite before the final dedicated regression: 1,221 passed,
  0 failed, 2 ignored;
- dedicated Rust latest-wins hover, minimal hover projection and Canvas-agent
  transport regressions: passed;
- release bundle: passed.

The implementation requirement and stale-hover gate are accepted. Absolute
CPU remains an open environment-sensitive metric for the final profiling
stage, where a native pointer source or a separately calibrated harness is
required.

### Stage 3 — staged Preview source publication: accepted in dev mode

Preview no longer updates the public Zola source tree through one durable
`WriteAuthority` transaction per file. Each workspace revision now
materializes a complete private generation below the installed Application
Preview cache authority. Relative paths are validated, all traversal is
descriptor-bound, ancestor symlinks are refused, directories and files are
create-only, and the public `source` name changes through one atomic directory
publication.

The fast path remains inside the existing filesystem trust base. Low-level
`rustix` operations live only in `kernel/write_authority/capability.rs`; the
Preview generation module only orchestrates scoped capability operations.
The path is restricted to `ApplicationPreviewCache`, uses mode `0700` for
private directories and `0600` for files, and retains the normal sealed
authority and path-identity checks.

Template annotation is fused with materialization, before the first and only
write of a template. Authoritative text comes from the exact in-memory
`WorkspaceProjectionLease`; accepted binary files are read through the active
project read lease and bounded descriptor reads. A failed pre-publication
generation cannot affect the visible source tree. After publication, the
previous tree stays privately named until Zola rendering and model/artifact
preparation release it, then it is retired through the bounded rebuildable
tree remover.

The rebuildable Preview cache deliberately does not `syncfs` the entire
filesystem or fsync every source file. The complete private namespace is
sealed once, then the atomic publication synchronizes the session parent. This
preserves in-process completeness for a cache which is discarded at the next
process start, without paying persistent-data durability semantics per file.

Final live dev-mode observation on `studio.pana.tm.ro`:

| Metric | Observed |
| --- | ---: |
| Materialized source entries | 84 |
| Materialized source bytes | 840,273 |
| Logical source publications | 1 |
| Source durability operations | 2 |
| Residual private staging directories | 0 |
| Remaining Preview lifecycle operations | 5 |

The five remaining lifecycle operations are the cleanup/create of the
top-level editor/session containers plus the artifact container and one
revision artifact directory. They produce ten audit records because every
operation has a planned and committed event; they are not per-source-file
transactions.

The structured
`kernel.preview_projection.generation.published` event records publication,
durability, entry and byte counts for both initial session startup and later
workspace revisions. In the final dev observation, Session → publication was
5,458 ms, publication → Prepared 16 ms, and Prepared → Canonical 1,231 ms.
These unoptimized dev timings are diagnostic only; the absolute 2.5-second
release gate remains assigned to the final profiling stage.

Validation:

- `npm run test:kernel`: 61/61 test files passed;
- Rust library suite: 1,225 passed, 0 failed, 2 ignored;
- capability trust-base compliance regression: passed;
- traversal and symlink-ancestor refusal regression: passed;
- one-publication/two-durability-operation/retirement regression: passed;
- real ignored Zola multi-revision integration (template, Sass, invalid
  candidate and last-good retention): passed;
- `cargo fmt --check`: passed before the final trust-base refactor and is
  rerun as part of the stage close.

Stage 3 passes its publication gate: one logical publication and fewer than
ten durability operations. No AppImage was rebuilt for this intermediate
stage; further work continues in dev mode and the release artifact is deferred
until the final validation stage.

### Stage 4 — event-driven idle: accepted in dev mode

The three known periodic wakeup sources are now event-driven:

- AI coordination publishes a typed Tauri event after every successful Rust
  transition. Pending requests and active leases use one exact Rust deadline
  task guarded by a monotonically invalidated generation; the frontend no
  longer reads coordination state every 500 ms.
- both local Preview HTTP listeners block in `accept()`. Shutdown sets the
  stop flag and opens one loopback connection to wake the listener; the former
  15 ms non-blocking accept loops no longer exist.
- external project changes use one recursive Linux `inotify` listener.
  Filesystem bursts are coalesced for 240 ms and published with exact
  project-root, runtime-session, watcher-generation and revision identity.
  The former full-manifest reads every 5/15 seconds were removed.

Watcher lifecycle transitions are serialized in Rust. A stop command must
present the exact watcher generation, so a stale frontend continuation cannot
stop a newer watcher for the same project session. Save, undo/redo and project
transition continue to suspend the watcher and drain the exact in-flight
manifest/reconciliation operation before their first persistent effect.
Derived/internal directories and symlinks are not watched. A five-minute
full-manifest audit remains as a low-frequency loss/overflow safety net; normal
operation scans only after a debounced native event.

Live dev-mode idle sample after opening the 84-file `studio.pana.tm.ro`
fixture:

| Process | 30-second average CPU |
| --- | ---: |
| Tauri main | 0.00% |
| WebKit network | 0.00% |
| WebKit web | 0.03% |
| Combined | 0.03% |

The measurement used `pidstat` at one-second resolution after the Editor was
fully visible. One WebKit interval reached 1%; every other measured process
interval was 0%. This passes the stage target of less than 1% combined idle
CPU in dev mode.

Validation:

- `npm run check`: 0 errors and 0 warnings;
- `npm run test:kernel`: 62/62 test files passed;
- Rust library suite: 1,227 passed, 0 failed, 2 ignored;
- native watcher event/debounce/stop regression: passed in 240 ms;
- event-driven idle source contract: passed;
- `cargo fmt --check`: passed;
- `git diff --check`: passed before this documentation append.

No AppImage was built for Stage 4. The final release build remains deferred to
the final profiling stage.

### Stage 5 — lazy workspaces and retained Editor runtime: accepted in dev mode

All non-Editor workspaces are now dedicated dynamic chunks loaded on first
activation. `EditorShell` remains the only static center workspace and stays
owned by the exact `ProjectSession`. Only the last auxiliary workspace is
retained while returning to Editor, so repeated Editor/activity navigation
does not continuously remount either side.

The live trace exposed a larger cost than JavaScript chunk loading: returning
to Editor deactivated the Canvas agent and synchronously rebuilt the complete
`ProjectModel` before the browser could paint the already retained iframe.
The frontend Canvas runtime now has an exact `suspended` phase. When the
session, Canvas identity, route and document epoch are unchanged, it retains
the Rust binding receipt, disables input while hidden and reactivates the same
binding on return. A separate interaction generation invalidates queued work
across suspension, and every continuation still checks its generation after
each `await`. A changed session, revision, Canvas identity or document epoch
continues through the full fail-closed bind path.

CodeMirror follows the same stable-owner rule. Activity changes no longer
destroy it. The controller records its physical host identity and is destroyed
only when Svelte removes or replaces that host, including a ProjectSession
replacement or Markdown surface. Reactivation requests a fresh CodeMirror
measurement without recreating its document state. A live DOM-identity check
across `Cod → Teme → Editor` confirmed that the same CodeMirror instance was
retained.

Cold auxiliary commands were also removed from WebKit's UI thread and from
long-lived Rust mutex guards:

- ProjectModel traversal/validation runs after capturing an exact build lease,
  outside the live session and workspace locks, then revalidates before cache
  publication.
- Workbench intent handling releases the project-workspace lock immediately
  after cloning the exact session.
- CSS, font and Design System reads use exact immutable projections and
  `spawn_blocking`; root, session, revision and accepted disk authority are
  revalidated before returning their receipt.
- Design System initially reads only the active subview. Its default global
  styles view no longer starts token, class and font scans concurrently.

The original live symptom was a full iframe repaint after a synchronous
Canvas bind. Before retained binding reactivation, two animation frames after
returning to Editor took approximately 1.3–1.5 seconds. After the change, a
20-cycle `Editor ↔ Teme` dev sample produced:

| Metric | Median | p95 | Maximum |
| --- | ---: | ---: | ---: |
| Editor activity receipt projected | 56.0 ms | 75.7 ms | 165.0 ms |
| Editor two-frame settlement | 87.0 ms | 106.5 ms | 211.0 ms |

The maximum was the first cycle after the cold auxiliary setup. The remaining
19 Editor activations were between 53 and 71 ms. The acceptance gate is
defined at activity projection, so the measured p95 passes the 100 ms target.

After changing Design System from four unconditional cold reads to one
active-subview read, a cold component mount was 105 ms to activity projection
and 214 ms to two-frame settlement. Returning immediately to Editor was 90 ms
to projection and 123 ms to settlement. Before the Rust lock/offload and
active-subview changes, the same return took 202 ms to projection, 288 ms to
paint and 473 ms end-to-end; before lock isolation it could remain blocked for
minutes.

Validation:

- `npm run check`: 0 errors and 0 warnings;
- `npm run test:kernel`: 62/62 test files passed;
- `cargo fmt --check`: passed;
- `cargo check --lib`: passed with the nine pre-existing dead-code warnings;
- stable Editor/CodeMirror lifetime, lazy chunk routing, active Design System
  subview and async Canvas bind contracts: passed;
- live CodeMirror identity retention: passed;
- live Editor return p95: passed in dev mode.

No AppImage was built for Stage 5. The 50-cycle PSS gate, release-mode
Session → Canonical tail, aggregate PSS and final release bundle remain in
Stage 6.

### Stage 6 — cold Canvas profile and final release gates

The final stage first added structured timing at every expensive candidate
boundary and at the Canvas bridge. The profile separated Rust publication
from browser navigation, font settlement and the two required styled frames.
This prevented browser variance from being misattributed to Zola or to the
Canvas transaction.

The Rust-first cold path now:

- builds `ProjectModel` concurrently with the independent Zola render and
  joins it only after Zola finishes;
- annotates and parses the rendered documents in a bounded, deterministic
  per-document parallel map;
- uses an exact string fast path when binding Canvas identity to prepared
  Editor HTML instead of reparsing every document;
- builds the immutable resource manifest concurrently with content
  preparation and `CanvasGraph`;
- acknowledges the exact canonical Canvas phase sequence in one validated
  Rust command and one batched observability append;
- navigates the first Editor iframe directly to its exact revision and Canvas
  transaction lease, avoiding the former redundant preflight refresh.

The phase contract was not relaxed. `resourcesReady`, `committed` and
`styledReady` still require the exact project root, runtime session, workspace
revision, preview revision and Canvas transaction, in canonical order. A
single failure receipt remains the only alternative sequence.

One attempted optimization was explicitly rejected. Automatically preloading
every bounded local font reduced the measured `fontsReady` interval by only
11–20 ms, increased navigation by 25–33 ms and changed total
Prepared → Canonical from approximately 973 ms to 987–1,000 ms. The code and
its speculative preload test were removed before the final build.

On the large fixture, the initial dev candidate profile was approximately
3,830 ms. The optimized dev samples were 2,475–2,547 ms without speculative
font preload. On the final optimized release artifact, five new-profile
candidate publications were 453–613 ms. A representative 453 ms publication
contained:

| Candidate stage | Release time |
| --- | ---: |
| Source generation publication | 124 ms |
| ProjectModel build | 81 ms |
| Zola render | 203 ms |
| Content preparation | 48 ms |
| CanvasGraph | 20 ms |
| Resource manifest | 5 ms, overlapped |
| Canvas transaction | 20 ms |

The component values overlap and therefore must not be added to reconstruct
the wall-clock total.

Five isolated release profiles produced Session → Canonical values of
2,105 ms, 2,152 ms, 2,299 ms, 2,184 ms and 2,018 ms. Nearest-rank p95 is
therefore 2,299 ms and passes the 2,500 ms gate. Prepared → Canonical remained
browser-dominated at 828–1,105 ms. Its instrumentation showed:

- navigation/HTML parsing/stylesheet work before bridge boot: 508–869 ms;
- font settlement: 130–319 ms;
- structural commit: 8–10 ms;
- resource settlement: 0 ms;
- font settlement plus the two styled frames: 186–378 ms.

The tail profile does not justify a cross-process generation cache: release
publication is already below 613 ms, while the remaining tail is browser
navigation and styled readiness. Reusing a previous process's derived
generation would add a verification and invalidation trust boundary without
addressing the dominant interval.

#### Final acceptance measurements

All measurements below use `studio.pana.tm.ro`. Release PSS samples include
the Tauri main process plus the WebKit network and web processes, and exclude
the small AppImage extraction parent.

| Gate | Final observation | Result |
| --- | ---: | --- |
| New-profile Session → Canonical p95 | 2,299 ms | pass |
| Editor return after warm-up, 50 cycles | p95 38.038 ms, max 39.013 ms | pass |
| Warm PSS change after 50 Editor/Teme cycles and 30 s idle | 646,173 → 651,279 KiB, +5.0 MiB | pass |
| Preview publication/durability | 1 logical publication, 2 durability operations | pass |
| Release idle main + WebKit, 30 s | 0.00% combined | pass |
| X11 pointer sweep, approximately 3,800 positions/30 s | 13.00% combined in dev, no stale sequence in regressions | harness-limited on release |
| Aggregate clean-project release PSS | 629,815 KiB, approximately 615 MiB | **fail** |

The absolute PSS target of 450 MB is not met. After both Editor and Teme were
warm, the aggregate was approximately 631–636 MiB. A final breakdown was
272,253 KiB main, 26,458 KiB WebKit network and 352,407 KiB WebKit web. This is
not an activity-switch leak: the warm 50-cycle delta is only 5.0 MiB. It is a
separate baseline footprint problem, concentrated in the retained Rust
application and WebKit document heaps, and must remain visible as future work.

The release AppImage activates native GTK Wayland and exposes no X11 window,
so the XTest absolute-coordinate sweep cannot be calibrated or attributed on
that artifact. The 13.00% measurement was made in the forced-X11 dev runtime
on the same final code. Rust and frontend regressions verify monotonically
accepted latest-wins hover sequences and reject stale continuations. This is a
harness limitation rather than a claimed release CPU measurement.

Validation:

- `npm run check`: 0 errors and 0 warnings;
- `npm run test:kernel`: 62/62 test files passed;
- Rust library suite: 1,231 passed, 0 failed, 2 ignored;
- `cargo fmt --check`, `cargo check --lib`, `git diff --check`: passed;
- production frontend build: passed;
- third-party license inventory: 906 packages, 425 unique texts, passed;
- the real Firefox Canvas harness was attempted twice but could not create a
  WebDriver session because the local Firefox package did not publish its
  Marionette port (`Failed to read marionette port`). No Canvas assertion ran.

Exactly one final Tauri release bundle was built:

`src-tauri/target/release/bundle/appimage/Pană Studio_0.1.2_amd64.AppImage`

- size: 109,124,088 bytes;
- SHA-256:
  `e4fe2506fe5be94f69890401f55bb95ba25f78f900aab922870b69569cc08d5c`.
