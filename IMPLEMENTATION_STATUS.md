# Zola 0.23.4 upgrade — implementation status

Document principal: `docs/zola-0.23.4-upgrade-plan-2026-08-27.md`
Ultima actualizare: 28 august 2026

## Status general

| Etapă | Status |
| --- | --- |
| 0 — Baseline reproductibil înainte de upgrade | COMPLETE |
| 1 — Pin unic la Zola 0.23.4 și compilare minimă | COMPLETE |
| 2 — Înlocuirea semanticii Tera 1 și eliminarea legacy | COMPLETE |
| 3 — Motor embedded, producție și preview | COMPLETE |
| 4 — Contracte și experiența Components | COMPLETE |
| 5 — Funcționalități Zola 0.23 în editor | COMPLETE |
| 6 — Conversia conținutului bundled și metadata | COMPLETE |
| 7 — Validare finală și pregătirea release-ului | COMPLETE |

## Etapa 0 — Baseline reproductibil înainte de upgrade

Status: **COMPLETE**

### Rezumat audit inițial

- Runtime-ul embedded este Zola 0.22.1, fixat la revizia `29540e9897dbe8aca388b13f7bdf615985f6ca2c`.
- Proiectul are suite frontend/kernel, teste Rust, verificare explicită a runtime-ului embedded, runner de performanță și baseline-uri istorice care nu trebuie suprascrise.
- `src-tauri/src/deploy/zola.rs` testează deja build-ul minimal, Sass, resurse statice, image processing, output-uri relative/absolute, validarea și anularea înainte de build.
- `src-tauri/src/preview/engine.rs` conține teste de paritate disk/memory, refresh incremental și rute de taxonomie.
- Lipsesc un fixture compact dedicat upgrade-ului care să acopere într-un singur proiect pagină, secțiune, taxonomie, paginare, Sass, image processing, search, feed, i18n și asset colocat, precum și un test reutilizabil care să construiască toate cele cinci startere bundled.
- Anularea înainte de build este testată, dar nu există o injecție deterministă pentru anularea exact după randare și înainte de publicarea atomică.
- Baseline-ul standard existent este istoric și costisitor; etapa necesită un artefact nou, separat, cu rezultatele comenzilor de verificare, timpii și dimensiunile relevante.

### Plan scurt de implementare

1. Adăugarea fixture-ului compact Zola 0.22.1 și a testului embedded pentru toate caracteristicile cerute.
2. Adăugarea testului care construiește toate starterele din copii temporare.
3. Introducerea unui hook intern de test după randare și verificarea anulării/publicării atomice înainte și după build.
4. Rularea suitelor baseline și capturarea rezultatelor într-un document nou, fără modificarea baseline-urilor istorice.
5. Audit final față de Etapa 0 și actualizarea acestui checkpoint.

### Verificări efectuate

