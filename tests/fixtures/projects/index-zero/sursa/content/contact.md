+++
title = "Contact demonstrativ"
description = "Un punct de contact fictiv și un formular care nu transmite date."
template = "page.html"
[extra]
eticheta = "CONTACT / DOMENIU INVALID"
+++

## Date fictive

**INDEX ZERO**

Strada Semnalului 0, Timișoara

`salut@index-zero.invalid`
`+40 000 000 000`

Niciuna dintre aceste date nu aparține unei organizații reale. Domeniul `.invalid` nu poate primi e-mail.

<form class="formular-demo" data-demo-form>
  <label for="nume-contact">Nume pentru test</label>
  <input id="nume-contact" name="nume" autocomplete="name" required>
  <label for="email-contact">E-mail</label>
  <input id="email-contact" name="email" type="email" autocomplete="email" required>
  <label for="mesaj-contact">Mesaj demonstrativ</label>
  <textarea id="mesaj-contact" name="mesaj" rows="7" required></textarea>
  <label class="bifa"><input type="checkbox" required> Înțeleg că mesajul nu va fi transmis.</label>
  <button class="buton buton-primar" type="submit">Simulează trimiterea</button>
  <p class="mesaj-formular" role="status" data-form-status></p>
</form>
