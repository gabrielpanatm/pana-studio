# INDEX ZERO

## Prezentare generală

INDEX ZERO este un website Zola editorial fictiv și proiectul canonic de stres pentru Pană Studio. Combină sute de pagini, conținut românesc, SCSS complex, animații, media și laboratoare izolate pentru DOM, CSS, motion și disk.

## Locație

`tests/fixtures/projects/index-zero`

Proiectul este versiunea canonică internă folosită de benchmark. Runnerul lucrează
numai pe copii temporare și nu deschide această sursă direct în Pană Studio.

## Stack

- Zola embedded compatibil 0.23.4;
- Tera + Markdown;
- SCSS propriu, fără nesting;
- JavaScript vanilla local;
- generator Rust determinist;
- fonturi și media locale.

## Structură

```text
index-zero/
├── AGENTS.md
├── brief.md
├── structura.md
├── inspiratie/
├── materiale/
├── resurse/
├── instrumente/
├── sursa/
└── export/
```

## Profile

- `control` — verificări rapide;
- `mare` — proiect complet, sub plafonul practic de 500 de intrări;
- `densitate` — template-uri și CSS dense;
- `margine-disk` — 991 de fișiere urmărite;
- `peste-limita` — minimum 1.001 fișiere, refuz fail-closed așteptat.

## Comenzi

Toate comenzile se rulează din rădăcina proiectului:

```bash
CARGO_TARGET_DIR=/tmp/index-zero-generator-target \
  cargo run --quiet --manifest-path instrumente/generator-stres/Cargo.toml -- genereaza mare

CARGO_TARGET_DIR=/tmp/index-zero-generator-target \
  cargo run --quiet --manifest-path instrumente/generator-stres/Cargo.toml -- verifica mare

zola --root sursa check
zola --root sursa build --output-dir /tmp/index-zero-build --force
```

Generatorul poate șterge numai directoarele declarate în `instrumente/generator-stres/src/fisiere.rs` și refuză regenerarea dacă markerul `.index-zero-generator.json` nu confirmă proprietatea.

## Status

Pasul 2 este implementat și validat la 21.08.2026. Profilul canonic `mare` conține 180 evenimente, 100 artiști, 20 spații, 50 articole, 1.250 celule DOM, 1.200 reguli CSS și 240 elemente motion. Rezultatele detaliate sunt în `validare-pas-2.md`.

## Planșa vizuală

`inspiratie/directie-index-zero.png` a fost generată cu ImageGen built-in și este reperul aprobat pentru sistemul editorial dark.

Cele opt fotografii editoriale finale sunt în `sursa/static/imagini/vizual-01.webp` — `vizual-08.webp`. Prompturile și sursele originale sunt inventariate în `resurse/prompts-imagegen.md`.

## Siguranță

Toate persoanele, organizațiile, adresele, evenimentele și datele comerciale sunt fictive. Formularele și biletele nu transmit date și nu procesează plăți.

## Note de arhivare

Proiectul trebuie păstrat stabil după validare. Schimbările structurale sau de volum se documentează aici și în manifestul generatorului pentru a menține comparabilitatea benchmark-urilor.
