# CSS funcțional pentru blocurile native

Blocurile native nu livrează design implicit. Registrul Rust poate declara numai
`functional_scss`: reguli fără de care semantica sau interacțiunea blocului nu
poate fi reprezentată corect.

Contractul exclude culori de temă, tipografie, borduri decorative, umbre,
radiusuri, spațiere estetică și tranziții. Nu există variabile globale
`--pana-block-*` și nici o legătură cu frameworkul CSS al proiectului.

Counter, Accordion, Tabs, Slider, Nav Menu și Icon funcționează fără stylesheet
generat. Dialog și Offcanvas păstrează numai geometria necesară overlay-ului,
limitarea overflow-ului și o suprafață neutră bazată pe culorile de sistem CSS.

Reconcilierea rămâne per pagină. Un template cu CSS funcțional primește blocul
administrat în `sass/pagini/<pagina>.scss`; eliminarea ultimei utilizări îl
șterge din acel fișier. Runtime-ul JS este deduplicat și reconciliat separat tot
per pagină.

Runtime-ul JS are un plan canonic produs în Rust din template, registrul nativ
și `PageJsConfig`. Fișierul `static/js/pana-<pagina>.js` conține nucleul comun o
singură dată numai dacă pagina folosește cel puțin un bloc JS, apoi câte un
provider pentru fiecare tip prezent, în ordinea registrului. Două instanțe de
Accordion nu dublează providerul; Accordion plus Slider livrează exact doi
provideri; ștergerea ultimei instanțe elimină providerul. O pagină cu numai
Motion nu primește deloc runtime de blocuri, iar Icon nu solicită JavaScript.

Interactive Preview execută același `pana-<pagina>.js` ca pagina publică.
Injectorul Preview adaugă numai protocolul său de observare și nu mai are o
copie ori un fallback monolitic pentru blocuri. Runtime-ul de blocuri citește
exclusiv lista `blocks`; configurația Motion nu îi este transmisă și nu este
expusă printr-o variabilă globală intermediară.

Singurele forme canonice sunt `data-pana-block`, metadatele `@pana-block` și
marcajele SCSS `pana:block`. Contractele istorice „component” nu sunt citite și
nu au o cale alternativă de execuție. `.panastudio` nu participă la contractul
blocurilor.
