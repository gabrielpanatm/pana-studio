# Protocol benchmark Pană Studio v1

## Scop

Acest protocol măsoară aplicația înaintea optimizărilor. Rezultatele sunt
baseline, nu modificări ale algoritmilor aplicației. Toate comparațiile de după
optimizare trebuie să folosească aceeași versiune de protocol și aceleași
fixture-uri verificate prin SHA-256.

## Fixture-uri

Sursa canonică este proiectul INDEX ZERO din
`tests/fixtures/projects/index-zero`, seed `20270821`. Runnerul copiază sursa,
execută generatorul Rust în copie și publică o rădăcină Zola read-only.
Profilurile `margine-disk` și `peste-limita` sunt normalizate la exact 991,
respectiv 1.001 fișiere în rădăcina pe care aplicația o deschide efectiv. Profilul
de 991 trebuie acceptat de contractul manifestului; proiecția File Explorer
limitată la 500 de intrări este raportată distinct. Profilul de 1.001 trebuie
refuzat fail-closed.
Proiectul canonic rămâne obligatoriu în profilul `mare`.

## Suite

| Suită | Utilizare | Kernel | UI warm | Open cold | Operații susținute |
| --- | --- | ---: | ---: | ---: | ---: |
| `smoke` | iterație locală | 20 | 5 | 2 | 100 |
| `standard` | baseline oficial | 100 | 30 | 10 | 500 |
| `soak` | validare release | 250 | 100 | 10 | 1.500 |

Warmup-ul nu intră în distribuții. `p99` este publicat numai pentru minimum 100
probe. Rulările cold și warm nu se amestecă.

## Straturi

1. Kernel Rust: open/model, HTML, CSS, reconciliere, lock-uri și fallback.
2. Build: frontend, bundle și Zola check/build.
3. End-to-end: selectare folder, commit, workspace, preview și Canvas utilizabil.
4. Interacțiuni: activități, documente, selecție, inspector, editări și Undo/Redo.
5. WebKit: input-to-două-cadre, frame time, DOM, CSS, media și motion.
6. Resurse: arbore de procese, RSS/PSS, CPU ticks și I/O; vârf și retenție.
7. Contracte: 991 trebuie acceptat la limită, iar 1.001 trebuie să eșueze
   fail-closed, fără workspace parțial.

## Persistența datelor

Fiecare probă este adăugată și flush-uită în `samples.jsonl` înaintea oricărui
verdict. `run.json` este scris mai întâi cu status `running`, apoi finalizat.
Eșecurile păstrează datele parțiale. `report.json` și `report.md` sunt derivate și
pot fi regenerate din JSONL împreună cu manifestul `run.json` alăturat.
Adaptoarele sunt citite linie cu linie, iar stdout și proba normalizată sunt
flush-uite înainte de citirea liniei următoare. WebKit execută familiile de
scenarii în loturi de maximum 10 cicluri, cu timeout separat; un lot eșuat nu
șterge loturile deja persistate.

## Mediu

Se înregistrează commit-ul, digestul worktree-ului, versiunile Rust/Cargo/Node/
Zola/kernel, CPU, RAM, swap, load average, governor și temperaturile disponibile.
Compilarea este raportată separat și nu intră în latențele runtime.

## Praguri

Țintele aspiraționale sunt: acțiuni frecvente `p95 ≤ 50 ms`, schimbare de
activitate/document `p95 ≤ 100 ms`, Canvas warm `p95 ≤ 1 s`, Canvas cold
`p95 ≤ 2 s` și cadre `p95 ≤ 16,7 ms`. Regresia este evaluată separat: o creștere
de peste 10% a p95, după zgomotul măsurat, necesită justificare explicită. Marja
de zgomot a comparației este derivată din dispersia robustă p50–p95 a ambelor
rulări, limitată la 2,5–5 puncte procentuale; pragul efectiv este 10% plus marja.
Pragurile aspiraționale nu șterg și nu invalidează probele brute.

Fiecare raport conține `benchmarkIdentity`: versiunea protocolului, versiunea
schemei probelor, suita, run-ul, SHA-256 pentru fiecare fixture și identitatea
stabilă a mediului (CPU, număr de fire, RAM total, kernel, Rust/Cargo/Node/Zola
și governor). O comparație este refuzată înainte de calcul dacă protocolul,
schema, suita, fixture-urile, hardware-ul sau toolchain-ul diferă. În particular,
`smoke`, `standard` și `soak` nu sunt baseline-uri reciproc comparabile, deoarece
au volume de lucru diferite. P95 este comparat numai pentru distribuții cu
minimum 10 probe în ambele rulări; valorile singleton de build/diagnostic rămân
în raport, dar nu pot produce verdicte de regresie.

DOM/CSS/media din iframe-ul Preview sunt publicate numai când WebKit expune un
target sau un execution context inspectabil pentru procesul cross-origin. În
caz contrar, probele rămân în JSONL cu status `unavailable`, motiv explicit și
numărul targeturilor/contextelor observate; valoarea zero nu este interpretată
ca măsurătoare reușită. Numărul de evenimente și dimensiunea jurnalului kernel
sunt proxy-uri pentru trafic; payload-ul IPC Tauri nu este estimat fără un hook
autoritar, pentru a evita cifre fabricate.

Un raport poate fi regenerat și comparat fără rerularea suitei:

```bash
cargo run --release --manifest-path tools/performance-benchmark/Cargo.toml -- \
  report --raw RUN/samples.jsonl --json RUN/report.json \
  --markdown RUN/report.md --baseline docs/performance-baseline-standard-v1.json
```

## Comenzi canonice

```bash
npm run performance:harness:test
npm run performance:smoke
npm run performance:standard
npm run performance:soak
```

Gate-ul mai lent care materializează de două ori toate cele cinci profile,
compară inventarul și SHA-256 și confirmă că sursa canonică rămâne neschimbată:

```bash
cargo test --manifest-path tools/performance-benchmark/Cargo.toml \
  canonical_profiles_are_deterministic_and_leave_source_immutable -- --ignored
```

Volumele brute din `benchmark-results/` nu se comit. Baseline-ul oficial compact
și concluziile factuale se păstrează în `docs/`.
