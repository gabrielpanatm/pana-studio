# Changelog

Toate modificările importante ale Pană Studio vor fi documentate aici.
Proiectul folosește [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.8] - 2026-08-23

### Added

- benchmark reproductibil Rust-first cu suite smoke, standard și soak, protocol
  end-to-end, bugete aspiraționale, comparații de regresie și rapoarte persistente;
- proiectul canonic INDEX ZERO, cu profile control, mare, densitate DOM, limita
  funcțională de 991 de fișiere și refuzul fail-closed la 1.001 de fișiere;
- gărzi automate pentru cicluri, reachability, proprietatea contractelor,
  comenzile Tauri, modularitatea HTML și observabilitatea performanței;
- măsurători distincte pentru selecția tabului, settlement-ul documentului,
  schimbarea activităților și panourilor, Canvas, memorie și frame pacing.

### Changed

- limita proiectului este aliniată la 1.000 de fișiere rezidente, iar File
  Explorer virtualizează arborii mari fără a trunchia proiectul la 500 de intrări;
- activarea documentelor este latest-wins, proiectează tabul optimist și
  reutilizează exact contextul canonic Template Workbench confirmat de Rust;
- persistența proiecției Workbench este write-behind, iar flush-ul editărilor
  sare cozile curate fără a elimina checkpoint-urile autoritative;
- compoziția frontend, contractele de domeniu și comenzile Rust sunt împărțite
  în module cu proprietari expliciți, fără gateway-uri sau fallback-uri legacy;
- temele incluse au devenit proiecte starter complete și centralizate, cu un
  contract unic pentru materializare și resurse.

### Fixed

- proiectele cu 991 de fișiere ajung la workspace și Canvas, în timp ce limita
  de 1.001 rămâne refuzată integral și determinist;
- verdictul Rust pentru drag-and-drop structural este proiectat în Canvas, iar
  scenariile de editare, Undo și Redo nu mai sunt blocate de prima operație;
- schimbarea documentelor actualizează tabul în câteva milisecunde și evită
  refresh-uri Explorer, layout-uri sau persistări redundante;
- `tauri dev` nu mai epuizează limita Linux de file watchers când rezultatele
  benchmarkului și cache-urile WebKit/Mesa sunt păstrate în repository.

## [0.1.7] - 2026-08-14

### Added

- bibliotecă offline cu 36 de familii de fonturi variabile WOFF2, subseturi
  Latin și Latin Extended, previzualizare locală și instalare Rust-first în proiect;
- panou „Stocare” pentru inventarierea și curățarea controlată a cache-ului,
  jurnalelor și sesiunilor aplicației;
- setări și destinații de publicare portabile în `.panastudio`, cu credențiale
  păstrate separat în fișierul `.env` al proiectului;
- status bar pentru editorul de cod, folding HTML/SCSS/JS și evidențierea
  delimitatorilor structurali ai selecției;
- ștergere autorizată Rust și previzualizare stabilă pentru resursele media locale.

### Changed

- blocurile native livrează exclusiv CSS-ul și JavaScript-ul funcțional cerut
  de instanțele inserate, fără stiluri vizuale implicite sau runtime-uri nefolosite;
- configurația Motion este stocată în `.panastudio`, iar proiectul publicat
  primește numai Anime.js și codul minim compilat pentru interacțiunile paginii;
- runtime-ul și sursele interne Motion sunt incluse în aplicație, nu copiate ca
  infrastructură editabilă în fiecare proiect;
- documentele HTML complete sunt detectate structural în Template Workbench,
  fără excepții hardcodate pentru `base.html`;
- fișierele generate ale proiectului sunt editabile în modul Cod, în timp ce
  aplicația păstrează contractele de regenerare și reconciliere autoritative.

### Fixed

- selectoarele SCSS pot fi selectate imediat după pornirea aplicației, iar
  panoul CSS își schimbă valorile fără demontări și flash-uri duble;
