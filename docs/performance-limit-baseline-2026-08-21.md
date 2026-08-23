# Baseline limite și performanță — 2026-08-21

## Scop

Acest document fixează starea de lucru necomisă din 21 august 2026 înaintea
proiectului Zola de stres și a optimizărilor. Obiectivul este separarea strictă
între:

1. limite explicite impuse de contractele Rust;
2. degradare măsurată înainte de limita dură;
3. costul complet perceput de utilizator în Tauri/WebKit;
4. limitele fixture-ului și ale instrumentelor de măsurare.

Mașina de referință are un Intel Xeon E5-2620 v3, 6 nuclee/12 fire, 15 GiB
RAM și SSD SATA. Înaintea testelor erau disponibili aproximativ 11 GiB RAM,
iar swap-ul de 2 GiB era deja ocupat aproape integral. Rezultatele sunt baseline
locale, nu praguri hardware universale.

## Limite explicite relevante

| Zonă | Limită | Comportament observabil |
| --- | ---: | --- |
| Manifest disk canonic | 1.000 fișiere urmărite | manifestul devine trunchiat; deschiderea este refuzată fail-closed |
| Inspecție inițială disk | 2.000 intrări fișier/director | inspecția și manifestul sunt marcate trunchiat |
| ProjectScan / File Explorer | 500 intrări | snapshotul este trunchiat; fișierele de după prefix nu sunt publicate |
| FileBufferStore | 500 fișiere text | bootstrap-ul se oprește și publică diagnostic |
| FileBufferStore per fișier | 2 MiB | fișierul text este refuzat/omis |
| FileBufferStore total | 24 MiB | bootstrap-ul se oprește și publică diagnostic |
| Reconciliere disk | 1.000 căi per lot | loturile mai mari depășesc contractul |
| Proiecție Preview | 16.384 intrări / 512 MiB | candidatul Preview este refuzat |
| HTML Preview per document | 8 MiB | injectarea suprafeței Preview este refuzată |
| Canvas | 4.096 documente / 250.000 noduri | candidatul Canvas este refuzat |
| Resurse Canvas | 16.384 / 512 MiB | manifestul Canvas este refuzat |
| CanvasPatch | 2 MiB | patch-ul structural este refuzat |
| Resursă binară Workspace | 32 MiB per fișier / 64 MiB total | stage/save/restore este refuzat |
| Preview runtime frontend | 64 operații în așteptare | se aplică backpressure |
| Mesaje Preview inbound | 512/secundă | fereastra de mesaje este limitată |
| Selecție | 256 membri | selecția peste limită este refuzată |
| Artefact deploy | 50.000 fișiere, 50.000 directoare, 512 MiB total | publicarea este refuzată |
| Artefact deploy per fișier | 64 MiB | publicarea este refuzată |
| Arbore Git materializat | 5.000 fișiere / 256 MiB total | operația de versionare este limitată |

Sursele principale sunt `project/manifest.rs`, `project/scan.rs`,
`kernel/file_buffer_store/bootstrap.rs`, `preview/preprocess/workspace.rs`,
`preview/inject.rs`, `preview/canvas.rs`,
`kernel/preview_projection/model.rs`, `kernel/project_workspace/model.rs`,
`deploy/artifact.rs` și `versioning/repository.rs`.

### Limita practică este mai mică decât limita manifestului

Deschiderea acceptă numai un manifest complet de maximum 1.000 de fișiere, dar
`scan_project_disk_manifest` sortează și taie proiecția la 500 de intrări înainte
de `bootstrap_file_buffer_store`. `ProjectWorkspace` este apoi creat din acel
buffer și prima proiecție este capturată imediat. Prin urmare, un benchmark care
alimentează direct `ProjectModel` cu 831–991 de fișiere validează motorul de
model, nu utilizabilitatea integrală a aceluiași proiect în aplicație.

Aceasta trebuie tratată ca o limită funcțională și ca o lacună a harness-ului,
nu ca o simplă valoare de configurare.

## Baseline release pentru ProjectModel

Toate valorile sunt p95 în milisecunde. Rulările folosesc același binar release,
mostre warm și fixture-uri determinate. `external reconcile` procesează același
lot separat de 96 fișiere/138.614 B și de aceea nu scalează cu fixture-ul.

