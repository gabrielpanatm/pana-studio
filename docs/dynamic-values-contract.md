# Contractul Dynamic Value

## Scop

`Câmp dinamic` este un singur widget generic. Utilizatorul îl inserează din **Adaugă element**, apoi alege în inspector contextul, valoarea și prezentarea. Interfața nu construiește expresii Tera și nu ghicește structura proiectului.

Selectorul de valori este căutabil și grupat semantic. Expresia Tera canonică, calculată de Rust, este doar informativă în secțiunea **Avansat**; nu este un al doilea câmp editabil și nu poate concura cu bindingul tipizat.

Rust deține contractul, validează selecția și generează sursa Tera. Fișierul din proiect rămâne sursa reală; Canvas-ul și inspectorul sunt proiecții ale acelei surse.

## Catalogul autoritativ

`SourceGraph.dynamicWidgetGraph.valueCatalog` este construit de Rust și conține definiții tipizate pentru:

- documentul Zola (`page` sau articolul curent): titlu, descriere, date, slug, cale, permalink, rezumat, conținut, limbă, greutate, număr de cuvinte și timp de citire;
- secțiunea curentă: titlu, descriere, cale, permalink și limbă;
- site/configurație: titlu, descriere, URL de bază și limbă implicită;
- câmpurile modelelor de conținut din `.panastudio/content-models`;
- valori descoperite în `config.extra` și în `extra` din frontmatter-ul secțiunilor.

Fiecare definiție declară sursa tipizată, contextele permise, tipul valorii, prezentările compatibile și valorile implicite pentru prezentare și eticheta HTML. Tipurile listă/obiect sunt catalogate, dar nu pot fi convertite implicit la text; ele cer un Listing sau un context repetor.

## Binding și prezentare

Bindingul persistat separă trei concepte:

1. contextul (`page`, `collectionItem`, `section`, `site`, `repeaterItem`, extensibil cu `taxonomyTerm`);
2. sursa (`builtin`, `customField`, `configExtra`, `sectionExtra`);
3. tipul valorii (`text`, `richHtml`, `date`, `number`, `boolean`, `url`, `image`, `listObject`).

Prezentarea este independentă de sursă și poate fi automată, text, titlu, paragraf, etichetă, dată, număr, monedă, procent, imagine, link, buton sau conținut HTML Zola. Rust validează combinația, eticheta HTML, precizia numerică și formatul, apoi generează Tera cu acces sigur inclusiv pentru chei care nu sunt identificatori (`page.extra["hero-title"]`).

`| safe` este permis numai pentru HTML-ul generat de Zola din `content` și `summary`. Valorile personalizate, prefixul, sufixul și fallback-ul nu primesc încredere HTML implicită.

## Valori absente

Absența unei chei opționale nu este o eroare de randare. Generatorul Rust testează existența expresiei înainte de interpolare și înainte de filtrele de dată sau număr:

- `renderEmpty` păstrează elementul semantic, dar fără valoare; o imagine absentă nu primește un `src` gol;
- `fallback` randează valoarea alternativă ca text sau atribut escapate, fără să aplice filtrele valorii originale asupra fallback-ului;
- `hide` nu emite elementul când valoarea lipsește ori este goală.

Acest contract se aplică uniform valorilor standard și câmpurilor personalizate, inclusiv în Listing Item-uri ale căror articole pot avea seturi diferite de chei `extra`.

## Listing Item

Într-un fișier administrat ca Listing Item:

- contextul este blocat la `collectionItem`;
- câmpurile personalizate sunt limitate la modelul declarat de Listing Item;
- valorile standard ale articolului, precum `item.title` și `item.permalink`, rămân disponibile;
- aceeași regulă este verificată în Rust la inserare, la actualizarea din inspector și la reconstruirea SourceGraph, nu doar în UI.

Listing-ul consumator randează colecția prin `get_section`, un singur `for item` și `include` pentru șablonul Listing Item.

## Persistență și migrare

Widgeturile sunt delimitate de markere Tera versionate. Schema curentă este `2`; markerele `1` sunt citite și migrate în memorie la noul binding. Prima actualizare rescrie instanța în schema curentă. O rescriere cere revizia exactă a sursei, a workspace-ului, a modelului și a Preview-ului, astfel încât o selecție veche nu poate suprascrie cod nou.

Inserările directe vechi rămân disponibile în kernel ca fallback de compatibilitate, dar nu mai sunt prezentate în fluxul normal al panoului.

## Proiecția în Canvas

Drop-ul produce mai întâi o singură mutație autoritativă Rust. Evenimentele identice aflate simultan în curs sunt deduplicate în adaptor. După commit, aplicația aplică patch-ul Canvas când este posibil și apoi reconciliază Preview-ul și SourceGraph cu revizia canonică. Dacă patch-ul local sau proiecția eșuează, mutația nu este repetată; interfața raportează resincronizarea și păstrează sursa Rust drept adevăr.

După reconcilierea reușită, instanța nouă este selectată prin `instanceId` și proveniența din EditorNavigationSnapshot. În **Straturi**, rădăcina nu apare ca un `span` anonim, ci cu eticheta semantică din catalog, de exemplu `Câmp dinamic · Titlu`. Un eșec temporar al Canvas-ului nu produce o a doua inserare și nu schimbă autoritatea sursei.

## Extindere

O sursă nouă trebuie adăugată întâi în enum-ul Rust și în constructorul catalogului, cu tip și contexte explicite. Abia apoi se adaugă rezolvarea Tera și prezentările compatibile. Inspectorul consumă catalogul și nu necesită o ramură specială pentru fiecare câmp nou.