- schimbarea rapidă a documentului nu mai transformă o selecție CSS validă
  într-o eroare stale și nu mai pierde legătura cu Inspectorul;
- selecția în cod evidențiază doar tagurile HTML sau selectorul și acoladele,
  fără a acoperi întregul conținut al elementului ori regulii;
- preload-ul fonturilor este scris în poziția corectă din `head`, iar
  documentele complete rămân previzualizabile independent;
- thumbnail-urile și previzualizările media importate folosesc URL-ul corect al
  sesiunii, fără resurse rupte după import;
- fișierele Motion și Anime.js proiectate în explorator se deschid și se editează
  prin aceeași cale autoritativă ca restul surselor proiectului.

## [0.1.6] - 2026-08-12

### Added

- audit de proiect Rust-first cu provideri expliciți, dovezi, filtre și remedieri
  autorizate, legate de identitatea exactă a sesiunii și reviziei;
- flux complet de preflight, build și publicare, plus deploy tipizat către Bunny,
  Cloudflare Pages, S3/R2, SFTP și FTP/FTPS, cu credențiale protejate, planuri
  stale-safe și receipt-uri parțiale pentru operațiile remote;
- blocuri native Rust pentru iconuri și slider, catalog Tabler inclus local și
  contracte tipizate pentru sloturi, limite și proprietăți;
- selecție multiplă și operații batch validate de nucleul Rust, cu identitate
  primară stabilă și reconciliere coordonată în Canvas și Inspector;
- `FontFaceGraph`, model canonic pentru familii CSS, declarații `@font-face`,
  fișiere locale sau de temă, metadate OpenType, roluri și livrare în browser.

### Changed

- editarea structurală HTML/Tera, navigarea și selecția folosesc sursa și
  ancorele autoritative Rust; parserul și identitățile HTML legacy din frontend
  au fost eliminate;
- inserarea, mutarea, duplicarea și ștergerea păstrează nesting-ul, indentarea,
  identitatea sursei și istoricul într-o singură cale ProjectWorkspace;
- auditul, publicarea, deploy-ul, blocurile și Font Manager-ul folosesc contracte
  Rust-first, cu proiecții frontend fără resolvere sau modele paralele;
- temele incluse au declarații și binare Inter/Poppins aliniate cu greutățile,
  subseturile Latin/Latin-ext, licențele și caracterele românești;
- CI și release folosesc toolchain-ul Rust 1.96.1 fixat, `clippy -D warnings`,
  verificarea catalogului de iconuri și bugetul bundle-ului frontend.

### Fixed

- generarea unei clase unice nu mai aplică modificarea altui element după
  schimbarea selecției sau reconcilierea Canvas-ului;
- mutările și inserările repetate nu mai degradează indentarea sau ierarhia
  HTML, iar ștergerea ultimului container structural este planificată corect;
- familiile CSS alias precum `Primary` și `Display` sunt legate de fonturile
  OpenType reale fără diagnosticul fals „neînregistrat” sau `font_face_missing`;
- fonturile system/external/missing, overlay-ul local peste temă, preload-ul și
  eliminarea controlată sunt clasificate și mutate prin aceeași identitate;
- auditul nu mai raportează utilizări necunoscute pentru resurse demonstrate și
  poate remedia determinist drift-ul structural fără a altera zonele vecine.

## [0.1.5] - 2026-08-04

### Added

- modele de conținut Rust-first păstrate în `.panastudio`, cu câmpuri
  personalizate reutilizabile, validare, atașare la secțiuni Zola și formulare
  de completare integrate în editorul paginii;
- widgeturi dinamice Tera pentru câmpuri și listing-uri, cu surse configurabile,
  prezentări tipizate, Listing Item reutilizabil și proprietăți editabile direct
  în Inspector;
- catalog de inserare proiectat autoritativ de Rust pentru HTML, blocuri native,
  componente, Tera și widgeturi dinamice, inclusiv inventarul HTML semantic
  complet pentru media, formulare, tabele și conținut încorporat;
