# Validare pas 2 — INDEX ZERO

**Data:** 21.08.2026

**Profil canonic:** `mare`
**Zola:** 0.23.4

## Volum generat

| Suprafață | Rezultat |
| --- | ---: |
| Evenimente | 180 |
| Artiști | 100 |
| Spații | 20 |
| Articole | 50 |
| Celule laborator DOM | 1.250 |
| Noduri element laborator DOM în browser | 5.072 |
| Reguli stylesheet generat | 1.200 |
| Elemente motion | 240 |
| Imagini media eager | 90 |
| Resurse SVG media distincte | 24 |
| Fișiere sursă urmărite | 464 |
| Directoare sursă urmărite | 31 |
| Total intrări urmărite | 495 / 500 |
| Total surse text | 922.190 bytes |
| Cel mai mare fișier text | 252.085 bytes |
| Fișiere în build-ul Zola | 421 |
| Dimensiune build | 6,4 MiB |

## Validări

- generator Rust compilat și formatat;
- `zola check`: succes;
- `zola build`: succes;
- homepage desktop: CSS și fonturi locale încărcate, hero WebP 1536 px;
- mobil 390 × 844: o singură coloană și fără overflow orizontal;
- meniu: `aria-expanded` și vizibilitate sincronizate;
- program: 180 intrări, 1.882 noduri element; filtrul „Ziua 7” afișează 18 și ascunde 162;
- laborator CSS: 288 eșantioane vizibile, stylesheet cu 1.200 reguli;
- laborator motion: 240 elemente și control de pauză funcțional;
- laborator media: 90/90 imagini încărcate, 24 URL-uri distincte;
- galerie: 8/8 imagini și lightbox funcțional pe cadrul selectat;
- ruta inexistentă: template `404.html` randat corect;
- consolă browser: zero avertismente și zero erori.

## Limite de siguranță

- maximum fișier text: sub 2 MiB;
- total surse text: sub 24 MiB;
- profilul mare: sub 500 intrări urmărite, inclusiv directoare;
- marker de proprietate obligatoriu înainte de regenerare;
- directoarele de build și cache sunt excluse din inventar.

Profile validate în această execuție:

- `margine-disk`: exact 991 fișiere, succes;
- `peste-limita`: exact 1.001 fișiere, succes ca fixture negativ;
- `mare`: restaurat la 464 fișiere + 31 directoare = 495 intrări.

Proiectul rămâne obligatoriu în profilul `mare` după orice verificare de prag.