- `npm run check`: PASS, 0 erori și 0 avertismente.
- `npm run test:kernel`: PASS, 145/145.
- `npm run build`: PASS, inclusiv bundle guard.
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`: PASS final,
  1.663 teste trecute și 20 ignorate intenționat.
- `node --test tests/zola-embedded-runtime.test.mjs`: PASS, 1/1.
- `deploy::zola::tests`: PASS, 12/12, inclusiv toate cele cinci startere,
  matricea Zola și anularea/publicarea atomică.
- `upgrade_baseline_fixture_keeps_preview_and_disk_generation_in_parity`: PASS.
- `cargo build --manifest-path src-tauri/Cargo.toml --release --locked`: PASS;
  binar 132.319.504 bytes.
- Baseline release: toate cele 5 probe au rulat; 6 bugete aspiraționale
  preexistente sunt depășite și sunt documentate complet în baseline.
- `cargo fmt --check` și `git diff --check`: PASS.

### Rezumat implementare

- A fost adăugat fixture-ul persistent
  `tests/fixtures/projects/zola-upgrade-baseline/`, care acoperă pagina,
  secțiunea, taxonomia, paginarea, Sass, image processing, search, feed, i18n și
  asset colocat.
- Build-ul și validarea embedded rulează fixture-ul și toate starterele din
  copii temporare.
- Preview Memory și build-ul Disk sunt comparate document cu document și pentru
  resursele derivate; rezolvarea căii disk pentru rute Zola fără extensie a fost
  generalizată pentru rute multilingve și paginate.
- Build-ul are un hook intern de test după randare, folosit pentru a dovedi că
  anularea înainte de publicare păstrează artifactul anterior. Sunt testate și
  anularea înainte de build și eroarea de randare.
- Au fost create artefactele noi
  `docs/zola-0.22.1-upgrade-baseline-2026-08-28.md` și
  `docs/zola-0.22.1-upgrade-performance-baseline-2026-08-28.json`; documentele
  istorice nu au fost modificate.

### Decizii tehnice importante

- Fixture-urile și starterele sunt copiate în directoare temporare înainte de
  build; sursele canonice nu sunt mutate sau poluate cu output.
- Depășirile de performanță deja cunoscute sunt baseline explicit, nu sunt
  ascunse și nu extind scopul upgrade-ului într-un refactor de performanță.
- Artifactul publicat rămâne autoritativ până la succesul complet al generației
  private.

### Probleme rămase

- Nicio problemă funcțională a Etapei 0.
- Cele șase bugete de performanță preexistente rămân deschise în documentația
  de performanță și vor fi folosite doar pentru detectarea regresiilor la Etapa 7.

## Etapa 1 — Pin unic la Zola 0.23.4 și compilare minimă

Status: **COMPLETE**

### Rezumat audit inițial

- `zola-site`, `zola-config` și `zola-utils` sunt fixate la revizia Zola 0.22.1;
  Cargo rezolvă Tera 1.20.1 din cerința directă `1.17`.
- Feature-urile Tera curente sunt `preserve_order` și `date-locale`; Tera 2.2.0
  folosită de Zola 0.23.4 necesită `preserve_order` și `fast`, iar
  `date-locale` nu mai există.
- Versiunea/SHA-ul runtime-ului sunt centralizate în `zola_engine.rs`, dar
  Preview și About mai conțin texte literale 0.22.1. Catalogul de creare expune
  deja versiunea backend, în timp ce About nu o consumă.
- Graful actual are o singură Tera 1, însă conține duplicate `reqwest` și
  `sha2`; acestea trebuie reevaluate după pin, fără upgrade-uri directe oarbe.
- Tera 2 nu mai expune AST-ul public, iar Zola 0.23.4 schimbă API-urile `Site`;
  `cargo check` după pin va fi folosit ca inventar autoritativ al adaptărilor
  minime necesare compilării.

### Plan scurt de implementare

1. Pin unic la commit-ul complet 0.23.4, Tera exact 2.2.0 și identitate backend.
2. About va citi versiunea dintr-un contract backend, fără literal duplicat.
3. Regenerarea lockfile-ului și rularea `cargo check` pentru erorile reale.
4. Adaptarea exclusivă a API-urilor necesare compilării, fără shim Tera 1.
5. Audit `cargo tree -d`, metadata/surse, licențe și checkpoint final.

### Verificări efectuate

- `cargo check --manifest-path src-tauri/Cargo.toml --locked`: PASS.
- `cargo metadata --locked`: PASS; `site`, `config` și `utils` sunt toate
  versiunea 0.23.4 din revizia completă
  `28daab8d47cacb1e6c863b97739148f424433f5b`; singura versiune Tera este 2.2.0.
- `cargo tree --locked -d`: auditat. `reqwest` 0.12/0.13 și `sha2` 0.10/0.11
  rămân duplicate deliberate între funcționalitățile directe Pana/AWS și noul
  graph Zola; alinierea lor nu este necesară compilării și ar extinde scopul.
- `npm run check`: PASS, 0 erori și 0 avertismente.
- `node --test tests/zola-embedded-runtime.test.mjs`: PASS.
- `npm run licenses:generate && npm run licenses:check`: PASS, 991 pachete și
  476 texte de licență unice.
- `cargo fmt --check`: PASS.

### Rezumat implementare

- Cele trei crate-uri Zola sunt fixate la commit-ul complet al tagului 0.23.4,
  dependența directă este Tera exact 2.2.0 cu feature-urile upstream, iar
  lockfile-ul a fost regenerat minimal.
- Identitatea runtime-ului este 0.23.4/revizia completă în backend; snapshot-ul
  App Home schema 3 expune versiunea, iar About o consumă fără literal local.
- Adaptările API obligatorii compilării folosesc `Site::build()`, noul
  `Arc<Library>`, `RenderCache`, randarea publică Tera și API-ul public de
  componente. Scannerul nu mai importă `tera::Template` ori `tera::ast`.
- Inventarul de licențe Rust a fost regenerat pentru graph-ul nou.

### Decizii tehnice importante

- Build-ul rămâne într-o generație privată, cu checkpoint-uri de anulare înainte
  și după `Site::build()`; numai succesul complet permite publicarea atomică.
- Source Graph păstrează temporar doar carcasa IR-ului propriu după separarea de
  AST; proiecția structurală completă și eliminarea modelelor legacy aparțin
  explicit Etapei 2 și sunt prima lucrare următoare.

### Probleme rămase

- Nicio problemă care aparține Etapei 1.
- Modelele Macro/Shortcode și proiecția semantică structurală sunt intenționat
  încă deschise și aparțin criteriilor Etapei 2.

## Etapa 2 — Înlocuirea semanticii Tera 1 și eliminarea legacy

Status: **COMPLETE**

### Rezumat audit inițial

- Deși dependența pe AST-ul privat Tera a fost întreruptă pentru compilare,
  `tera_semantics.rs`, `component_graph.rs`, modelul Source Graph și contractele
  încă expun variante Macro/Shortcode.
- `zola_shortcode.rs` și `zola_shortcode.pest` sunt încă active, iar
  `pest`/`pest_derive` sunt dependențe directe exclusiv pentru această cale.
- Scannerul lossless recunoaște sintaxa Tera 1, dar nu încă definițiile și
  apelurile JSX-like de componente Tera 2, namespace-urile, argumentele, body-ul
  și range-urile cerute.
- Catalogul runtime păstrează semnături și filtre Tera 1 și trebuie reaudiat
  față de registrele Zola 0.23.4.

### Plan scurt de implementare

1. Inventarierea completă a modelelor, scannerelor, contractelor și fluxurilor
   incremental/reconciliation care depind de Macro/Shortcode.
2. Introducerea IR-ului structural Tera 2 cu definition/call și range-uri.
3. Înlocuirea grafului legacy cu modelul unic de componentă și diagnosticarea
   explicită a sintaxei vechi.
4. Eliminarea parserului shortcode și a dependențelor sale nefolosite.
5. Reauditarea catalogului runtime, fingerprint-urilor și testelor incrementale.

### Rezumat implementare

- Scannerul lossless deținut de Pana produce un IR neutru Tera 2, fără acces la
  AST-ul intern upstream. IR-ul acoperă definiții/apeluri namespaced, argumente
  tipizate, default/rest/body, map/spread/slice/optional/ternary/comprehension și
  set blocks, cu range-uri exacte UTF-8.
- Component Graph schema 4 folosește exclusiv definiții și apeluri de componente
  Tera 2, inclusiv argumente declarate/furnizate, relații consumer și diagnostice
  pentru definiții/argumente lipsă, necunoscute sau incompatibile.
- Au fost eliminate parserul și gramatica shortcode, dependențele Pest dedicate,
  modelele Macro/Shortcode, migrarea legacy și căile de inserare/reconciliere
  aferente. Sintaxa veche produce diagnostice localizate de incompatibilitate.
- Contractele Rust–TypeScript, paleta/inserarea Tera, navigarea, proveniența,
  adnotarea preview și catalogul de șabloane consumă noile kind-uri
  `componentDefinition`/`componentCall`.
- Catalogul runtime a fost reaudiat față de registrele Zola 0.23.4/Tera 2 și
  include semnăturile noi plus deprecarea `get_taxonomy_url(name=...)` către
  `term`.
- Rebuild-ul incremental invalidează consumatorii unei componente și reconstruiește
  indexul de noduri înaintea grafurilor derivate, obținând același rezultat ca
  scanarea completă.
- Validarea structurală detectează taguri/scope-uri neînchise sau nepotrivite și
  acceptă corect ramura `else` pentru `if` și `for`.

### Decizii tehnice importante

- CST-ul Pana este autoritatea pentru range-uri și editări locale; Tera/Zola
  rămâne autoritatea de compilare project-wide.
- Nu există runtime dual, convertor permanent sau variantă paralelă pentru
  macro/shortcode. Detectoarele legacy rămase există numai pentru diagnosticul
  explicit cerut de plan.
- Filtrul Zola `date` este înregistrat explicit numai în harness-ul Tera izolat
  al testelor; generatorul de producție păstrează sintaxa canonică Zola.

### Verificări efectuate

- `npm run check`: PASS, 0 erori și 0 avertismente.
- `npm run test:kernel`: PASS, 145/145, inclusiv arhitectură, contracte,
  TypeScript, i18n și toate testele Node.
- Testele Source Graph focalizate: PASS, 92 trecute și 2 ignorate intenționat.
- Testul componentelor cu range-uri exacte în template și Markdown: PASS.
- Testul incremental component contract vs full scan: PASS.
- Testele motorului Tera Insert portate la componente: PASS, 12/12.
- Testele focalizate pentru mutații de componente/date, dynamic widgets și
  proiecție structurală: PASS.
- `cargo check --locked`: PASS; `cargo fmt --check`: PASS.
- Auditul `rg` nu găsește variante de producție AST Tera 1, Macro/Shortcode,
  parser shortcode sau dependențe Pest dedicate.
- `cargo test --locked`: 1.655 trecute, 20 ignorate și un singur eșec rămas,
  testul de politică draft al preview-ului; cheia rutei Zola 0.23.4 este
  `despre`, nu `despre/`. Acest caz aparține explicit Etapei 3 și este primul
  punct al auditului următor.

### Probleme rămase

- Nicio problemă cunoscută care aparține Etapei 2.
- Textele UI legacy rămase și wizard-ul complet sunt urmărite de Etapa 4, conform
  ordinii documentului; comportamentul preview/build rămas este urmărit de Etapa 3.

## Etapa 3 — Motor embedded, producție și preview

Status: **COMPLETE**

### Rezumat audit inițial

- Build-ul de producție folosește deja `Site::build()` într-o generație privată,
  cu publicare atomică și checkpoint-uri înainte/după apelul upstream.
- Preview-ul folosește `Arc<Library>`, `RenderCache`, API-ul Tera public și
  `render_component`; mutex-ul global protejează în continuare `SITE_CONTENT`.
- Suita completă indică o diferență de contract pentru cheia rutelor memory-mode:
  Zola 0.23.4 publică pagina draft la cheia `despre`, în timp ce testul Pana
  așteaptă forma veche `despre/`.
- Trebuie auditate în continuare refresh-ul cache-ului, paritatea completă a
  matricei, CSS-ul de highlighting și asset-urile, apoi refăcute gate-urile
  specifice etapei.

### Plan scurt de implementare

1. Corectarea contractului rutelor memory-mode și verificarea draft include/exclude.
2. Izolarea fiecărui reload template/Sass/static într-o generație nouă.
3. Eliminarea fazelor manuale de asset rendering devenite redundante.
4. Selectarea componentei de preview din template-ul activ și randarea prin
   metadatele publice Tera 2.
5. Extinderea matricei cu CSS-ul de highlighting din output și rularea gate-urilor.

### Rezumat implementare

- Build-ul de producție și build-ul inițial Preview folosesc `Site::build()`;
  reload-ul reține `Site`, dar folosește API-ul public `reload_templates()`, care
  reîncarcă registrul Tera și execută coada canonică Zola.
- Orice delta de template, Sass sau static produce o generație privată nouă.
  Generația publicată nu mai poate fi modificată indirect de image processing,
  highlighting CSS ori cache busting în timpul construirii candidatului.
- A fost eliminată complet orchestrarea manuală Preview pentru Sass, CSS de
  highlighting, imagini și static assets.
- Schimbările de conținut/config/temă reconstruiesc `Site`, `Library` și
  `RenderCache`; schimbările de template reconstruiesc registrul Tera prin API-ul
  upstream. Contextul Workbench consumă valorile canonice din cache.
- Preview-ul unei componente alege definiția din fișierul activ și folosește
  `get_component_definition`/`render_component`, cu fixture-uri generate după
  tipurile și required/default publice Tera 2.
- Contractul rutelor memory-mode acceptă cheia canonică Zola 0.23.4 fără slash;
  serverul păstrează rezolvarea ambelor forme pentru URL-uri de director.
- Fixture-ul upgrade validează acum `giallo.css` în output, alături de Sass,
  search, feeds, imagine procesată, asset colocat și static.

### Verificări efectuate

- `preview::engine::tests`: PASS, 16 trecute și 1 ignorat (socket loopback).
- Paritate retained-site după template, Sass și schimbare mixtă: PASS față de
  build embedded fresh.
- Politica draft workspace include / disk exclude: PASS.
- Preview componentă exactă în prezența unei alte componente globale: PASS.
- Matricea embedded upgrade și `giallo.css`: PASS.
- `cargo check --locked` și `cargo fmt --check`: PASS.
- `cargo test --locked`: PASS final, 1.658 trecute, 20 ignorate, 0 eșecuri.
- Test runtime embedded/absență sidecar: PASS, 1/1.
- `npm run build`: PASS, inclusiv bundle guard.
- Audit `rg`: niciun apel de producție către fazele manuale Zola eliminate.

### Probleme rămase

- Nicio problemă cunoscută care aparține Etapei 3.
- Smoke testul cu socket real și aplicația/AppImage sunt gate-uri explicite ale
  Etapei 7.

## Etapa 4 — Contracte și experiența Components

Status: **COMPLETE**

### Rezumat audit inițial

- Contractele și mutațiile tratau componenta în principal ca fișier și nu
  identificau simbolul exact dintr-un fișier cu mai multe definiții.
- Components Workspace păstra taburi și fluxuri paralele pentru partial/macro,
  iar creare, filtrare, rename, usages și preview nu acopereau integral modelul
  Tera 2 tipizat.
- Template Workbench selecta prima componentă din fișier, astfel încât preview-ul
  nu era determinist pentru un fișier cu mai multe componente.
- Localele și testele UI păstrau texte și așteptări legacy, iar contractul TS al
  Workbench-ului rămăsese în urma modelului Rust.

### Plan scurt de implementare

1. Extinderea contractului de mutație cu simbolul sursă/destinație și operații
   semantice validate de Zola.
2. Înlocuirea workspace-ului legacy cu un singur catalog Components și wizard
   Tera 2 tipizat.
3. Legarea usages, navigării, provenance, rename/delete și preview de simbolul
   exact.
4. Eliminarea textelor și ramurilor UI legacy, regenerarea i18n și actualizarea
   testelor Rust/TS/UI.

### Rezumat implementare

- Mutațiile schema 3 folosesc `symbolName`, `sourceSymbol` și
  `destinationSymbol`. Create validează definiția exactă; rename rescrie atomic
  definiția și apelurile rezolvate; delete elimină numai simbolul selectat când
  fișierul conține și alte componente și refuză ștergerea cu utilizări active.
- Components Workspace expune un singur catalog Tera 2, filtre după namespace și
  origine, wizard cu argumente tipizate/default/rest/body, exemplu de apel,
  companions, usages navigabile, provenance și comenzile edit/rename/delete.
- Preview Workbench schema 5 propagă `preferredComponentName` până la Rust,
  validează simbolul și păstrează componenta exactă în cheia de reuse și în
  proiecția publicată.
- Contractele TS stale pentru componente/Workbench au fost sincronizate cu Rust;
  tipurile interne nu mai sunt exportate în afara autorității lor.
- UI-ul și localele nu mai expun macro/shortcode. Detectarea sintaxei legacy a
  rămas numai în Source Graph pentru diagnosticul explicit cerut de Etapa 2.
- Catalogul i18n a fost regenerat din cele două surse locale; testele de contract
  Components, Workbench, inserare și pattern UI au fost actualizate.

### Decizii tehnice importante

- Identitatea unei componente este simbolul namespaced, nu calea fișierului;
  duplicate/move/extract la nivel de fișier nu sunt prezentate drept operații
  semantice valide.
- Range-urile Source Graph sunt autoritatea editărilor multi-fișier, iar Zola
  validează candidatul complet înainte de publicare.
- Selectarea explicită a componentei face parte din identitatea cache-ului de
  preview; nu există fallback silențios când utilizatorul cere un simbol absent.

### Verificări efectuate

- Testele Rust focalizate pentru mutații: PASS, 10/10.
- Testele Rust focalizate Template Workbench și Preview: PASS.
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`: PASS, 1.661 teste
  trecute, 20 ignorate intenționat și 0 eșecuri.
