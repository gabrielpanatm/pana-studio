# Pachete de teme Zola în Pană Studio

## Scop și autoritate

Pană Studio distribuie teme Zola ca resurse imuabile ale aplicației. Catalogul, validarea,
planul de impact și mutațiile proiectului aparțin nucleului Rust. Frontendul afișează exclusiv
snapshoturile Rust și trimite intențiile `install` sau `activate`; nu enumeră și nu citește
direct filesystemul.

Versiunea Zola integrată și testată de această implementare este `0.22.1`.

Sursele oficiale care definesc semantica păstrată de Pană Studio:

- [Themes overview](https://www.getzola.org/documentation/themes/overview/)
- [Creating a theme](https://www.getzola.org/documentation/themes/creating-a-theme/)
- [Installing and using themes](https://www.getzola.org/documentation/themes/installing-and-using-themes/)
- [Overriding a theme](https://www.getzola.org/documentation/themes/extending-a-theme/)
- [Sass in Zola](https://www.getzola.org/documentation/content/sass/)

## Layout bundled

Fiecare pachet este local aplicației:

```text
src-tauri/resources/theme-packs/{id}/
├── pana-theme.toml
├── preview.webp
├── theme/
│   ├── theme.toml
│   ├── templates/
│   ├── sass/
│   └── static/
└── recipe/
    ├── content/
    ├── data/
    ├── templates/
    ├── sass/
    └── static/
```

`theme/` este tema Zola propriu-zisă și se instalează în `themes/{id}/`. `recipe/` este
materialul inițial al unui proiect nou: conținut, date și, când designul o cere, surse locale
demonstrative. Rețeta nu este aplicată automat peste un proiect existent.

`theme.toml` rămâne manifestul oficial Zola. `pana-theme.toml` este contractul Pană Studio:

```toml
schema_version = 1
id = "id-stabil"
display_name = "Nume vizibil"
summary = "Descriere scurtă"
version = "1.0.0"
category = "starter"
preview = "preview.webp"
capabilities = ["responsive-layout"]
required_pages = ["content/_index.md"]
required_data = ["data/catalog.toml"]
editor_anchors = ["templates/base.html"]

[zola]
minimum = "0.22.0"
tested = "0.22.1"
```

ID-ul folosește numai litere ASCII mici, cifre și `-`. Căile sunt relative canonice, fără
segmente goale, absolute sau `..`. Cerințele declarate trebuie să existe în `recipe/`, iar
ancorele trebuie să existe în `theme/`.

## Validarea registrului

`ThemeRegistry` enumeră numai `resources/theme-packs`. El refuză fail-closed:

- schema necunoscută, TOML invalid și metadate contradictorii;
- ID-uri duplicate sau ID diferit de director;
- teme incompatibile cu Zola embedded;
- path traversal, symlinkuri și intrări care nu sunt fișiere/directoare regulate;
- rooturi sau extensii nepermise;
- mai mult de 16 pachete, 512 fișiere per pachet sau 64 MiB per pachet;
- manifest peste 64 KiB și preview peste 5 MiB;
- preview fără semnătură RIFF/WEBP;
- cerințe și ancore care nu sunt livrate de pachet.

Frontendul primește starea `available`, `installed` sau `active`, compatibilitatea, capabilitățile,
numărul de fișiere și override-urile locale. Aceste valori nu sunt recalculate în TypeScript.

## Instalare și activare

Citirea, planificarea și aplicarea sunt comenzi distincte.

1. Planul cere `expectedProjectRoot`, `expectedSessionId` și `expectedRevision`.
2. Rust enumeră toate destinațiile, conflictele, cerințele lipsă și template-urile locale care
   vor masca tema.
3. Planul primește un token SHA-256. Apply recalculează planul pe aceeași identitate și refuză
   tokenul sau revizia stale.
4. `install` copiază pachetul în overlay-ul ProjectWorkspace fără activare și fără overwrite.
5. `activate` modifică numai cheia top-level `theme` din configurația Zola cu `toml_edit`;
   comentariile, secțiunile și ordinea celorlalte valori rămân intacte.
6. Candidatul complet trece prin contractele Pană, SourceGraph și Zola embedded.
7. Recovery este persistat înaintea publicării live. Operația devine exact o intrare Undo.

Tema activă precedentă nu este ștearsă. Template-urile locale păstrează prioritatea oficială
Zola și sunt raportate ca impact, nu modificate.

## Proiect nou

`pana-basic` conține numai `.gitignore` și configurația Zola neutră. Inițializarea aleasă în UI:

1. validează pachetul înainte de orice scriere;
2. publică starterul neutru;
3. publică `theme/` în `themes/{id}/`;
4. publică `recipe/` în rootul Zola;
5. activează tema lossless în configurație;
6. rulează Zola embedded;
7. publică succesul sau retrage toate fișierele prin rollback WriteAuthority.

Nu există un ID de temă implicit în inițializator sau în frontend.

## Temele vizuale incluse

Catalogul bundled include starterul tehnic `pana-studio` și trei teme complete:

- `nord` — servicii profesionale și B2B, cu structură modulară, contrast precis și colecție
  de servicii;
- `cadru` — studio creativ și portofoliu, cu direcție editorială întunecată și colecție de
  proiecte;
- `radacini` — ospitalitate, wellness și afaceri locale, cu direcție organică și colecție de
  camere.

Fiecare temă livrează rețetă de conținut, date TOML, fonturi locale cu licențele lor,
imagine hero WebP, meniu accesibil și preview generat din randarea Zola reală.

## Adăugarea unei teme viitoare

Pentru fiecare design:

1. creează un ID stabil și un director nou în `resources/theme-packs`;
2. păstrează tot designul reutilizabil în `theme/`, iar conținutul demonstrativ în `recipe/`;
3. scrie manifestele oficial și Pană fără a modifica registrul Rust;
4. exportă o previzualizare WebP landscape, sub 5 MiB;
5. declară numai capabilități reale, cerințe livrate și ancore existente;
6. rulează testele registrului, inițializarea într-un dosar gol, instalarea inactivă,
   activarea, override-urile, Undo/Redo, salvarea și redeschiderea.

Importul extern, marketplace-ul, actualizarea automată și ștergerea temelor sunt intenționat în
afara acestui contract.
