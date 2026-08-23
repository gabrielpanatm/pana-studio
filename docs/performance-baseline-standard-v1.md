# Baseline standard de performanță v1 — 2026-08-22

## Verdict

Acesta este baseline-ul oficial înaintea optimizărilor. Suita `standard` a rulat
105,37 minute, între 2026-08-22 04:53:45 și 06:39:07 (Europe/Bucharest), și a
produs 7.834 probe brute, 379 distribuții și 31 de verdicte de buget.
Rezultatul este `complete_with_diagnostics`: măsurarea este validă, dar aplicația
nu îndeplinește încă obiectivul de răspuns aproape instantaneu. Au eșuat 26 din
31 de bugete aspiraționale.

Artefactul comparabil, fără JSONL-ul de 2 MiB, este
`docs/performance-baseline-standard-v1.json`. Sursa locală completă a rulării
este `benchmark-results/standard-1787363625196-1110733/` și rămâne ignorată de
Git. Schema și metodologia sunt definite în `benchmarks/protocol-v1.md`.

## Identitatea rulării

- commit: `16b22e691a229a2ecf993101ebecb3c184b77de9`;
- worktree: modificat, digest SHA-256
  `d236168c4fcd7075950e2047f5a5685c018fc147b7ded87319de4789d82f78a0`;
- CPU: Intel Xeon E5-2620 v3, 12 fire; governor `schedutil`;
- RAM: 15,53 GiB; disponibil la start: 10,49 GiB; swap liber: 1,52 MiB din 2 GiB;
- load average la start: `5.31 4.58 4.84`;
- Rust/Cargo 1.96.1, Node 24.18.0, Zola 0.22.1, Linux 7.0.0-29.

Mediul nu este unul de laborator complet izolat. Comparațiile ulterioare trebuie
să folosească aceeași suită, aceleași fixture-uri și aceeași identitate stabilă
de hardware/toolchain. Runnerul refuză automat baseline-urile incompatibile și
compară numai distribuții cu minimum 10 probe în ambele rulări.

## Fixture-uri

| Profil | Fișiere | Directoare | Bytes | Așteptare | SHA-256 (prefix) |
| --- | ---: | ---: | ---: | --- | --- |
| control | 126 | 20 | 1.397.621 | acceptat | `16719d89bc79` |
| mare | 446 | 20 | 2.040.801 | acceptat | `fe0da12b305e` |
| densitate | 228 | 20 | 2.149.655 | acceptat | `c2046b4886c4` |
| margine-disk | 991 | 21 | 1.432.236 | acceptat la manifest | `37e5c25c092a` |
| peste-limita | 1.001 | 21 | 1.433.511 | refuz fail-closed | `ebf581dc0890` |

Fixture-urile au fost regenerate cu seed `20270821`, făcute read-only, verificate
prin inventar și SHA-256 după probe, apoi eliminate. Proiectul canonic INDEX ZERO
nu a fost modificat.

## Rezultate end-to-end

Toate valorile sunt p95. `terminal` pentru profilul 1.001 este timpul până la
refuzul corect; la 991 este timeout-ul funcțional, nu un open reușit.

| Profil | Canvas cold | Canvas warm | Terminal | Acțiune susținută | Document switch | Frame time | Peak PSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| control | 6.908,500 ms | 505 ms | 6.908,500 ms | 207,021 ms | 9.863 ms | 34 ms | 908.215 KiB |
| mare | 12.736,287 ms | 3.616 ms | 12.736,287 ms | 232,417 ms | 30.592 ms | 34 ms | 1.613.387 KiB |
| densitate | 16.572,462 ms | 6.955 ms | 16.572,462 ms | 238,474 ms | 16.194 ms | 34 ms | 1.420.364 KiB |
| 991 | indisponibil | indisponibil | 180.174,952 ms, eșuat | — | — | — | 388.187 KiB |
| 1.001 | refuzat | — | 257,178 ms, corect | — | — | — | 380.619 KiB |

Cele 540 de probe frame-time au fost capturate cu `document.visibilityState =
visible`, deci p95 de 34 ms nu este explicat de throttling de background. La 500
de operații, p99 pentru schimbarea activității este 222,598 ms control, 267,429
ms mare și 276,443 ms densitate. Retenția PSS după operații și 30 s de settle
este 206.055 KiB control, 337.241 KiB mare și 259.900 KiB densitate.

## Separarea costului Rust de UI