- suprafețe vizuale editabile pentru documente și template-uri goale, cu zone de
  drop stabile și aceeași regulă de autoritate ca documentele deja populate.

### Changed

- documentul activ este întotdeauna suprafața editabilă direct în Canvas;
  limitele Tera moștenite sau incluse rămân externe și se deschid explicit;
- mutațiile structurale folosesc o anvelopă comună pentru identificare,
  poziționare, nesting, indentare și păstrarea markerilor multi-linie ai
  widgeturilor dinamice;
- crearea arhivelor și a conținutului de secțiune pregătește automat structura
  Zola necesară, iar arhivele noi folosesc paginare implicită;
- regulile SCSS ale template-urilor reutilizabile primesc ținte și consumatori
  expliciți, astfel încât stilurile Listing Item să fie salvate și proiectate;
- panoul „Adaugă element” și stările goale ale Inspectorului folosesc acum
  layout-ul, iconografia Tabler și limbajul vizual comun al aplicației.

### Fixed

- inserarea blocurilor din catalog păstrează fragmentul complet, identitatea
  Rust și panoul de proprietăți, în locul unor elemente HTML goale sau parțiale;
- selecția, ștergerea și mutarea elementelor nu mai eșuează când instanța randată
  provine dintr-un template dinamic sau dintr-un document proaspăt gol;
- gate-ul Tera nu mai blochează al doilea element adăugat în documentul activ și
  nu mai alternează vizual între limite concurente la selectare;
- template-urile Listing Item afișează o singură instanță reprezentativă în
  editor, fără a confunda sursa reală cu repetarea din arhivă;
- navigarea contextuală din Straturi, inserarea câmpurilor dinamice și
  actualizarea front matter-ului păstrează revizia și identitatea workspace-ului.

## [0.1.4] - 2026-08-02

### Added

- ciclu de inițializare Rust-first pentru proiect, cu faze explicite până la
  montarea Workbench-ului și deschiderea curată a paginii `index`;
- recuperare WAL acționabilă direct din ecranul de pornire, cu diagnostic,
  recitire și redeschiderea proiectului după reconciliere;
- reconstrucție incrementală ProjectModel și SourceGraph, cu invalidare
  tranzitivă sigură, raportare și fallback complet determinist;
- editor de conținut Markdown dedicat activității Conținut, cu Tiptap și
  setările paginii afișate într-un panou lateral;
- reprezentare semantică distinctă pentru sursele Markdown proiectate prin
  Tera, inclusiv blocuri Markdown evidențiate separat în Straturi;
- editor complet pentru fundaluri CSS cu straturi multiple, imagini și
  gradienturi, plus o piesă dedicată pentru configurarea gradienturilor;
- Grid Builder vizual în Inspector pentru compunerea valorilor CSS Grid fără
  editare directă în Canvas;
- fonturi locale și licențele lor în tema Pană Studio inclusă în aplicație.

### Changed

- mutațiile Preview, inclusiv Undo/Redo, folosesc promovarea incrementală a
  stilurilor și păstrează documentul montat când nu este necesar un rebuild;
- drag-and-drop-ul afișează numai indicatorul în timpul tragerii și aplică
  mutația structurală o singură dată, imediat după drop;
- editarea vizuală a blocurilor native reconciliază HTML, SCSS și Page JS
  într-o singură tranzacție ProjectWorkspace;
- fișierele Markdown deschise din Editor sunt tratate ca sursă Cod, iar
  editarea vizuală Markdown este deținută exclusiv de activitatea Conținut;
- exploratorul de fișiere serializează selecțiile rapide și evită reconstruirea
  inutilă a arborilor mari la hover sau expandare;
- autoritatea de scriere, istoricul și reconcilierea Preview folosesc
  identitatea exactă a tranzacției, fără mecanismul frontend paralel de lease.

### Fixed

- prima mutare vizuală, Undo și Redo nu mai așteaptă reconstruiri complete și
  nu mai produc refresh-uri care reîncarcă fonturile;
