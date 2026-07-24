# Changelog

Toate modificările importante ale Pană Studio vor fi documentate aici.
Proiectul folosește [Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/gabrielpanatm/pana-studio/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/gabrielpanatm/pana-studio/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/gabrielpanatm/pana-studio/releases/tag/v0.1.0
