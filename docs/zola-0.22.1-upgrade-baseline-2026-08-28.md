# Baseline înainte de upgrade-ul Zola 0.23.4

Data rulării: 28 august 2026
Motor embedded: Zola 0.22.1 (`29540e9897dbe8aca388b13f7bdf615985f6ca2c`)
Commit de bază: `b3cd0856331c6b1969b94a097b5718876ecf7115`
Rust/Cargo: 1.96.1
Node.js: 24.18.0

## Verdict

Baseline-ul funcțional este verde. Type checking, kernel-ul frontend, build-ul
frontend, întreaga suită Rust, runtime-ul embedded, matricea Zola, paritatea
Preview/disk și toate cele cinci startere au trecut.

Baseline-ul release de performanță a executat cu succes toate cele cinci probe,
dar are status `failed` deoarece șase bugete aspiraționale preexistente sunt
depășite. Aceste depășiri sunt consecvente cu diagnosticele din
`docs/performance-baseline-standard-v1.md`; nu au fost provocate de upgrade,
care nu începuse la momentul măsurării. Rezultatele brute sunt păstrate în
`docs/zola-0.22.1-upgrade-performance-baseline-2026-08-28.json` și trebuie
comparate cu măsurarea post-upgrade din Etapa 7.

## Verificări funcționale

| Verificare | Rezultat | Durată | Memorie maximă RSS |
| --- | --- | ---: | ---: |
| `npm run check` | PASS, 0 erori și 0 avertismente | 65,78 s | 1.619.832 KiB |
| `npm run test:kernel` | PASS, 145/145 | 41,85 s | 476.892 KiB |
| `npm run build` | PASS, inclusiv bundle guard | 80,92 s | 3.174.200 KiB |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked` | PASS, 1.662 trecute, 20 ignorate intenționat | 31,33 s | 1.109.988 KiB |
| `node --test tests/zola-embedded-runtime.test.mjs` | PASS, 1/1 | 0,16 s | 51.544 KiB |
| testele `deploy::zola` | PASS, 12/12 | 3,10 s runtime | — |
| paritate fixture upgrade Preview/disk | PASS, 1/1 | 0,51 s runtime | — |
| `cargo build --manifest-path src-tauri/Cargo.toml --release --locked` | PASS | 249,10 s | 6.593.140 KiB |

Numărul suitei Rust de mai sus este captura inițială. După adăugarea testului
dedicat de paritate Preview/disk, suita conține încă un test; checkpoint-ul
Etapei 0 înregistrează rezultatul rerulării finale.

## Matricea Zola capturată

Fixture canonic: `tests/fixtures/projects/zola-upgrade-baseline/`.

Acesta verifică împreună:

- pagina și secțiunea;
- taxonomia și feed-urile taxonomiei;
- paginarea;
- compilarea Sass;
- procesarea unei imagini prin `resize_image`;
- search index pentru română și engleză;
- feed pentru limba implicită și limba secundară;
- conținut și rute multilingve;
- asset colocat și resursă statică.

Testul de paritate randează același fixture în modul Memory folosit de Preview și
în modul Disk folosit de build, compară fiecare document și resursele derivate.
Testul starterelor copiază în directoare temporare și validează/construiește
`cadru`, `minimal`, `nord`, `pana-studio` și `radacini` fără a modifica sursele.

## Publicare atomică și anulare

Testele deterministe confirmă:

- anularea înainte de build păstrează artifactul publicat;
- anularea injectată exact după randare și înainte de publicare păstrează
  artifactul anterior și elimină generația privată;
- eroarea de randare păstrează artifactul anterior;
- build-ul reușit înlocuiește output-ul vechi și nu publică fișierele private ale
  aplicației.

## Build și dimensiuni

- director frontend `build`: 5.346.403 bytes;
- client SvelteKit: 5.371.447 bytes;
- binar release `src-tauri/target/release/pana-studio`: 132.319.504 bytes;
- graful inițial română: 1.255.916 bytes raw / 323.057 bytes gzip;
- cel mai mare chunk client: 490.966 bytes.

## Performanță release

Runnerul a compilat profilul release și a executat 5/5 teste de performanță în
712,76 s, cu RSS maxim 6.558.308 KiB. Nu lipsesc operații.

| Operație/metrică | p95 măsurat | Buget | Verdict |
| --- | ---: | ---: | --- |
| external reconcile | 7.220 µs | 10.000 µs | PASS |
| project open | 450.920 µs | 40.000 µs | preexistent, peste buget |
| HTML edit incremental | 72.624 µs | 20.000 µs | preexistent, peste buget |
| CSS edit incremental | 6.639 µs | 1.500 µs | preexistent, peste buget |
| project model build | 72.624 µs | 20.000 µs | preexistent, peste buget |
| HTML full rebuild | 413.110 µs | 50.000 µs | preexistent, peste buget |
| project model clone | 7.002 µs | 1.500 µs | preexistent, peste buget |

Aceste șase depășiri nu sunt rezolvate în upgrade-ul Zola decât dacă o modificare
din etapele următoare le agravează direct. Etapa 7 trebuie să compare aceleași
operații și să investigheze orice regresie față de valorile de mai sus, separat
de bugetele aspiraționale deja neîndeplinite.

## Condiții de comparație

Rularea a avut loc pe sistemul desktop activ, nu într-un laborator izolat.
Comparația post-upgrade trebuie făcută pe același hardware și toolchain, cu
același fixture și aceleași comenzi. Schimbarea deliberată a Tera/Zola poate
modifica timpul de compilare, dimensiunea binarului și costul parsing/randare;
aceste diferențe trebuie raportate explicit.
