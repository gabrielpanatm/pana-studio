# Inventar model legacy ProjectModel

Data baseline inițială: 2026-08-11. Închidere identitate structurală:
2026-08-12.

## Arhitectura canonică

`filesystem/input extern -> ProjectWorkspace -> WorkspaceProjectionSnapshot -> ProjectModel`

Doar frontiera de deschidere/reconciliere/salvare poate inspecta discul. Toate
operațiile semantice și testele lor trebuie să consume o proiecție imutabilă.

Baseline verificat: `cargo check --all-targets --locked` trece fără warning-uri.

## Model vechi eliminat

| Element eliminat | Baseline | Înlocuitor canonic | Rezultat |
| --- | --- | --- | --- |
| `project_model::build_project_model` | 132 apariții cu definiția în 22 de fișiere | `ProjectModelTestFixture -> WorkspaceProjectionSnapshot -> build_project_model_from_workspace_projection` | 0 definiții și 0 apeluri legacy |
| `build_project_model_with_projection` | constructor mixt disk + drafturi/deleții | proiecție complet materializată | eliminat |
| `collect_project_model_files` și scannerul recursiv asociat | colector disk folosit de constructorul vechi | `collect_project_model_files_from_workspace_sources` | eliminate |
| `source_graph::build_source_graph`, `build_source_graph_with_drafts`, `build_source_graph_with_projection` | scanare disk sau proiecție parțială | `build_source_graph_from_workspace_projection` | eliminate; scannerul intern cere obligatoriu snapshot complet |
| `tera_insert_engine::plan_tera_insert` | wrapper de test cu document implicit | `plan_tera_insert_for_active_document` | eliminat |
| `plan_template_reference_workspace_mutation` | planner care își reconstruia graful de pe disk | `plan_template_reference_workspace_mutation_from_graph` | eliminat |
| `read_project_model_with_drafts` | comandă/API frontend care accepta drafturi, dar le ignora | `read_project_model` din contextul `ProjectWorkspace` | comandă, registru, wrapper TS și permisiune eliminate |
| fallback-urile disk din Content Models, Listing Items și Dynamic Widgets | reciteau surse lipsă din snapshot | hărțile complete `source_texts` + `deleted_sources` | eliminate, inclusiv din rebuild incremental |
| identitatea frontend `generated-identity`, `htmlTargetFromPageSection` și `selectTeraLayerSource` | clasă/selector/locație reconstruite în TypeScript | `SourceNodeId` opac alocat și reconciliat exclusiv în Rust | fișierul și simbolurile eliminate; test arhitectural anti-regresie |
| aliasuri, fingerprint și fallback după selector/linie/range | puteau reselecta un frate similar după mutație | `SelectionAnchor` + `SourceChangeSet` + CAS pe revizii exacte | zero apelanți și zero fallback-uri în sursele de producție |

## Grupuri de consumatori `build_project_model`

- Motoare ProjectModel: attribute, delete, duplicate, insert, move, tag, text,
  zola-image, template-workbench și motoarele Tera.
- Kernel: canvas interaction, editor navigation, preview projection și
  incremental rebuild.
- Alte suprafețe de test: blocks/slots, preview engine, CSS și project commands.

## Frontiere I/O legitime

| Frontieră | Verdict | Condiție de păstrare |
| --- | --- | --- |
| manifestul și scanarea inițială a proiectului | Păstrează | rezultatul acceptat inițializează `ProjectWorkspace`; consumatorii semantici nu recitesc discul |
| reconciliere externă și Save | Păstrează | verificare fail-closed față de manifestul acceptat |
| teste pentru symlink, path traversal, root ilizibil și limite de scanare | Păstrează/mută | testează modulul de frontieră, nu un constructor alternativ `ProjectModel` |
| materializarea asset-urilor binare | Păstrează | identitate prin manifest/proiecție; fără fallback semantic la disk |

## Marcaje „legacy” care nu reprezintă modelul vechi

Migrarea configurațiilor utilizatorului (Page JS, motion, teme, dynamic widgets,
block markers, deploy credentials), citirea protocoalelor WAL istorice și
diagnosticele care refuză formate vechi sunt contracte de compatibilitate sau
recovery. Nu sunt căi alternative de construire a ProjectModel și nu se șterg
în acest goal decât dacă auditul dependențelor demonstrează contrariul.

## Ordinea migrării executate

1. Fixture-ul comun și motoarele text/tag/delete.
2. Restul motoarelor HTML și Tera.
3. Preview, blocks, commands și kernel.
4. Testele SourceGraph de frontieră și planner-ul disk-backed.
5. Incremental/parity, ștergerea API-urilor vechi și căutarea finală repo-wide.

## Progres implementat

- Fixture comun pur din punct de vedere semantic: surse text, drafturi,
  deleții, revizie/tranzacție și resurse binare sunt materializate în
  `WorkspaceProjectionSnapshot`; discul este folosit numai pentru identitatea
  canonică a root-ului temporar.