- promovarea CSS în Preview nu mai expune pentru un cadru HTML nestilizat și
  nu mai produce layout shift după mutații structurale;
- fișierele SCSS mari se deschid corect, iar încărcarea sursei nu mai lasă
  sentinelul intern în editor;
- navigarea nu mai păstrează rute sau identități stale după schimbarea
  documentului și nu mai mută selecția la generarea unei clase unice;
- ștergerea straturilor de gradient nu mai blochează interacțiunile UI, iar
  listele CSS de fundal nu mai emit valori invalide precum `, center`;
- erorile de compilare Zola deschid sursa diagnostică în Cod și permit
  repararea proiectului fără abandonarea dosarului curent;
- proiectele noi nu mai pornesc cu eroarea falsă de reconciliere Canvas;
- inserarea unui bloc nativ nu mai publică identități ProjectModel
  incompatibile și păstrează panoul de proprietăți disponibil;
- Inspectorul corelează acum instanța blocului selectat cu `BlockGraph` folosind
  câmpul `renderInstanceId` transmis efectiv de Rust și afișează controalele
  blocului imediat după inserare;
- operațiile de fundal, gradient și grid păstrează controalele editabile după
  golirea unei valori și refuză stările CSS structural invalide.

## [0.1.3] - 2026-07-30

### Added

- monitorizare event-driven a modificărilor proiectului pe Linux, legată de
  sesiunea Rust și fără scanări periodice costisitoare în starea stabilă;
- cache-uri bounded pentru ProjectModel, Workbench Preview și navigarea
  editorului, cu invalidare pe identitatea exactă a reviziei;
- teste de regresie pentru schimbarea documentelor, stabilitatea Inspectorului,
  exploratorul de fișiere și durata de viață a suprafeței Editor.

### Changed

- EditorShell, Canvas-ul, panourile laterale și Inspectorul rămân montate pe
  durata sesiunii proiectului, iar activitățile auxiliare sunt încărcate lazy;
- validarea folosită la deschiderea și editarea proiectelor este locală și
  offline; verificarea linkurilor externe rămâne disponibilă în validarea
  canonică explicită;
- proiecțiile Preview, navigarea Canvas și actualizările selecției refolosesc
  snapshot-uri Rust autoritative în locul reconstruirilor redundante.

### Fixed

- proiectele Zola mari nu mai declanșează verificări de rețea la deschidere sau
  după mutațiile interactive și nu mai sunt declarate invalide când un serviciu
  extern este indisponibil;
- schimbarea taburilor și a modurilor Vizual/Cod păstrează documentul activ și
  nu mai revine implicit la `index.html`;
- starea sesiunii, dosarele expandate și selecția exploratorului de fișiere nu
  mai intră în conflict la restaurarea proiectului;
- Canvas-ul refuză controlat snapshot-urile unui alt document fără a pierde
  documentul activ;
- editările HTML/CSS, mutările și Undo/Redo păstrează revizia selecției și nu
  mai reconstruiesc inutil conținutul Inspectorului;
- inițializarea proiectelor complexe și recuperarea WAL nu mai montează
  editorul înaintea deciziei de recuperare.

## [0.1.2] - 2026-07-29

### Added

- localizare completă română/engleză și preferințe de sistem pentru limbă,
  temă, accent și scalarea interfeței;
- navigare Rust-first pentru Canvas, selecție coordonată, explorator de
  fișiere, taxonomii, fonturi locale și tokeni de design;
- flux de pornire și creare a proiectelor validat de nucleul Rust, plus stare
  globală unificată pentru notificări și operații;
- audit de performanță reproductibil pe un proiect Zola real cu 35 de rute.

### Changed

- Canvas-ul și inspectorul folosesc identități și receipt-uri Rust autoritative
  pentru hover, selecție, mutare și editare;
- modelul de animații, gestionarea fonturilor, șabloanelor, taxonomiilor și
  exploratorului de fișiere au fost consolidate în contracte Rust-first;
