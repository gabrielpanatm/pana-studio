# Checklist manual — panoul HTML

Acest checklist validează panoul HTML împotriva sursei canonice din
`ProjectWorkspace`. Pentru fiecare scenariu, pornește de la un proiect de test
nou și păstrează deschis alternativ Preview-ul și fișierul în CodeMirror.

## 1. Valori goale și eliminare explicită

1. Selectează un `<img>` și golește `alt`, după ce acesta avea o valoare.
2. Confirmă prin blur.
3. Verifică în CodeMirror că atributul există încă drept `alt=""` (sau în stilul
   de quoting original), nu că a dispărut.
4. Adaugă `data-state` cu valoare goală și verifică faptul că rămâne prezent.
5. Pentru `dir`, alege `nesetat` și verifică faptul că atributul este eliminat.
6. Focalizează un câmp gol care nu exista și ieși fără să tastezi: nu trebuie
   creat un atribut gol accidental.

## 2. Boolean-presence, ARIA și enumerated

1. Pe un `<input>`, activează `required`; sursa trebuie să conțină atributul,
   inclusiv forma minimizată/goală validă.
2. Dezactivează `required`; atributul trebuie eliminat complet.
3. Pentru `aria-hidden`, testează distinct `true`, `false` și `nesetat`.
   `false` trebuie păstrat explicit, nu interpretat ca absență.
4. Pentru `contenteditable` și `draggable`, verifică `true`, `false` și
   valoarea implicită (atribut absent).
5. Introdu o valoare numerică invalidă în `tabindex`, `rows` sau `width`;
   eroarea trebuie afișată lângă control, iar sursa canonică nu trebuie schimbată.

## 3. Round-trip al sintaxei atributelor

În CodeMirror pregătește un tag cu atribute bare, unquoted, single-quoted și
double-quoted, de exemplu:

```html
<input disabled data-state=ready title='Titlu' aria-label="Câmp">
```

Editează pe rând fiecare atribut din panou. Confirmă că:

- nu apar duplicate;
- eliminarea atributului bare nu lasă reziduuri;
- quote-ul original este păstrat când este sigur;
- apostrofurile și ghilimelele sunt escaped corect;
- restul tagului rămâne neschimbat bit-cu-bit.

## 4. Design Safe versus sursa canonică

1. Pe un link, setează `target="_blank"` și activează `download`.
2. Panoul trebuie să indice „doar sursă”. CodeMirror trebuie actualizat, iar
   Design Safe Preview trebuie să neutralizeze navigarea/descărcarea.
3. Pe un formular, modifică `action`; rezultatul trebuie salvat în sursă, fără
   a permite submit în Preview.
4. Verifică faptul că `onclick`, `srcdoc` și URL-uri `javascript:` sunt blocate
   de kernel, chiar dacă payloadul nu provine din controlul normal.
5. Un `iframe` existent rămâne în sursă, dar este descris drept inert/eliminat
   din Design Safe Preview.

## 5. Schimbarea tagului

1. Schimbă `section` în `article`; Preview, Structură și CodeMirror trebuie să
   ajungă la aceeași revizie.
2. Confirmă că destinațiile void (`img`, `input`, `source`) nu sunt oferite.
3. Confirmă că tranzițiile incompatibile (`ul` → `section`, `a` → `button`) și
   destinațiile eliminate din Preview (`iframe`) nu sunt oferite și sunt
   refuzate și de kernel dacă sunt cerute direct.
4. Rulează Undo și Redo; ambele taguri, selecția și CodeMirror trebuie să
   urmărească revizia canonică.

## 6. Pickere media, curse și anulare

1. Pe imaginea A tastează o cale nouă, apoi selectează imediat imaginea B
   înainte de expirarea debounce-ului.
2. Confirmă că imaginea B nu primește niciodată valoarea destinată imaginii A.
3. Pe imaginea A începe o editare și apasă `Escape`; valoarea inițială revine,
   draftul live dispare, iar Save/Undo nu primesc o mutație canonică nouă.
4. Repetă pentru `video src` și `audio src`.
5. Alege un asset din listă și apoi schimbă rapid selecția; commit-ul trebuie să
   rămână legat de ținta capturată.

## 7. ACK live, latest-wins și recovery

1. Tastează rapid mai multe valori în același atribut.
2. Ultima valoare trebuie să fie cea afișată și cea salvată; un ACK vechi nu
   trebuie să rescrie statusul sau draftul nou.
3. Oprește/reîncarcă Preview în timpul editării. Eșecul proiecției speculative
   trebuie raportat, dar commit-ul canonic prin ProjectWorkspace poate continua.
4. După proiecția canonică, draftul live trebuie închis cu ACK și statusul să
   confirme ProjectWorkspace.

## 8. Save, Undo/Redo, CodeMirror și Tera

1. Combină într-o singură sesiune: text, atribut gol, boolean, clasă și tag.
2. Verifică Save, apoi seria completă Undo/Redo fără pierderea selecției.
3. Deschide CodeMirror înainte și după fiecare operație; conținutul trebuie să
   reflecte imediat revizia ProjectWorkspace, nu un buffer paralel.
4. Repetă pe un element provenit dintr-un template/partial Tera editabil.
5. Pentru o zonă Tera fără ancoră stabilă, panoul trebuie să rămână blocat cu o
   cauză explicită, fără fallback legacy și fără scriere directă pe disk.
6. Salvează, închide și redeschide proiectul; rezultatul trebuie să fie identic
   cu ultima revizie confirmată.