- Migrate și verificate: toate motoarele ProjectModel HTML/Tera, Template
  Workbench, Preview, Canvas Interaction, Editor Navigation, Blocks, comenzile
  CSS/Project și rebuild-ul incremental.
- `ProjectModel` păstrează namespace-ul exact al proiecției. Validarea Zola
  Image nu mai consultă metadata/path-uri de pe discul live și verifică
  existența și ambiguitatea exclusiv față de această autoritate imutabilă.
- `SourceGraph` nu mai are mod intern disk/proiecție. Citirea sursei este strict
  din snapshot; output-ul Zola generat este exclus explicit din modelul
  editabil.
- Testele care au nevoie de un proiect Zola materializat capturează discul o
  singură dată prin `from_integration_disk_boundary`, apoi folosesc exact
  builderul de producție. Numele frontierei este explicit și nu exportă un al
  doilea constructor semantic.

## Închiderea identității structurale Rust-first

- `SourceNodeId` este opac, stabil pentru nodurile păstrate și indexat printr-o
  structură derivată, neserializată, `SourceNodeId -> node`. Inserarea și
  duplicarea alocă ID-uri noi; ștergerea le retrage; mutarea și editarea păstrează
  ID-urile exacte.
- `SelectionAnchor` este unica autoritate de selecție. Workspace-ul și selecția
  sunt validate pe revizii exacte, iar publicarea folosește CAS fail-closed;
  un ID lipsă sau o reconciliere ambiguă invalidează operația, fără retargetare.
- `SourceChangeSet` leagă `base_revision` de `result_revision`, editările text
  exacte și lifecycle-ul structural. Inserările, mutările, duplicările,
  ștergerile și restaurările HTML/Tera folosesc tranziții de arbore explicite,
  inclusiv păduri Tera cu mai multe rădăcini contigue.
- Code, preview, CanvasPatch și undo/redo consumă același model `after` validat.
  Recovery schema este v6 și păstrează identitatea exactă părinte + ordine de
  frați; formatele incompatibile sunt refuzate controlat.
- Coordonatele de sursă și selectorii CSS rămași sunt numai date de proiecție,
  navigare sau autoritate CSS în aceeași revizie; nu participă la identitatea
  structurală între revizii.

## Cutover multi-select structural

- `SelectionCoordinator` schema v2 deține setul ordonat (maximum 256), primary,
  range origin, revizia, agregarea HTML și intersecția capabilităților. Nu există
  DTO singleton sau coordonator paralel în frontend.
- Canvas și arborele trimit numai intenții opace replace/toggle/range/primary;
  ordinea structurală, inclusiv pentru noduri colapsate, este calculată în Rust.
- Mutațiile batch folosesc o singură amprentă ordonată de set, un plan detached,
  un CAS, un `SourceChangeSet` per fișier, un after-model și o intrare atomică de
  history/recovery v6. Instanțele repetate ale aceluiași SourceNodeId și seturile
  cu părinte+descendent sunt refuzate fail-closed.
- Inspectorul citește DOM numai pentru primary; common/mixed și motivele de
  dezactivare provin din Rust. Code, status și AI publică primary plus lista
  opacă bounded, iar endpointul AI o compară exact cu setul curent din Rust.
- Parserul HTML frontend `SourceNodeRange` și retargetarea nefolosită după
  `domPath`/selector au fost eliminate. `domPath` rămas este doar observație
  fizică efemeră și guard în aceeași revizie, nu identitate structurală.

## Dovezi de închidere

- Căutările repo-wide pentru constructorii/plannerele/comanda legacy au zero
  definiții și zero apeluri (în afara acestui document istoric).
- `cargo check --all-targets --locked` trece fără warning-uri.
- `cargo test --locked`: 1540 trecute, 0 eșecuri, 8 teste de infrastructură sau
  performance ignorate explicit în profilul debug.
- `cargo clippy --all-targets --locked -- -D warnings` și `cargo fmt --check`
  trec.
- `npm run check`: 0 erori și 0 warning-uri Svelte; `npm run test:kernel`: 80
  teste trecute; `npm run build`, bundle check, i18n/icons și licenses check
  trec.
- Benchmark release warm final: update multi-select p95 0,195/0,295/0,975/1,329
  ms la 1/10/100/256 membri pe 1k noduri și 1,320/1,364/2,346/3,289 ms pe 10k;
  batch CanvasPatch 256 p95 0,757 ms; reconcile-to-patch p95 4,881 ms la 1k și
  42,202 ms la 10k. Pe fixture-ul Zola real, rebuild complet p95 2 ms și
  incremental p95 1 ms, deci fără regresie peste 10%.
- Browser real: 100 overlay-uri p95 9 ms și CanvasPatch p95 2 ms; rollback-ul
  batch, history forward/inverse/redo și identitatea documentului sunt verzi.
- Auditul repo-wide și contractul automat confirmă absența simbolurilor
  `htmlTargetFromPageSection`, `selectTeraLayerSource`,
  `EditorLayerContextMenuRequest`, `offset_for_source_location`, a aliasurilor
  și a fallback-urilor selector/location/fingerprint din producție.
