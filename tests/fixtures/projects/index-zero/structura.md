# Structura site — INDEX ZERO

> Arhitectură aprobată la 21.08.2026.

## Tip proiect

Website Zola multi-page, editorial și fixture determinist de stres.

## Pagini și colecții

| # | Pagină | URL | Scop |
| --- | --- | --- | --- |
| 1 | Acasă | `/` | Identitate, repere, program selectat și intrări spre colecții |
| 2 | Program | `/program/` | Toate evenimentele grupate și filtrabile |
| 3 | Evenimente | `/evenimente/` | Arhivă și taxonomii pentru 180 de evenimente |
| 4 | Eveniment | `/evenimente/[slug]/` | Detalii, oră, spațiu, artiști și evenimente asociate |
| 5 | Artiști | `/artisti/` | Index pentru 100 de artiști |
| 6 | Artist | `/artisti/[slug]/` | Biografie, disciplină și apariții în program |
| 7 | Spații | `/locatii/` | Index pentru 20 de spații |
| 8 | Spațiu | `/locatii/[slug]/` | Adresă fictivă, accesibilitate și program local |
| 9 | Jurnal | `/jurnal/` | Arhivă editorială cu 50 de articole |
| 10 | Articol | `/jurnal/[slug]/` | Conținut Markdown lung, imagini și citate |
| 11 | Galerie | `/galerie/` | Grilă media și lightbox accesibil |
| 12 | Despre | `/despre/` | Manifest, echipă fictivă și metodă |
| 13 | Bilete | `/bilete/` | Flux demonstrativ fără plată |
| 14 | Contact | `/contact/` | Formular demonstrativ și date fictive |
| 15 | Întrebări | `/intrebari-frecvente/` | Accordion accesibil |
| 16 | Laboratoare | `/laboratoare/` | Indexul testelor deliberate |
| 17 | Densitate DOM | `/laboratoare/densitate/` | Aproximativ 5k noduri în profilul mare |
| 18 | Motion | `/laboratoare/motion/` | Animații concurente și reduced motion |
| 19 | CSS | `/laboratoare/css/` | Mii de reguli utilizate și controale vizuale |
| 20 | Media | `/laboratoare/media/` | Decodare eager, rasterizare și memorie |
| 21 | Legale | `/legal/termeni/`, `/legal/confidentialitate/`, `/legal/cookies/`, `/legal/anpc/` | Modele adaptate proiectului fictiv |
| 22 | 404 | `/404.html` | Recuperare și întoarcere în program |

## Homepage — secțiuni

### 1. Header

- wordmark text „INDEX ZERO”;
- Program, Evenimente, Artiști, Spații, Jurnal;
- CTA „Vezi programul”;
- meniu mobil controlat prin JS și ARIA.

### 2. Hero

- **H1:** „INDEX ZERO”
- dată fictivă: 7–16 octombrie 2027;
- CTA „Deschide programul” și „Explorează artiștii”;
- fotografie industrială originală și compoziție tipografică frontală;
- dată supradimensionată și metadate monospace.

### 3. Semnal

- manifest scurt despre intersecția dintre corp, cod și spațiu;
- trei coordonate tematice: observă, intervine, transmite.

### 4. Program selectat

- 6 evenimente recomandate pe homepage și 180 în program;
- alternanță între carduri dense și accente cromatice;
- legături către programul complet.

### 5. Cifre

- 10 zile, 180 evenimente, 100 artiști, 20 spații;
- contoare tipografice fără animație JS costisitoare.

### 6. Artiști

- selecție de 8 artiști;
- grilă fragmentată, hover și focus cu tratament halftone.

### 7. Hartă conceptuală

- hartă abstractă deterministă pentru 20 de spații;
- puncte interactive accesibile și listă echivalentă.

### 8. Jurnal

- 3 articole pe homepage și 50 în arhivă;
- variație mare de lungime a titlurilor pentru stres tipografic.

### 9. CTA final

- „Intră în program înainte ca orașul să se rescrie.”
- legături către program și laborator.

### 10. Footer

- navigație, date fictive, linkuri legale;
- declarație explicită de proiect de test;
- badge ANPC SAL pentru simularea unui site comercial;
- creditul local standard.

## Taxonomii

- `discipline`: performance, sunet, instalație, imagine, cercetare, atelier;
- `zile`: ziua-01 până la ziua-10;
- `formate`: live, expoziție, discuție, atelier, intervenție urbană;
- `subiecte`: corp, memorie, infrastructură, ecologie, algoritmi, comunitate.

## Note de performanță

- listele mari sunt rute reale, nu markup duplicat fără sens;
- pagina program este intenționat densă;
- laboratoarele izolează câte o dimensiune pentru diagnostic;
- profilul `mare` rămâne sub 500 de intrări publicate;
- profilele de 991 și 1.001+ fișiere sunt generate separat;
- toate numerele așteptate sunt stocate într-un manifest verificabil.

## Aprobare

- [x] Structură aprobată de Gabriel la 21.08.2026
