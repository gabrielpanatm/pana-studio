# Protocol benchmark Pană Studio v2

Protocolul v2 păstrează fixture-urile, suitele, straturile, persistența și
identitatea de mediu definite în `protocol-v1.md`, dar corectează semantic
măsurarea schimbării documentelor. Rapoartele v1 și v2 nu sunt comparabile.

## Settlement document autoritar

Adaptorul WebKit nu mai consideră două cadre RAF drept dovadă că documentul
este gata. Pentru fiecare activare așteaptă simultan:

- tabul Rust-confirmat selectat;
- calea activă și identitatea documentului țintă;
- finalizarea încărcării sursei;
- settlement-ul frontend `ready` al aceleiași cereri latest-wins.

Sunt publicate separat `input_to_tab_selected` și
`input_to_document_ready`, împreună cu direcția, suprafața și rezultatul
cache-ului Template Workbench.

## Scenarii document

1. `code_to_code` alternează două surse code fără cost Canvas necesar.
2. `canonical_template_reactivation` revine din code la același template deja
   publicat și separă reutilizarea canonică de materializare.
3. `rapid_document_alternation` trimite un burst alternant și verifică faptul
   că numai ultima intenție ajunge la settlement.

Bugetele aspiraționale sunt p95 ≤ 100 ms pentru activarea tabului și pentru un
document code gata, respectiv p95 ≤ 500 ms pentru reactivarea canonică și
settlement-ul unui burst latest-wins. Un render necesar este etichetat
`materialized` și nu este amestecat cu probele `reused`.
