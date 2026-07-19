# Auditul panoului CSS

Data auditului: 17 iulie 2026

## Contractul canonic

Panoul CSS are o singură autoritate de scriere:

1. controlul produce un draft vizual;
2. Inspectorul grupează ultimele valori pentru aceeași combinație proiect/sesiune/fișier/selector/viewport;
3. la încheierea interacțiunii sau la bariera Save/History/Snapshot, Inspectorul apelează comenzile CSS/SCSS Rust;
4. Rust citește și modifică exclusiv documentele ProjectWorkspace/FileBufferStore;
5. receipt-ul CSS schema 2 publică revizia, tranzacția, fișierele scrise și snapshoturile `FileBufferTextSnapshot` exacte;
6. CodeMirror, cache-ul sursei și cursorul de sincronizare FileBuffer sunt rebazate din aceste snapshoturi;
7. Preview-ul este proiectat pentru tranzacția și revizia confirmate, iar stratul optimist este eliminat numai dacă identitatea sa încă este curentă.

Frontendul nu mai conține un writer CSS paralel. `source-sync.ts` rămâne doar o proiecție read-only pentru sursa deschisă.

## Inventarul suprafeței

| Secțiune | Proprietăți / funcții auditate | Observații după refactorizare |
|---|---|---|
| Typography | familie, dimensiune, line-height, weight, align, spacing, style, transform, decoration | toggle-ul activ emite intenție de ștergere; Rust elimină declarația sau regula goală |
| Colors | text, background color/image/gradient, size, repeat, position, attachment, background blend, clip | `background-blend-mode` înlocuiește folosirea incorectă a `mix-blend-mode`; gradientele nereprezentabile intră în mod brut |
| Spacing | padding/margin shorthand și laturi, gap unificat/separat, white-space, overflow unificat/X/Y | shorthand-urile concurente rămân vizibile și pot fi eliminate; valorile implicite au opțiune explicită de revenire la „implicit” |
| Layout | display, flex direction/grow/shrink/wrap, justify/align, grid columns/rows | controalele segmentate comit valori atomice sau ștergere, niciodată `property: ;` |
| Position | position, offsets, z-index | eliminarea `position` nu șterge implicit offset-urile; ele rămân date CSS independente |
| Size | width/height/min/max, ratio, object, scroll, scrollbar, touch | expresiile arbitrare sunt drafturi până la încheierea editării și sunt validate structural în Rust |
| Border | shorthand, width/style/color, radius shorthand/colțuri, outline shorthand/width/style/color | shorthand-urile existente nu mai sunt ascunse de editorii longhand |
| Shadow | liste box/text, culoare, inset, dimensiuni, adăugare/ștergere | parser cu virgule/paranteze/quotes; sintaxa neacoperită este păstrată brut, nu convertită la valori implicite |
| Transform | transition, transform, origin/style/backface și preseturi | presetul este adăugat atomic; textul liber este validat structural înainte de commit |
| Effects | opacity, mix blend, clip-path, filter/backdrop-filter, mask image/size/repeat/position | `mix-blend-mode` are un singur proprietar semantic aici; asset-urile folosesc URL public de proiect |
| Variables | grupare, culoare, text, breakpoint-uri | nu se mai scrie la fiecare caracter; valoarea goală este respinsă înainte de mutație; migrarea media query rulează o singură dată pe commit |

## Defecte confirmate și eliminate

- Writerul TypeScript `upsertCssPropertyInSource` transforma ștergerea într-o declarație SCSS invalidă (`text-align: ;`). A fost eliminat.
- Fișierul deschis în CodeMirror ocolea contractul pentru stylesheet-ul de pagină și putea omite legarea sa în template. Acum folosește aceeași comandă Rust ca fișierul închis.
- Receipt-ul CSS nu conținea textul și revizia FileBuffer ca o singură dovadă. Schema 2 conține proiecții exacte, validate înainte de actualizarea UI.
- Evenimentul derivat ProjectWorkspace putea dubla proiecția exactă și eroarea Zola. O proiecție exactă consumă timerul derivat pentru aceeași revizie.
- Fiecare caracter producea mutație, recovery și randare Zola. Drafturile sunt grupate și golite la finalul interacțiunii și la toate barierele globale.
- Variabilele SCSS puteau ajunge temporar la `$nume: ;`; Rust respinge valoarea goală.
- Valorile cu paranteze, quotes sau delimitatori incompleți puteau intra în ProjectWorkspace. Validatorul Rust refuză expresia incompletă și separatorii de declarații la nivel superior.
- Scannerul Rust confunda acoladele din strings/comentarii cu blocuri și nu recunoștea selectori grupați. Scannerul canonic ignoră trivia/quotes și actualizează regula grupată existentă.
- Ștergerea ultimei declarații dintr-un stylesheet de pagină putea crea sau păstra un contract CSS gol. Comanda elimină stylesheet-ul și linkul dacă nu mai există reguli efective.
- Editorul de umbre putea înlocui culori necunoscute cu negru și fragmenta `calc(...)`; editorul de gradient putea înlocui stopuri necunoscute. Valorile nereprezentabile sunt acum editate brut.
- Selecturile cu fallback (`normal`, `visible`, `repeat`, `scroll`, `border-box`) nu distingeau moștenirea de o declarație explicită. Opțiunea goală reprezintă acum ștergerea.

## Limite intenționate

- Panoul editează reguli top-level și reguli top-level din media query-urile desktop/tablet/mobile. Nesting-ul SCSS rămâne editabil în CodeMirror, dar nu este rescris implicit de panou.
- Câmpurile text acceptă expresii CSS/SCSS arbitrare. Validatorul garantează integritatea structurală, nu încearcă să reimplementeze compilatorul Sass sau gramatica semantică a fiecărei proprietăți.
- Eliminarea unei proprietăți condiționale (`display`, `position`) nu elimină automat proprietățile dependente. Aceasta evită pierderea implicită a datelor și păstrează comportamentul cascadei CSS.
- Sintaxa complexă a gradientelor și umbrelor pe care editorul structurat nu o poate proiecta fără pierderi este afișată ca valoare brută.

## Matrice de regresie

- setare, înlocuire și ștergere pe desktop/tablet/mobile;
- ștergerea ultimei declarații și eliminarea regulii;
- selector de bază, pseudo-clasă, selector custom și selector grupat;
- fișier CSS/SCSS deschis versus închis în CodeMirror;
- stylesheet existent versus stylesheet de pagină creat și legat automat;
- expresii cu `calc`, variabile SCSS, quotes, URL-uri, funcții cu virgule și valori incomplete;
- umbre multiple și fallback brut; gradient structurat și fallback brut;
- receipt străin, snapshot inconsistent și revizie/transactionId neconforme;
- coalescing pentru tastare, flush la Save/Undo/Redo și deduplicarea proiecției Preview;
- Undo/Redo peste documentele modificate de panou, cu rebazarea exactă a CodeMirror/FileBuffer.