| Profil | ProjectModel open | Open nativ total | Preview prepare | Zola render |
| --- | ---: | ---: | ---: | ---: |
| control | 141 ms | 934,807 ms | 829 ms | 216 ms |
| mare | 540 ms | 3.341,899 ms | 3.106 ms | 473 ms |
| densitate | 504 ms | 3.683,935 ms | 3.506 ms | 455 ms |

Lock wait p95 este 0 ms în toate cele trei profile; lock held rămâne sub 0,257
ms. Costul dominant al open-ului nativ este pregătirea Preview, nu contendența
lock-urilor. Diferența până la Canvas cold arată un al doilea cost major în
startup-ul Tauri/WebKit, montarea UI și publicarea Canvas.

Kernel p95 pentru editarea HTML este 33,351 ms control, 153,223 ms mare și
329,536 ms densitate. Full rebuild p95 este 100,042 / 527,725 / 458,640 ms, iar
CSS edit p95 este 2,521 / 15,498 / 19,199 ms. Reconcilierea externă rămâne sub
6,8 ms p95. Zola build p95 rămâne sub 295 ms pentru toate profilele utilizabile.

## Limite și defecte confirmate

1. `margine-disk` este acceptat corect de manifest la 991 fișiere, dar Files
   publică exact limita de 500 și niciuna dintre cele 10 lansări nu ajunge la
   workspace, Canvas sau refuz explicit în 180 s. Limita practică și contractul
   UX nu sunt aliniate cu limita manifestului de 1.000. Defectul s-a reprodus în
   30 din 30 de lansări agregate din două suite `standard` și suita `soak`.
2. Profilul de 1.001 este refuzat fail-closed în 257,178 ms p95, fără workspace
   parțial. Acest contract trece în kernel și UI.
3. Contractul Canvas real se oprește la drag/drop: `CanvasAgent did not project
   the blocked Rust move verdict`. Cele 100 de proiecții ale selecției au p95 14
   ms și p99 18 ms, dar scenariile ulterioare de editare și Undo/Redo nu sunt
   atinse după defect.
4. Contorizarea DOM/CSS/media din iframe este `unavailable`, nu zero: WebKitGTK
   expune trei targeturi, dar niciun execution context inspectabil pentru iframe-
   ul Preview cross-origin. Latența rutelor este totuși măsurată: p95 729 ms
   control, 3.662 ms mare și 6.074 ms densitate.
5. Payload-ul IPC nu este estimat fără un hook autoritar. Raportul păstrează ca
   proxy numărul evenimentelor și bytes din jurnalul kernel, fără cifre fabricate.

## Build și dimensiuni

- frontend production: 91,563 s; 99 fișiere / 4.722.812 B în `build`;
- graful inițial român: 14 fișiere JS, 1.230.112 B raw / 317.123 B gzip;
- binar Tauri release: 250,822 s build, 133.069.896 B;
- buildul și compilarea sunt raportate separat și nu intră în latențele runtime.

## Soak

Suita `soak` a rulat 137,80 minute, între 02:06:01 și 04:23:49, și a produs
18.797 probe brute, 365 distribuții și 24 de verdicte de buget, dintre care 20
au eșuat. Cele 10 lansări la 991 au expirat toate, iar cele 10 lansări la 1.001
au fost refuzate corect. Control și densitate au produs câte 1.217 probe WebKit.

În acel run, adaptorul WebKit monolitic `mare` a eșuat la `reload-input:31` și a
pierdut probele WebKit anterioare din profil. Defectul instrumentului a fost
remediat ulterior prin streaming JSONL și loturi de maximum 10 cicluri. Smoke-ul
final și baseline-ul standard de mai sus au validat câte 228, respectiv 377
probe WebKit complete per profil. Soak-ul rămâne dovadă de stres pre-optimizări,
dar nu este promovat ca baseline comparabil; următorul soak trebuie executat cu
runnerul final și comparat numai cu un baseline `soak` compatibil.

```bash
npm run performance:soak
```

## Utilizare ulterioară

După fiecare lot de optimizări, se rulează din nou suita standard și se compară
raportul candidat cu acest baseline:

```bash
cargo run --release --manifest-path tools/performance-benchmark/Cargo.toml -- \
  report --raw RUN/samples.jsonl --json RUN/report.json \
  --markdown RUN/report.md \
  --baseline docs/performance-baseline-standard-v1.json
```

O regresie este semnalată numai pentru distribuții cu minimum 10 probe, peste
10% plus marja de zgomot de 2,5–5 puncte procentuale. Identitatea suitei,
fixture-urilor, hardware-ului și toolchain-ului trebuie să coincidă. Bugetele
aspiraționale rămân independente de acest verdict.