- `npm run check`: PASS, 0 erori și 0 avertismente.
- `npm run test:kernel`: PASS final, 145/145, inclusiv arhitectură, ownership,
  i18n, iconuri și toate contractele Node.
- `npm run build`: PASS, inclusiv adapter static și bundle guard.
- `npm run i18n:generate`: PASS, 2 locale, 17 domenii, 4.120 mesaje.
- Auditul textelor vizibile și `git diff --check`: PASS.

### Probleme rămase

- Nicio problemă cunoscută care aparține Etapei 4.
- Detectoarele legacy din scanner și localele lor de diagnostic sunt deliberate
  și nu reprezintă o cale UI sau un runtime paralel.

## Etapa 5 — Funcționalități Zola 0.23 în editor

Status: **COMPLETE**

### Rezumat audit inițial

- `ZolaProjectSettings` și panoul Project Settings nu expun încă
  `skip_content_templating` ori `markdown.highlighting.data_attr_position`.
- Modelul frontend și mutația Rust de front matter cunosc `draft`, dar nu
  `hidden` sau `include_in_feeds`; absența lui `hidden` trebuie păstrată ca stare
  moștenită, iar `include_in_feeds = true` nu trebuie materializat inutil.
- Contractul administrat `resize_image` acoperă operație, format și calitate,
  dar nu filtrul opțional de sampling introdus în Zola 0.23.
