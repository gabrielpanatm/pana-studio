# Audit codebase — 2026-08-20

## Inventar verificat

Aplicația are 469 fișiere Rust / 265.195 linii, 254 fișiere TypeScript /
58.605 linii, 108 componente Svelte / 49.293 linii și 22 fișiere JavaScript /
6.248 linii. Numerele includ testele Rust colocate și codul generat. Worktree-ul
conține deja refactorizarea amplă AppState și separarea comenzii Rust `project`;
auditul tratează această stare necomisă ca baseline și nu reintroduce căile
șterse.

Baseline-ul este sănătos: zero cicluri frontend, zero module frontend orfane,
zero erori TypeScript/Svelte, 113 teste de contract frontend și 1.642 teste Rust
trecute. `cargo fmt`, `cargo check` și buildul de producție trec.

## Lotul 1 aplicat

Navigatorul proiectului, Editor/Canvas, Inspectorul și Activity Rail erau
dependențe statice ale ecranului de pornire, deși nu se montează până la
deschiderea proiectului sau a setărilor. Ele sunt acum o frontieră dinamică
unică, încărcată înainte de montarea workspace-ului și păstrată stabilă pe toată
sesiunea. Nu există o implementare paralelă sau un fallback legacy.

Rezultat build pentru graful inițial `ro`:

| Metrică | Înainte | După | Schimbare |
| --- | ---: | ---: | ---: |
| JavaScript raw | 1.490.367 B | 1.243.732 B | −16,5% |
| JavaScript gzip | 387.481 B | 318.949 B | −17,7% |
| Application shell raw | 204.364 B | 169.017 B | −17,3% |

Bugetele sunt coborâte la `<1.350.000 B` raw și `<350.000 B` gzip, iar testul
de arhitectură interzice reintroducerea importurilor statice. În Rust,
`AppState`, handle-ul MCP și accesul la runtime sunt limitate la crate; cele
patru avertismente inițiale și expunerea accidentală au dispărut.

## Lotul 2 aplicat

Gateway-ul `project/io` a fost eliminat complet. Cele 122 de apeluri Tauri au
fost distribuite către modulele IO ale domeniilor care le dețin, iar
responsabilitățile strict project sunt împărțite în module înguste pentru
lifecycle, startup, workspace, configuration, external disk și Zola. Nu există
barrel sau reexporturi de compatibilitate.

Din suprafața publică veche au fost eliminate 14 funcții fără consumatori și 8
reexporturi de tip inutile. Validatorul de receipt folosit între mai multe
domenii a devenit helper intern explicit. Testele contractuale verifică acum
proprietarii reali, iar o gardă nouă interzice reapariția vechii căi,
barrel-urile și readucerea modulelor exclusiv lazy în graful inițial.

Rezultat build pentru graful inițial `ro`:

| Metrică | Înainte | După | Schimbare |
| --- | ---: | ---: | ---: |
| JavaScript raw | 1.243.732 B | 1.236.327 B | −7.405 B (−0,60%) |
| JavaScript gzip | 318.949 B | 316.865 B | −2.084 B (−0,65%) |

Buildul produce 64 de chunk-uri JavaScript; cel mai mare are 490.954 B, iar
`pana-core-domain` are 168.500 B. `test:kernel` trece cu 114 teste, iar
TypeScript, Svelte check, ciclurile, reachability și buildul sunt verzi.

## Hotspot-uri rămase

1. `src/lib/types.ts` (7.820 linii): catalog global de contracte. Se separă pe
   domeniile Rust, iar consumatorii importă direct contractul deținut; nu se
   păstrează un barrel global compatibil.
2. Rust: `write_authority/capability.rs` (9.186), `editor_navigation.rs`
   (3.120 linii înaintea testelor), `selection_coordinator.rs` (2.795 înaintea
   testelor), `preview/canvas.rs` (2.629 înaintea testelor) și
   `preview/engine.rs` (1.775 înaintea testelor). Se fragmentează după model,
   planificare, execuție și proiecție, păstrând o singură autoritate publică.
3. Performanță preview: rămân de măsurat tranzacția staged-tree pentru proiecția
   surselor și CanvasGraph per rută/lazy, identificate de auditul anterior ca
   principalele costuri după optimizarea ProjectModel.

Ordinea următoare recomandată este contractele din `types.ts` →
`preview/engine.rs`; fiecare lot trebuie să reducă un owner mare,
să elimine vechea cale în aceeași schimbare și să păstreze bugete măsurabile.