- construcția SourceGraph, procesarea rutelor Preview și legarea identității
  Canvas rulează indexat și, unde operațiile sunt independente, paralel;
- suprafețele Preview Motion și Interactive sunt generate la cerere, nu la
  fiecare deschidere a proiectului.

### Fixed

- timpul până la Canvas verificat pentru proiectul de audit a scăzut de la
  aproximativ 14,7 secunde la 6,1–6,8 secunde în build-ul de dezvoltare;
- evenimentele redundante de hover Canvas–Rust au fost reduse cu 98,7% atunci
  când cursorul rămâne pe același element semantic;
- adnotarea șabloanelor mari nu mai rescanează sursa pentru fiecare nod și nu
  mai mută repetat sufixul unui șir în creștere;
- monitorizarea modificărilor externe rulează scanarea discului în worker și
  nu mai invalidează întreaga stare reactivă la fiecare heartbeat;
- materializarea Preview nu mai recreează tranzacții pentru directoare deja
  validate.

## [0.1.1] - 2026-07-24

### Added

- un Workbench Rust-first persistent pentru activități, documente, grupuri,
  split-uri, viewport și panoul inferior, cu identitate de sesiune, revizii
  monotone și receipt-uri tipizate;
- Activity Rail și Command Center (`Ctrl+K`) cu căutare Rust pentru comenzi,
  activități, fișiere și simboluri Tera;
- workspace-uri dedicate pentru Șabloane, Componente, Blocuri, Teme, Date,
  Sistem de design, Resurse, Conținut, Probleme și audit, Control versiuni și
  Publicare;
- canvas responsive cu mod Fit/fix, lățime exactă, zoom, riglă, redimensionare
  liberă și breakpoint-uri SCSS;
- audit unificat, inventar și redenumire sigură a claselor, plus operații de
  publicare anulabile și legate de sesiunea proiectului;
- catalog Rust-first pentru temele Zola, cu planificare, validare, instalare,
  activare, override local și o singură intrare Undo;
- temele bundled `Nord`, `Cadru` și `Rădăcini`, fiecare cu rețetă de conținut,
  date TOML, active locale, preview WebP și design responsive;
- catalog semantic pentru șabloane Tera, relații, consumatori și operații de
  creare, duplicare, redenumire, override și ștergere;
- model separat pentru componente Tera și blocuri native configurabile, cu
  proprietăți tipizate de nucleul Rust;
- SourceGraph extins pentru Tera/Zola, shortcodes, front matter și date TOML,
  JSON, YAML, CSV, BibTeX și XML;
- editor vizual pentru datele proiectului și stilurile tematice ale titlurilor,
  textelor, imaginilor, legăturilor, listelor și citatelor;
- setări dedicate exclusiv aplicației și color picker propriu bazat pe
  `colorjs.io`;
- teste de contract pentru shell, terminologie, densitate, Command Center,
  teme, șabloane, date, blocuri, setări și integrarea Git în Workbench;
- contract de optimizare Zola pentru elementele `<img>`, configurabil direct
  din inspector și păstrat de operațiile structurale de mutare, duplicare și
  ștergere.

### Changed

- interfața principală a fost reconstruită în jurul activității utilizatorului,
  cu topbar redus, suprafețe contextuale și navigare comparabilă cu IDE-urile
  consacrate;
- controalele preview-ului au fost reunite într-o singură bară inferioară, iar
  zoom-ul, viewport-urile și lățimea fluidă au un singur punct de control;
- taburile de documente folosesc scroll orizontal lin, iar butoanele și
  iconurile au dimensiuni și familii vizuale coerente;
- starea restaurabilă a navigării este deținută de Rust, iar Svelte păstrează
  numai stare efemeră de interacțiune;
- preview-ul vizual și codul pot fi afișate simultan în grupuri sincronizate;
- mesajele de stare, notificările, autoritatea AI și diagnosticele folosesc
  canale explicite, fără bannere concurente sau indicator flotant;