- Catalogul runtime conține deja semnăturile cerute pentru `allow_missing`,
  `lang`, `text_direction`, `get_env` și `get_taxonomy_url(term=...)`, însă
  testul de catalog nu le afirmă încă pe toate explicit.
- Testele curente dovedesc păstrarea câmpurilor TOML/front matter necunoscute și
  build-ul imaginii, dar nu round-trip-ul ori build-ul noilor opțiuni.

### Plan scurt de implementare

1. Extinderea settings Rust–TS și a UI-ului cu lista de globuri și poziția
   atributelor de highlighting, fără a crea o secțiune highlighting invalidă.
2. Extinderea front matter Rust–TS/UI cu `hidden` tri-valued și
   `include_in_feeds` cu default implicit `true`.
3. Propagarea filtrului opțional prin intent, metadata, preview și inspectorul
   imaginii, emițând `filter=` numai când utilizatorul îl alege explicit.
4. Consolidarea testelor pentru semnăturile runtime, round-trip lossless și
   build embedded Zola pentru fiecare opțiune.

### Rezumat implementare

- Project Settings expune `skip_content_templating` ca listă de globuri și
  `markdown.highlighting.data_attr_position` cu valorile Zola validate. Citirea
  și scrierea TOML păstrează comentariile și câmpurile fără legătură, iar
  configurația rezultată este validată de `zola-config` înainte de commit.
