# Blocul Icon — contract Rust-first

## Autoritate și traseu

`icon` este un block nativ `Static` cu `BlockScale::Element`. Registrul, validarea,
randarea SVG, inserarea și mutația persistentă sunt deținute de Rust. Frontendul
primește numai pagini limitate pentru picker și intenții tipizate; nu furnizează
markup SVG către insert engine.

Traseul de inserare este:

`NativeBlockRegistry/InsertCatalog (Rust) → drag tipizat → Insert Engine (Rust) →
ProjectWorkspace → CanvasPatch → reconciliere Zola`.

Editarea specifică este disponibilă numai în `BlockPropertiesPane`, prin
`IconBlockPropertiesEditor`. `HtmlPane` nu conține picker sau controale pentru
identitatea iconului. Stilizarea rămâne în CSS Pane.

## Sursă și atomicitate

Sursa persistentă este un `<svg>` inline cu rădăcină canonică:

- `data-pana-block="icon"` și `data-pana-instance`;
- `data-pana-icon="tabler-outline:<id>"`;
- `viewBox="0 0 24 24"`, `currentColor`, dimensiune și stroke validate;
- contract de accesibilitate exclusiv: decorativ (`aria-hidden`) sau semantic
  (`role="img"` și `aria-label`).

Source Graph și preview proiectează numai rădăcina. Geometria administrată
`path` nu devine nod editabil, astfel că un click pe geometrie se rezolvă la
instanța rădăcină. Mutația `SetIcon` înlocuiește identitatea, atributele
administrate și geometria într-o singură operație cu rollback; clasele, stilul,
`data-anim`, instanța și proveniența rămân neatinse.

## Proveniență și reproducibilitate

Fișierul `src-tauri/resources/icon-packs/tabler-outline-3.41.1.json` este generat
determinist de `scripts/generate-icon-registry.mjs` din:

- `@tabler/icons` versiunea exactă `3.41.1` din `package-lock.json`;
- `icons.json` pentru categorii/taguri;
- `tabler-nodes-outline.json` pentru geometrie.

Comenzile sunt `npm run icons:generate` și `npm run icons:check`. Verificarea
refuză diferențe față de lockfile, ID-uri necanonice, mai mult de 32 de noduri,
taguri altele decât `path`, path data peste 8 KiB, atribute necunoscute,
scripturi, event handlers, URL-uri și valori nepermise. Rust repetă validarea la
încărcarea registrului compilat, iar bridge-ul validează din nou `SetIcon`.

Tabler Icons este distribuit sub MIT. Intrarea și textul licenței pentru
`@tabler/icons 3.41.1` există în
`src-tauri/resources/licenses/THIRD_PARTY_LICENSES.txt`.

## Dimensiune și transport

Măsurat la generare pentru 5.039 iconuri:

- registru JSON: **1.924.017 bytes**;
- același registru comprimat gzip: **359.434 bytes**;
- același registru comprimat xz: **263.664 bytes**.

Registrul este inclus o singură dată în binarul Rust prin `include_str!` și nu
este listat separat în `tauri.conf.json > bundle.resources`; astfel se evită o
copie suplimentară în AppImage. Estimarea xz este măsura relevantă pentru
contribuția compresibilă la pachet, iar dimensiunea finală a AppImage este
verificată separat la buildul de release.

IPC nu transportă întregul registru. `read_icon_catalog` trimite doar rezumatul,
iar `search_icon_catalog` limitează query-ul la 128 bytes și pagina la maximum
96 de rezultate (pickerul cere 48), cu debounce și anulare logică latest-wins.
