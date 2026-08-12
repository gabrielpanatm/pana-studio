# FontFaceGraph — contract și baseline

## Autoritate

`FontFaceGraph` este singura proiecție pentru Font Manager. Rust construiește lanțul:

`token SCSS -> font-family CSS -> @font-face -> src URL -> resursă activă -> metadate OpenType`.

ID-ul public este `css:<familie normalizată>`. Normalizarea elimină doar ghilimelele exterioare, comprimă spațiile și aplică lowercase; păstrează punctuația semnificativă. Directorul este doar proveniență (`directories[]`), niciodată identitate. URL-urile locale sunt rezolvate prin indexul căii publice, cu resursa locală înaintea aceleiași căi din tema activă. `public/` nu este sursă de adevăr.

Frontend-ul primește snapshot-ul Rust și folosește `family.id`. Rolurile, preview-ul, preload-ul, `font-display`, importul și eliminarea controlată consultă același graf. Operațiile de familie acceptă `familyId`; preload-ul și preview-ul acceptă calea exactă demonstrată de graf.

## Cache și invalidare

- Snapshot graf: cheie `(project_root, runtime_session_id, workspace_revision, accepted_disk.generation)`, maximum 16 snapshot-uri fără bytes binari.
- Fișier disk: cheie `(zola_root, relative_path, manifest.version_token)`, maximum 4.096 intrări.
- Metadate OpenType: cheie `content_hash`, maximum 4.096 intrări; câmpurile dependente de cale/nume sunt reaplicate la cache hit.
- Orice mutație ProjectWorkspace schimbă revizia. Reconcilierea disk schimbă generația/tokenul. O sesiune sau un proiect nou nu poate reutiliza un snapshot vechi.
- Preview-ul reutilizează graful și citește/transferă numai fișierul solicitat. Bytes binari rămân în ProjectWorkspace sau pe disk, nu în cache-ul grafului.

Memoria este `O(faces + assets + stylesheets)` per snapshot. Cache-urile au limite explicite și sunt golite la depășirea lor.

## Theme packs

Fișierele Inter și Poppins provin din CSS-ul oficial Google Fonts din 2026-08-12. Fiecare face are perechi Latin/Latin-ext și `unicode-range` exact; licențele OFL existente rămân lângă binare.

- Inter variable 100–900: Latin `2c295d99…`, Latin-ext `5e6d4fe9…`.
- Poppins 600: Latin `f4e80d9d…`, Latin-ext `bb1f2d58…`.
- Poppins 700: Latin `9338e65f…`, Latin-ext `ccfd87f6…`.

Testul Rust verifică metadatele, weight/style, lipsa duplicatelor intra-temă și reuniunea glyph-urilor `ăâîșțĂÂÎȘȚ` pentru `cadru`, `nord`, `pana-studio` și `radacini`. Pachetele sunt self-contained; nu se migrează automat proiectele utilizatorilor.

## Baseline release

Comandă:

```text
cargo test --release fonts::graph::tests::benchmark_font_face_graph_100_and_1000_faces --lib -- --ignored --nocapture
```

Baseline pe mașina de dezvoltare, 2026-08-12, mediană din 31 rulări:

- 100 faces: `460.236 µs`
- 1.000 faces: `4.332023 ms`
- cost normalizat: `0.941` (`<= 1.10`)

Prima execuție a testului a expus rescannare cvadratică (`4.667 ms` / `368.482 ms`, normalizat `7.895`). Cursorul parserului este acum incremental, iar intervalele markerilor managed sunt calculate o dată per familie. Testul ignorat eșuează dacă 1.000 faces depășesc cu peste 10% costul liniar raportat la 100.

## Acceptanță 11111d

```text
PANA_FONT_GRAPH_PROJECT=/home/gabriel/11111d \
cargo test fonts::graph::tests::real_project_reports_css_aliases_without_false_missing_faces --lib -- --ignored
```

Acceptanța cere `Primary -> Inter`, `Display -> Poppins`, toate cele cinci face-uri vechi rezolvate, `swap`, zero `font_face_missing` fals și `$font-ui` absent. Duplicatele Inter și mismatch-ul istoric Poppins 600/400 rămân diagnostice reale, fără a modifica proiectul.