- Front matter-ul paginilor și secțiunilor expune `hidden` ca stare
  moștenit/ascuns/vizibil. Paginile expun și `include_in_feeds`; valoarea
  implicită `true` este reprezentată prin absența cheii, iar `false` este
  materializat explicit.
- Inspectorul imaginilor propagă filtrul opțional `resize_image` prin contractul
  TS, intent-ul Preview și motorul Rust. Sunt acceptate valorile upstream
  `nearest`, `triangle`, `catmull_rom`, `gaussian` și `lanczos3`; absența nu
  produce un argument redundant.
- Catalogul runtime și testele sale afirmă explicit `allow_missing` și `lang`
  pentru pagini/secțiuni, `get_env`, `text_direction` și
  `get_taxonomy_url(term=..., lang=...)`, păstrând diagnosticul deprecării lui
  `name`.
- Un proiect embedded dedicat construiește împreună globurile de templating,
  poziția atributelor de highlighting, moștenirea `hidden`, excluderea din feed
  și filtrul de imagine și verifică output-ul rezultat.

### Decizii tehnice importante

- `data_attr_position` poate fi modificat numai când proiectul are deja o
  secțiune `[markdown.highlighting]` validă; editorul nu inventează o secțiune
  incompletă fără tema obligatorie.
- Default-urile Zola sunt păstrate prin absență pentru `include_in_feeds` și
  filtrul de imagine, evitând modificări TOML/front matter fără semnificație.
