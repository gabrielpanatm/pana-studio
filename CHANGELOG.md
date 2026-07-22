# Changelog

Toate modificările importante ale Pană Studio vor fi documentate aici.
Proiectul folosește [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- un Workbench Rust-first persistent pentru activități, documente, grupuri,
  split-uri, viewport și panoul inferior, cu identitate de sesiune, revizii
  monotone și receipt-uri tipizate;
- Activity Rail și Command Center (`Ctrl+K`) cu căutare Rust pentru comenzi,
  activități, fișiere și simboluri Tera;
- workspace-uri dedicate pentru Site, Componente, Sistem de design, Resurse,
  Conținut, Probleme și audit, Control versiuni și Publicare;
- canvas responsive cu mod Fit/fix, lățime exactă, zoom, riglă, redimensionare
  liberă și breakpoint-uri SCSS;
- audit unificat, inventar și redenumire sigură a claselor, plus operații de
  publicare anulabile și legate de sesiunea proiectului;
- teste de contract pentru shell, terminologie, densitate, Command Center,
  prezentarea generală a site-ului și integrarea Git în Workbench.

### Changed

- interfața principală a fost reconstruită în jurul activității utilizatorului,
  cu topbar redus, suprafețe contextuale și navigare comparabilă cu IDE-urile
  consacrate;
- starea restaurabilă a navigării este deținută de Rust, iar Svelte păstrează
  numai stare efemeră de interacțiune;
- preview-ul vizual și codul pot fi afișate simultan în grupuri sincronizate;
- mesajele de stare, notificările, autoritatea AI și diagnosticele folosesc
  canale explicite, fără bannere concurente sau indicator flotant;
- terminologia vizibilă a fost unificată în română, iar controalele folosesc un
  sistem comun de tokeni, focus vizibil, text de minimum 11 px și zone de
  interacțiune de minimum 32 px;
- versionarea Git este acum activitatea centrală „Control versiuni”, accesibilă
  din Activity Rail, prezentarea Site și Command Center, nu un drawer local;
- fluxul de Publicare reunește verificarea, build-ul, jurnalul, anularea și
  deploy-ul într-o singură operație Rust urmărită.

### Removed

- funcționalitatea de planșă vizuală și integrările ei frontend, Rust, AI/MCP,
  Tauri și de inițializare a proiectelor; datele vechi rămân neatinse pe disc,
  dar nu mai sunt încărcate sau urmărite de aplicație;
- shell-ul CSS global legacy și arhitectura paralelă `SiteWorkspace`;
- grupurile redundante de comenzi din topbar și vechiul overlay Git;
- căile frontend directe care duplicau operațiile semantice mutate în nucleul
  Rust.

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

[Unreleased]: https://github.com/gabrielpanatm/pana-studio/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/gabrielpanatm/pana-studio/releases/tag/v0.1.0