| Profil | Fișiere | Noduri în template | Open | HTML incremental | HTML complet | CSS | Clone model | Reconcile |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| control | 111 | 200 | 29,963 | 11,455 | 30,107 | 0,804 | 0,631 | 6,790 |
| extins implicit | 831 | 1.000 | 438,388 | 88,963 | 399,552 | 6,492 | 8,754 | 7,658 |
| margine fișiere | 991 | 1.000 | 559,735 | 76,261 | 504,833 | 9,948 | 7,123 | 9,366 |
| densitate noduri | 111 | 10.000 | 1.104,458 | 1.175,370 | 1.213,433 | 24,490 | 24,182 | 7,162 |

La 10.000 de noduri, parsarea și reconcilierea template-ului consumă
1.057,392 ms p95 din cele 1.175,370 ms ale editării incrementale. Chiar și CSS
depășește 24 ms deoarece calea clonează modelul mare. Densitatea unui singur
template este limita de latență dominantă înaintea limitei Canvas de 250.000 de
noduri.

Fixture-ul cu 1.011 fișiere confirmă plafonul dur: manifestul este trunchiat și
benchmark-ul de open este oprit imediat. Aceeași stare este refuzată de calea
reală de open prin `scan_project_disk_manifest`.

## Bugetele existente și rezultatul lor

Bugetele curente sunt: open 40 ms, HTML/model incremental 20 ms, oracle complet
50 ms, CSS 1,5 ms, clone 1,5 ms și external reconcile 10 ms.

- profilul control trece toate bugetele;
- profilul implicit extins eșuează open, HTML incremental, HTML complet, CSS,
  clone și model build;
- numai external reconcile rămâne în buget la profilul extins;
- runner-ul nu scrie raportul JSON cerut când bugetele sunt depășite, deoarece
  verifică și aruncă eroarea înaintea serializării;
- prima recompilare release a durat 5m19s, în timp ce testele au durat 17,1s;
  sweep-urile trebuie să refolosească binarul pentru a nu confunda build-ul cu
  runtime-ul.

Garda de arhitectură pentru observabilitatea performanței trece fără încălcări.

## Baseline frontend de producție

Buildul de producție trece. Graful inițial român conține 14 fișiere JavaScript,
1.228.122 B raw și 316.338 B gzip. Există 63 chunk-uri client, iar cel mai mare
are 490.956 B. Application shell are 156.048 B raw/38.848 B gzip și este în
graful inițial. Suprafețele mari ale workspace-ului sunt încărcate lazy.

Aceste valori trec bugetele existente, dar nu demonstrează timpul până la prima
interacțiune, costul montării workspace-ului sau retenția de memorie WebKit.

## Ce nu măsoară încă baseline-ul

`performance:baseline` nu execută calea completă `open_project_bootstrap` și nu
include materializarea Workspace, Zola embedded, Sass, rutele, CanvasGraph,
publicarea HTTP, navigarea WebKit, fonturile, imaginile, paint-ul sau montarea
Svelte. Fixture-ul are CSS minimal, un singur widget, fără conținut realist,
media, fonturi, taxonomii, paginare, căutare ori animații reale.

Lipsesc încă:

- p50/p95/p99 end-to-end pentru cold open, warm open și first usable Canvas;
- input-to-paint pentru selecție, inspector, drag/drop, text, CSS, Undo/Redo și
  schimbarea activităților;
- frame time, long tasks, IPC count/payload, I/O și fsync;
- RSS/PSS per proces, peak memory și retenția după 100–500 de operații;
- CPU idle și activ, cache hit/fallback și throughput pentru loturi;
- comportamentul vizual cu mii de reguli CSS, animații concurente și media.

## Contract pentru pasul 2

Proiectul de stres trebuie generat determinist, dar să fie și un site Zola real,
vizual coerent. Va avea profile scalabile, nu un singur număr magic:

1. **control** — proiect realist mic pentru comparații și regresii;
2. **mare utilizabil** — sub 500 de intrări publicate, dar cu rute, conținut,
   CSS, date, componente, fonturi, imagini și animații aproape de buget;
3. **densitate DOM** — puține fișiere și template-uri cu 1k/2k/5k/10k noduri;
4. **margine disk** — 991 fișiere pentru limita manifestului și 1.001+ pentru
   refuzul fail-closed;
5. **resurse** — volume progresive până la limitele Workspace/Preview/deploy.

Fiecare profil va avea manifest de așteptări, seed fix, dimensiuni verificate și
comenzi separate pentru generare, validare, build Zola și măsurare. Testele
reale se rulează în release și raportează separat cold/warm, fără a amesteca
compilarea aplicației în latențele runtime.