- Validarea finală aparține crate-ului oficial `zola-config`, nu unei liste
  paralele de reguli reimplementate în frontend.

### Verificări efectuate

- Teste Rust settings: PASS, 7/7, inclusiv round-trip lossless, glob invalid și
  secțiune highlighting absentă.
- Teste Rust front matter: PASS, 9/9, inclusiv toate cele trei stări `hidden` și
  semantica implicită pentru feed.
- Teste Rust image contract: PASS, 4/4; test catalog runtime: PASS, 1/1.
- Test embedded combinat pentru toate opțiunile Etapei 5: PASS, 1/1.
- Teste Node focalizate pentru settings/front matter/imagini și UI: PASS.
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`: PASS, 1.667 teste
  trecute, 20 ignorate intenționat și 0 eșecuri.
- `npm run test:kernel`: PASS, 145/145, inclusiv TypeScript, arhitectură, i18n și
  contract ownership.
- `npm run build`: PASS, inclusiv adapter static și bundle guard.
- `cargo check --locked`, `npm run check`, `cargo fmt --check` și
  `git diff --check`: PASS.

### Probleme rămase

- Nicio problemă cunoscută care aparține Etapei 5.
- Auditul și conversia tuturor resurselor bundled sunt urmărite separat în
  Etapa 6, conform documentului principal.

## Etapa 6 — Conversia conținutului bundled și metadata

Status: **COMPLETE**

### Rezumat audit inițial

- Toate cele cinci startere sunt deja materializate ca proiecte locale valide
  și au test embedded de build, dar manifestele declară încă
  `tested = "0.22.1"`, iar matricea nu le randează pe toate în Preview.
- Fixture-ul `index-zero` conține exact trei utilizări ale filtrului Tera 1
  `slice(end=...)`; nu au fost găsite macro-uri, importuri macro, directoare ori
  apeluri shortcode sau `get_taxonomy_url(name=...)` în resursele bundled.
- Niciun fișier Markdown din startere ori `index-zero` nu conține delimitatori
  `{{`/`{%`; nu este necesară introducerea artificială de `raw` sau
  `skip_content_templating`.
- README, `THIRD_PARTY_NOTICES.md` și documentele active ale fixture-ului
  `index-zero` păstrează versiunea/revizia veche. About consumă deja versiunea
  autoritativă din backend și nu are literal de actualizat.
- Inventarul generat al licențelor reflectă graph-ul Cargo curent, dar trebuie
  regenerat/verificat după actualizarea notice-ului și trebuie adăugate gate-uri
  care împiedică revenirea metadata ori sintaxei legacy.

### Plan scurt de implementare

1. Convertirea slicing-ului `index-zero` la sintaxa nativă Tera 2 și adăugarea
   unui build embedded dedicat fixture-ului.
2. Actualizarea manifestelor celor cinci startere și verificarea
   create/open/Preview/build pentru întregul catalog.
3. Adăugarea unui audit automat al template-urilor/Markdown-ului bundled pentru
   sintaxa incompatibilă și templating literal accidental.
4. Actualizarea README, notices și documentelor active, păstrând neschimbate
   changelog-ul și baseline-urile istorice.
5. Regenerarea/verificarea licențelor și rularea tuturor gate-urilor Etapei 6.

### Rezumat implementare

- Cele trei filtre Tera 1 `slice(end=...)` din `index-zero` au fost înlocuite
  cu slicing nativ Tera 2 (`pages[:n]`), iar fixture-ul complet trece acum
  verificarea și build-ul embedded 0.23.4.
- Toate cele cinci manifeste bundled declară `tested = "0.23.4"`. Matricea de
  teste dovedește materializarea/create-open, randarea Preview și build-ul
  embedded pentru fiecare starter.
- Testul de contract al starterelor auditează recursiv template-urile și
  Markdown-ul pentru macro/import, filtrul `slice`, directoare shortcode,
  argumentul depreciat `name` și delimitatori Markdown neintenționați.
- README și `THIRD_PARTY_NOTICES.md` indică versiunea/revizia 0.23.4 și cele
  trei crate-uri Zola directe. Documentele active `index-zero` au fost
  actualizate; About a rămas legat de contractul backend, fără literal local.
- Inventarul complet al licențelor a fost regenerat din lockfile-uri și metadata
  Cargo și verificat în modul read-only.

### Decizii tehnice importante

- Nu au fost adăugate blocuri `raw` ori globuri `skip_content_templating`,
  deoarece auditul nu a găsit delimitatori literali în Markdown-ul livrat.
- Changelog-ul, planul de upgrade și baseline-urile 0.22.1 rămân istoric
  explicit și nu au fost rescrise.
- Preview-ul fiecărui starter rulează dintr-o copie temporară și într-un output
  separat; resursele canonice nu sunt mutate de teste.

### Verificări efectuate

- Create/open pentru cele cinci opțiuni: PASS, 1 matrice / 5 startere.
- Preview embedded pentru toate starterele: PASS, 1 matrice / 5 startere.
- Check + build embedded pentru toate starterele: PASS, 1 matrice / 5 startere.
- Check + build embedded `index-zero`: PASS.
- Audit automat bundled și contract metadata/documentație: PASS.
- `npm run licenses:generate && npm run licenses:check`: PASS, 987 pachete și
  476 texte de licență unice.
- `npm run check`: PASS, 0 erori și 0 avertismente.
- `npm run test:kernel`: PASS, 145/145.
- `npm run build`: PASS, inclusiv adapter static și bundle guard.
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`: PASS, 1.669 teste
  trecute, 20 ignorate intenționat și 0 eșecuri.
- `cargo fmt --check`, `git diff --check` și auditul versiunii/SHA-ului vechi:
  PASS; aparițiile rămase sunt exclusiv istorice.

### Probleme rămase

- Nicio problemă cunoscută care aparține Etapei 6.

## Etapa 7 — Validare finală și pregătirea release-ului

Status: **COMPLETE**

### Rezumat audit inițial

- Gate-urile automate pentru Svelte/TypeScript, kernel/frontend, build, Rust,
  runtime embedded, licențe, startere și matricea multilingvă sunt prezente și
  au trecut la checkpoint-ul Etapei 6.
- Nu există încă un AppImage construit din sursele post-upgrade și nici un
  raport de performanță post-upgrade comparabil cu baseline-ul Etapei 0;
  binarul release existent este captura veche de 132.319.504 bytes.
- Smoke testul real pentru Preview, editare și build/deploy local nu este
  acoperit numai de testele unitare. Proiectul include helpers AT-SPI pentru
  pornirea aplicației și deschiderea controlată a unui proiect.
- Auditul preliminar `cargo tree -d` arată Tera 2.2.0 unic și întregul subgraf
  Zola 0.23.4 la revizia fixată. Familiile duplicate generale ale aplicației
  trebuie inventariate final, fără upgrade-uri în afara scopului.
- `check:unused`, verificările de ownership/reachability și compilarea Rust sunt
  deja gate-uri, dar auditul final trebuie să confirme explicit lipsa runtime-
  ului legacy, a fazelor Zola duplicate și a codului mort introdus de upgrade.

### Plan scurt de implementare

1. Smoke test în aplicația Tauri reală pe o copie temporară a fixture-ului:
   deschidere, Preview, editare/salvare și build local.
2. Construirea AppImage-ului release, verificarea structurii/checksum-ului și
   smoke test de pornire pe artefactul rezultat.
3. Rularea profilului release de performanță pe același fixture ca Etapa 0,
   capturarea memoriei și dimensiunilor și compararea metricilor.
4. Audit final `rg`, `cargo tree -d`, cod mort, formatare și licențe; corectarea
   oricărei regresii care aparține upgrade-ului.
5. Rerularea tuturor gate-urilor obligatorii și auditul final al documentului.

### Rezumat implementare

- Smoke testul Tauri real a deschis o copie temporară a starterului minimal,
  a randat Preview-ul embedded, a salvat o editare din CodeMirror și a produs
  artifactul local `public/index.html`. Preflight-ul Publish a validat Zola
  embedded 0.23.4 și a blocat corect deploy-ul extern fără target/credentiale.
- AppImage-ul final a fost construit și pornit. Backend-ul, WebKit și MCP au
  rămas active fără crash, apoi s-au închis fără procese reziduale.
- Fixture-ul de performanță a fost convertit la Tera 2 nativ după ce prima
  rulare a identificat macro/import legacy. Reconstrucția grafului de componente
  a fost optimizată cu index pe fișier și reutilizare incrementală, păstrând
  rezoluția globală a apelurilor Tera 2 și echivalența cu full scan.
- Raportul final a fost salvat în
  `docs/zola-0.23.4-upgrade-validation-2026-08-28.md`, iar rezultatele brute în
  `docs/zola-0.23.4-upgrade-performance-2026-08-28.json`.

### Decizii tehnice importante

- Nu a fost executat niciun deploy extern: proiectul de smoke nu avea target
  ori credentiale, iar gate-ul cere deploy/build local, nu efecte remote.
- Component definitions neafectate sunt reutilizate incremental; toate apelurile
  sunt re-rezolvate față de tabelul global Tera 2. Testele incremental/full scan
  confirmă identitatea rezultatului.
- Cele șase bugete aspiraționale preexistente rămân vizibile în JSON. Gate-ul de
  upgrade compară aceleași probe cu baseline-ul și este PASS: toate metricile
  finale sunt mai bune, fără a redefini retroactiv baseline-ul.

### Verificări efectuate

- Smoke real Preview/edit/save/build local și Publish preflight: PASS.
- `npm run tauri build`: PASS; AppImage 121.195.000 bytes, SHA-256
  `f4cbd4fa7ed355dcaa5b7a54bc9715816285c971b530d4f2bb03b818bc77809d`.
- Smoke AppImage final: PASS; backend/WebKit/MCP active, 228.160 KiB RSS pentru
  procesul principal, zero procese reziduale după închidere.
- Performanță p95 vs 0.22.1: external reconcile -1,8%, startup -5,4%, CSS -4,2%,
  HTML incremental -42,3%, project open -74,5%, full rebuild -70,8%, clone -7,9%.
  Benchmark cache-uit: 194.840 KiB RSS; 5/5 probe funcționale PASS.
- Binar release 132.271.632 bytes: -47.872 bytes (-0,036%) față de baseline;
  frontend/client +0,379%/+0,377%, cel mai mare chunk neschimbat.
- `npm run check`: PASS, 0 erori și 0 avertismente.
- `npm run test:kernel`: PASS, 145/145.
- `npm run build`: PASS, inclusiv adapter static și bundle guard.
- `cargo test --locked`: PASS, 1.669 trecute, 20 ignorate intenționat.
- `cargo clippy --all-targets --locked -- -D warnings`: PASS.
- `node --test tests/zola-embedded-runtime.test.mjs`: PASS, 1/1.
- `npm run licenses:check`: PASS, 987 pachete și 476 texte unice.
- Matrice create/open/Preview/build pentru toate starterele, fixture-ul
  multilingv și `index-zero`: PASS în suita Rust finală.
- `cargo tree -d` și metadata: auditate; o singură Tera 2.2.0, toate crate-urile
  Zola 0.23.4 din aceeași revizie. Dublurile `reqwest`/`sha2` rămân deliberate.
- `check:unused`, auditul `rg`, `cargo fmt --check` și `git diff --check`: PASS.

### Audit final al etapei

- Toate cele 10 gate-uri obligatorii și criteriul de acceptare au fost
  recitite și adresate.
- Nu există fallback 0.22.1, sidecar Zola, runtime macro/shortcode, faze manuale
  duplicate sau regresii de performanță semnificative.
- Referințele 0.22.1 rămase sunt exclusiv istoric explicit ori versiunea fără
  legătură a crate-ului `base64`.

### Probleme rămase

- Nicio problemă cunoscută care aparține upgrade-ului Zola 0.23.4.

## Extensie post-upgrade — Categoria semantică `partial`

Status: **COMPLETE**

### Rezumat audit inițial

- SourceGraph detecta deja resursele din `partials/`, consumatorii Tera direcți
  și paginile afectate tranzitiv, dar catalogul le excludea din toate intrările
  semantice afișate de activitatea Șabloane.
- Template Workbench rezolva deja un partial inclus în consumatorul Zola real și
  accepta o pagină preferată; nu era necesar un al doilea motor de preview.
- Redenumirea, duplicarea, override-ul de temă și ștergerea protejată existau ca
  tranzacții ProjectWorkspace generice. Lipseau crearea semantică `partial` și
  expunerea acestor operații într-o categorie proprie.
- `templates/components/` are rolul separat `ComponentLibrary`, iar
  `listing-items/` are rol și comandă CRUD specializate; ambele trebuiau să
  rămână în afara noii categorii.

### Rezumat implementare

- Contractul catalogului a fost ridicat la schema 6 și include acum categoria și
  rolul semantic `partial`. Fiecare resursă `partial` efectivă primește o intrare
  unică, indiferent dacă provine local sau din tema activă.
- Tabul „Parțiale” afișează șabloanele care includ direct resursa și toate
  paginile afectate tranzitiv. Fiecare pagină poate deschide partialul în
  contextul său real de preview.
- Crearea este validată în Rust și limitată la `templates/partials/`; draftul nu
  conține `extends`. Redenumirea și duplicarea păstrează acest spațiu semantic,
  iar controlul de layout este ascuns și respins în backend pentru partiale.
- Operațiile existente de rename, duplicate, theme override și delete protejat
  sunt reutilizate. Resursele `components/` și `listing-items/` rămân excluse
  explicit din categoria `partial`.

### Verificări efectuate

- Test contract activitate Șabloane: PASS.
- Teste Rust catalog semantic: PASS, 4/4.
- Test Rust creare semantică și namespace partial: PASS.
- `npm run check`: PASS, 0 erori și 0 avertismente.
- `npm run test:kernel`: PASS, 145/145, inclusiv contractele de arhitectură,
  ownership, localizare și densitate vizuală.
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`: PASS, 1.670 teste
  trecute, 20 ignorate intenționat și 0 eșecuri.
- `npm run build`: PASS, inclusiv adapter static și bundle guard.
- `cargo fmt --check` și `git diff --check`: PASS.

### Probleme rămase

- Nicio problemă cunoscută care aparține categoriei semantice `partial`.
