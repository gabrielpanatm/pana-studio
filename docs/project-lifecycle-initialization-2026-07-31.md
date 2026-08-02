# ProjectLifecycle Rust-first

Data: 2026-07-31

## Contract

`ProjectLifecycleRuntime` este autoritatea unică pentru deschiderea proiectului. Snapshot-ul public conține:

- `activeSession`: sesiunea deja publicată și readiness-ul ei;
- `transition`: `idle`, `inspecting`, `awaiting_recovery_decision`, `preparing` sau `committing`;
- `operationId`: identitatea anti-stale a deschiderii în curs;
- revizia, timestamp-ul tranziției și motivul ultimei schimbări.

Tranziția și sesiunea activă sunt intenționat separate. La A → B, A rămâne activ până când B trece verificarea de manifest la commit. Un eșec anterior commit-ului elimină numai contextul provizoriu.

Readiness-ul sesiunii active este deținut tot de Rust:

```text
initializing_frontend
        │ frontend ACK
        ▼
preparing_preview
        │ preview prepared
        ▼
awaiting_canvas
        │ canonicalVerified
        ▼
      ready

orice eșec post-commit ──► degraded { capability, diagnostic }
```

Frontendul nu poate produce `ready`; numai ACK-ul Canvas `canonicalVerified`, validat față de root, runtime session și workspace revision, face tranziția.

## Deschidere

```text
folder ales
   │
   ▼
Startup inspect ── candidate token + manifest privat din aceeași parcurgere
   │
   ▼
inspect_project_open
   ├─ root canonic
   ├─ consumă manifestul disk deja capturat
   ├─ root fingerprint
   └─ recovery assessment
          │
          ├─ decision_required ──► așteaptă decizia pentru același operationId/token
          │
          ▼
open_project
   ├─ consumă contextul inspecției; nu reinspectează candidatul
   ├─ construiește provizoriu FileBufferStore + recovery + ProjectWorkspace
   ├─ proiectează ProjectScan din manifest + config + Workbench + documentul inițial
   ├─ construiește o singură generație Zola/Preview prin autoritatea provizorie
   ├─ verifică o singură dată manifestul live la commit
   └─ publică împreună sesiunea, ProjectModel-ul, Preview-ul și Workbench-ul
          │
          ▼
ProjectOpenBootstrapReceipt ── o singură hidratare frontend
          │
          ▼
Preview ──► Canvas canonicalVerified ──► Ready
```

`open_project` cere obligatoriu `operationId` și `candidateToken`. Comanda veche separată pentru evaluarea recovery a fost eliminată, deci o deschidere nu poate ocoli inspecția autoritară.

## Receipt-ul de bootstrap

Receipt-ul unic conține:

- `ProjectScan` și manifestul acceptat;
- `ProjectWorkspaceSnapshot` și `FileBufferStoreSnapshot` după recovery;
- configurația locală a proiectului;
- `WorkbenchSnapshot` restaurat;
- documentul activ, sursa lui și ruta de preview cunoscută;
- fișierul CSS/SCSS inițial;
- snapshot-ul `ProjectLifecycle` de după commit.

Documentul activ este ales o singură dată de Rust din Workbench-ul restaurat. Frontendul proiectează exact `activeDocument` din receipt și nu mai aplică o preferință sau un fallback propriu.

SCSS și SourceGraph sunt proiecții secundare. Ele sunt încărcate după hidratare și nu blochează readiness. SourceGraph citește exclusiv `ProjectModel` publicat de Preview pentru aceeași workspace revision. Dacă modelul autoritar lipsește, capabilitatea SourceGraph devine `degraded`; inițializarea nu pornește un rebuild paralel.

Inspecția Startup produce inventarul, fingerprint-ul candidatului și manifestul acceptabil prin aceeași parcurgere sortată, apoi validează structural proiectul și sintaxa TOML fără să încarce un al doilea `Site`. `inspect_project_open` consumă acel manifest privat prin token și adaugă fingerprint-ul root/recovery, fără un nou crawl. Validarea canonică este chiar generația Preview pregătită înainte de commit. Prima comandă frontend de pornire a Preview-ului găsește astfel aceeași revizie deja activă și întoarce un cache hit, nu un al doilea build.

## Rollback și degradare

- Operation ID sau candidate token stale: refuz înainte de pregătire.
- Disk schimbat între inspecție și commit: refuz, sesiunea veche rămâne activă.
- Recovery schimbat: refuz înainte de publicare.
- Hidratarea frontend, Preview, Canvas sau SourceGraph eșuate după commit: proiectul rămâne deschis și capabilitatea exactă este `degraded`.
- Un Preview candidat eșuat înainte de commit este retras împreună cu spațiul lui privat; Preview-ul sesiunii vechi nu este șters sau oprit.
- Schimbarea rapidă A → B: inspecția B invalidează operation ID-ul A.

## Observabilitate

Evenimentele `projectLifecycleTransition` includ operation ID, root, session ID și duratele inspecției/pregătirii până la commit. Evenimentele Preview existente includ workspace revision, preview revision, tranzacția Canvas și momentul `canonicalVerified`.

## Fișiere principale

- `src-tauri/src/project/lifecycle.rs` — state machine și teste unitare;
- `src-tauri/src/commands/startup.rs` — inspecție, anulare și ACK frontend;
- `src-tauri/src/commands/project.rs` — pregătire, commit și bootstrap receipt;
- `src-tauri/src/commands/preview.rs` — readiness Preview/Canvas;
- `src/lib/state/project-controller.ts` — proiecția receipt-ului în UI;
- `src/routes/+page.svelte` — overlay și blocarea editării până la readiness.
