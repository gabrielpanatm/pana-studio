# INDEX ZERO — Context de proiect

> Sursa unică de adevăr pentru proiectul Zola de stres folosit la validarea Pană Studio.

## Proiect

- **Client:** INDEX ZERO / proiect cultural fictiv de test
- **URL canonic:** `https://index-zero.invalid`
- **Tip:** website editorial Zola multi-page și fixture de performanță
- **Temă:** exclusiv dark
- **Limbă:** română
- **Dosar:** `tests/fixtures/projects/index-zero`
- **Caracter juridic:** toate entitățile, persoanele, evenimentele și datele comerciale sunt fictive și există numai pentru testare

## Obiectiv principal

Proiectul trebuie să fie simultan un website coerent, credibil și un fixture determinist care împinge Pană Studio aproape de limitele sale de fișiere, DOM, CSS, animații, conținut și media. Nicio optimizare a aplicației nu se implementează în acest proiect; el rămâne referința stabilă pentru măsurători.

## Audiență fictivă

Public urban 20–45 ani interesat de arte performative, instalații, sunet, design și tehnologie; artiști, curatori, studenți și vizitatori ai Timișoarei.

## Stack

- **Framework:** Zola embedded compatibil 0.22.1
- **Markup:** HTML semantic + Tera + Markdown
- **Stiluri:** SCSS propriu compilat de Zola, fără nesting
- **JavaScript:** vanilla local, fără librării externe
- **Fonturi locale:** Oswald Variable + Atkinson Hyperlegible Next Variable + Geist Mono Variable
- **Media:** WebP local, SVG inline numai pentru elemente grafice deterministe
- **Deploy:** nu se publică; `base_url` folosește domeniul rezervat `.invalid`
- **Analytics:** fără
- **Generator:** Rust, determinist, fără dependențe runtime externe

## Profile de stres

### `control`

Set redus pentru verificări rapide ale buildului și ale contractelor.

### `mare`

Profilul canonic, complet utilizabil în Pană Studio:

- 180 evenimente;
- 100 artiști;
- 20 de spații;
- 50 de articole;
- aproximativ 430–470 intrări fișier/director publicabile în File Explorer;
- mai puțin de 1.000 de fișiere în manifestul canonic;
- mai puțin de 24 MiB text și 2 MiB per fișier;
- aproximativ 380 de rute și sub 250.000 de noduri Canvas agregate.

### `densitate`

Adaugă laboratoare deliberate cu 1.000, 2.000, 5.000 și 10.000 de noduri literale într-un template, plus matrice CSS folosită integral.

### `margine-disk`

Produce exact 991 de fișiere urmărite pentru limita manifestului și trebuie să rămână integral utilizabil în File Explorer și în namespace-ul sursă rezident.

### `peste-limita`

Produce minimum 1.001 fișiere urmărite și trebuie să fie refuzat fail-closed. Profilul există numai pentru testarea mesajului și a siguranței.

## Direcție vizuală aprobată

- fundal grafit aproape negru, alb osos, portocaliu-semnal și verde acid;
- estetică editorială elvețiană disruptivă, densitate controlată, grile tehnice și tipografie condensată supradimensionată;
- texturi halftone și grain, chenare fine, marcaje de coordonate și contoare monospace;
- colțuri în general drepte; fără carduri SaaS generice și fără abuz de pills;
- motion orchestrat, nu decor aleator; toate efectele respectă `prefers-reduced-motion`.

## Arhitectură de conținut

- colecțiile reale stau în `sursa/content/`: `evenimente`, `artisti`, `locatii`, `jurnal`;
- layout-urile colecțiilor stau în `sursa/templates/`;
- datele globale și programul reutilizabil stau în `sursa/date/`;
- homepage-ul vizual stă în `templates/index.html`, iar `content/_index.md` păstrează numai metadatele;
- laboratoarele de stres sunt rute explicite sub `/laborator/`, etichetate clar ca instrumente de test;
- fișierele generate sunt deținute exclusiv de generator și pot fi regenerate numai după validarea markerului de proprietate.

## Reguli obligatorii

- denumiri tehnice în română, fără diacritice;
- diacritice obligatorii în textele afișate;
- zero CSS inline și zero JS inline;
- zero nesting CSS/SCSS;
- zero librării CSS sau JS externe;
- toate imaginile au dimensiuni explicite și texte alternative;
- fiecare script este încărcat numai unde este folosit;
- sursele generate trebuie să fie deterministe pentru același profil și seed;
- generatorul nu șterge niciun director fără markerul său de proprietate;
- proiectul mare rămâne baseline: schimbările de conținut sau structură se documentează în `readme.md`.

## Decizii

| Data | Decizie | Motiv |
| --- | --- | --- |
| 22.08.2026 | Fixture canonic integrat în `tests/fixtures/projects/index-zero` | Centralizează proiectele complete de test și le versiunează împreună cu benchmarkul |
| 21.08.2026 | Concept cultural INDEX ZERO | Susține natural sute de pagini, taxonomii, galerii, program și motion |
| 21.08.2026 | Profil mare sub 500 de intrări publicate | Păstrează un reper intermediar stabil, separat de profilul de limită |
| 21.08.2026 | Profile disk separate | Limita de 1.000 de fișiere poate fi testată fără a compromite proiectul canonic |
| 21.08.2026 | Generator Rust determinist | Respectă arhitectura Rust-first și permite benchmark-uri reproductibile |
| 21.08.2026 | Temă dark editorială | Direcție vizuală aprobată și potrivită conținutului experimental |

## Status

- [x] Brief aprobat
- [x] Structură aprobată (`structura.md`)
- [x] Direcție vizuală aprobată (`inspiratie/`)
- [x] Conținut principal și reguli de generare finalizate (`resurse/text/`)
- [ ] Generator Rust implementat
- [ ] Setup Zola complet
- [ ] Profil `control` validat
- [ ] Profil `mare` validat
- [ ] Profilele de limită validate
- [ ] Review vizual și responsive
- [ ] Validare în Pană Studio

## Pașii următori

1. Implementarea unui generator Rust determinist și a manifestului de așteptări.
2. Construirea sursei Zola și a sistemului vizual.
3. Integrarea resurselor ImageGen și rularea profilelor de validare.
