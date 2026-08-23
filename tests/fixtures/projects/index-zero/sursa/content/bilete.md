+++
title = "Rezervare demonstrativă"
description = "Un flux local pentru verificarea formularului, fără tranzacții sau stocare."
template = "page.html"
[extra]
eticheta = "DEMO / FĂRĂ PLATĂ"
+++

> Acesta este un proiect fictiv. Formularul nu transmite date și nu poate crea o rezervare reală.

<form class="formular-demo" data-demo-form>
  <label for="nume">Nume pentru test</label>
  <input id="nume" name="nume" autocomplete="name" required>
  <label for="email">E-mail</label>
  <input id="email" name="email" type="email" autocomplete="email" required>
  <label for="categorie">Categorie demonstrativă</label>
  <select id="categorie" name="categorie">
    <option>Acces general</option>
    <option>Atelier</option>
    <option>Tur accesibil</option>
  </select>
  <label class="bifa"><input type="checkbox" required> Înțeleg că aceasta este doar o simulare locală.</label>
  <button class="buton buton-primar" type="submit">Simulează rezervarea</button>
  <p class="mesaj-formular" role="status" data-form-status></p>
</form>