- terminologia vizibilă a fost unificată în română, iar controalele folosesc un
  sistem comun de tokeni, focus vizibil, text de minimum 11 px și zone de
  interacțiune de minimum 32 px;
- versionarea Git este acum activitatea centrală „Control versiuni”, accesibilă
  din Activity Rail și Command Center, nu un drawer local;
- fluxul de Publicare reunește verificarea, build-ul, jurnalul, anularea și
  deploy-ul într-o singură operație Rust urmărită;
- dosarul selectat este acum chiar rădăcina proiectului Zola, fără structura
  intermediară `sursa`; output-ul implicit revine la `public`, iar build-ul și
  deploy-ul urmează exact `output_dir` configurat de utilizator, inclusiv o
  locație externă permisă;
- Preview, Source Browser, validarea și build-ul folosesc un singur motor Rust
  Zola 0.22.1 embedded, fixat la o revizie oficială și serializat printr-o
  autoritate comună; inițializarea aplică starterul direct prin
  `ProjectBootstrapLease`/`WriteAuthority`;
- modificările continue din color picker sunt previzualizate live, dar sunt
  grupate într-o singură mutație la confirmare, cu salvare și Undo/Redo
  autoritative.

### Fixed

- redeschiderea color picker-ului păstrează culoarea reală și nu mai produce
  flash-ul controlului nativ;
- schimbarea unei culori nu mai generează o revizie pentru fiecare mișcare;
- salvarea regulilor CSS folosește receipt-ul exact al ProjectWorkspace, iar
  Undo/Redo restaurează corect mutația;
- taburile numeroase nu mai produc scroll vertical și răspund lin la rotița
  mouse-ului pe axa orizontală;
- stilurile tuturor paginilor din temele bundled sunt încărcate consecvent, nu
  numai pe pagina principală.

### Removed

- funcționalitatea de planșă vizuală și integrările ei frontend, Rust, AI/MCP,
  Tauri și de inițializare a proiectelor; datele vechi rămân neatinse pe disc,
  dar nu mai sunt încărcate sau urmărite de aplicație;
- shell-ul CSS global legacy și arhitectura paralelă `SiteWorkspace`;
- panourile redundante Site, History și Settings, fila Pagină, panoul Variabile
  și grupurile vechi de comenzi din topbar;
- contractele legacy `page-components` și editorul separat de loop-uri, înlocuite
  de modelele distincte pentru componente și blocuri;
- căile frontend directe care duplicau operațiile semantice mutate în nucleul
  Rust;
- optimizarea bulk a imaginilor și rescrierea globală a output-ului;
- binarul Zola inclus, checksum-ul, sidecar-ul, fallback-urile CLI/PATH,
  expunerea căii executabilului și vechiul scaffold exterior proiectului Zola.

## [0.1.0] - 2026-07-19

### Added

- prima versiune publică de test pentru Linux x86-64;
- editor vizual și preview izolat pentru proiecte Zola;
- editare HTML/Tera, SCSS, Markdown și JavaScript;
- timeline de animații și gestionarea resurselor;
- versionare Git locală și operații remote explicite;
- integrare MCP/Codex și deploy opțional către Bunny;
- motor și binar Zola `0.22.1` incluse.

### Changed

- repository pregătit pentru publicare open-source;
- licența proiectului stabilită la `EUPL-1.2-or-later`;
- documentația publică, politica de securitate și atribuirea componentelor terțe
  completate.

[Unreleased]: https://github.com/gabrielpanatm/pana-studio/compare/v0.1.6...HEAD
[0.1.6]: https://github.com/gabrielpanatm/pana-studio/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/gabrielpanatm/pana-studio/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/gabrielpanatm/pana-studio/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/gabrielpanatm/pana-studio/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/gabrielpanatm/pana-studio/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/gabrielpanatm/pana-studio/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/gabrielpanatm/pana-studio/releases/tag/v0.1.0
