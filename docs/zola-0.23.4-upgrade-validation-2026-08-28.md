# Validare finală upgrade Zola 0.23.4

Data: 28 august 2026
Motor embedded: Zola 0.23.4 (`28daab8d47cacb1e6c863b97739148f424433f5b`)
Baseline comparat: `docs/zola-0.22.1-upgrade-baseline-2026-08-28.md`

## Verdict

Toate gate-urile funcționale și arhitecturale sunt verzi. Aplicația folosește o
singură Tera 2.2.0 și toate crate-urile Zola provin din același commit 0.23.4.
Nu există sidecar Zola, fallback 0.22.1, runtime macro/shortcode sau orchestrare
manuală paralelă a build-ului.

## Smoke test real

O copie temporară a starterului minimal a fost deschisă în aplicația Tauri reală.
Preview-ul embedded a randat pagina, o modificare `stage7-smoke-0234` făcută și
salvată din editor a fost observată pe disc și în Preview, iar taskul local de
build a produs `public/index.html` cu modificarea. Publish preflight a validat
explicit proiectul cu Zola embedded 0.23.4 și a blocat corect publicarea externă
în absența unei ținte și a credentialelor.

AppImage-ul final a pornit backend-ul, procesele WebKit și serverul MCP, fără
crash. Procesul principal stabilizat a folosit 228.160 KiB RSS și s-a închis fără
procese reziduale.

## Performanță față de Zola 0.22.1

Fixture-ul de performanță a fost convertit din macro/import Tera 1 la markup
Tera 2 nativ. O regresie intermediară a grafului de componente a fost eliminată
prin indexarea nodurilor pe fișier și reutilizarea incrementală a definițiilor;
apelurile Tera 2 sunt încă rezolvate global pentru corectitudine. Testele de
echivalență incremental/full scan trec.

| Metrică p95 | 0.22.1 | 0.23.4 | Diferență |
| --- | ---: | ---: | ---: |
| external reconcile | 7.220 µs | 7.088 µs | -1,8% |
| startup inspection | 7.579 µs | 7.166 µs | -5,4% |
| CSS edit | 6.639 µs | 6.361 µs | -4,2% |
| HTML edit incremental | 72.624 µs | 41.920 µs | -42,3% |
| project model build | 72.624 µs | 41.920 µs | -42,3% |
| project open | 450.920 µs | 115.209 µs | -74,5% |
| HTML full rebuild | 413.110 µs | 120.711 µs | -70,8% |
| project model clone | 7.002 µs | 6.452 µs | -7,9% |

Rularea cache-uită a benchmark-ului a avut RSS maxim 194.840 KiB. Toate cele
cinci probe funcționale trec și nu există regresii față de baseline. Raportul
brut păstrează statusul `failed` numai fiindcă aceleași șase bugete aspiraționale
preexistente rămân sub țintele istorice; acestea au fost documentate înaintea
upgrade-ului și nu sunt folosite pentru a masca regresii.

## Dimensiuni și artefact

- binar release: 132.271.632 bytes, cu 47.872 bytes (-0,036%) sub baseline;
- frontend `build`: 5.366.655 bytes (+0,379%);
- client SvelteKit: 5.391.722 bytes (+0,377%);
- graf inițial română: 1.259.560 bytes raw / 324.063 bytes gzip
  (+0,290% / +0,311%);
- cel mai mare chunk client: 490.966 bytes, neschimbat;
- AppImage: 121.195.000 bytes;
- SHA-256 AppImage:
  `f4cbd4fa7ed355dcaa5b7a54bc9715816285c971b530d4f2bb03b818bc77809d`.

## Gate-uri finale

- `npm run check`: PASS, 0 erori/avertismente.
- `npm run test:kernel`: PASS, 145/145.
- `npm run build`: PASS, inclusiv bundle guard.
- `cargo test --locked`: PASS, 1.669 trecute, 20 ignorate intenționat.
- `cargo clippy --all-targets --locked -- -D warnings`: PASS.
- runtime embedded, licențe, startere, matrice multilingvă și `index-zero`: PASS.
- `cargo fmt --check`, `git diff --check`, `check:unused`, audit legacy și
  dependency graph: PASS.
