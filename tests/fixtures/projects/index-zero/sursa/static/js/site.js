(() => {
  const selector = (valoare, radacina = document) => radacina.querySelector(valoare);
  const selectori = (valoare, radacina = document) => [...radacina.querySelectorAll(valoare)];

  const butonMeniu = selector('[data-menu-toggle]');
  const meniu = selector('[data-menu]');

  if (butonMeniu && meniu) {
    butonMeniu.addEventListener('click', () => {
      const deschis = butonMeniu.getAttribute('aria-expanded') === 'true';
      butonMeniu.setAttribute('aria-expanded', String(!deschis));
      meniu.dataset.deschis = String(!deschis);
    });

    document.addEventListener('keydown', (eveniment) => {
      if (eveniment.key === 'Escape') {
        butonMeniu.setAttribute('aria-expanded', 'false');
        meniu.dataset.deschis = 'false';
        butonMeniu.focus();
      }
    });
  }

  const filtre = selectori('[data-filter]');
  const elementeFiltrabile = selectori('[data-filter-item]');

  filtre.forEach((buton) => {
    buton.addEventListener('click', () => {
      const filtru = buton.dataset.filter;
      filtre.forEach((element) => element.classList.toggle('activ', element === buton));
      elementeFiltrabile.forEach((element) => {
        element.hidden = filtru !== 'toate' && element.dataset.filterItem !== filtru;
      });
    });
  });

  const zile = selectori('[data-day]');
  const intrariProgram = selectori('[data-program-day]');

  zile.forEach((buton) => {
    buton.addEventListener('click', () => {
      const zi = buton.dataset.day;
      zile.forEach((element) => element.classList.toggle('activ', element === buton));
      intrariProgram.forEach((element) => {
        element.hidden = zi !== 'toate' && element.dataset.programDay !== zi;
      });
    });
  });

  const butonMotion = selector('[data-motion-toggle]');

  if (butonMotion) {
    butonMotion.addEventListener('click', () => {
      const oprit = document.body.classList.toggle('motion-oprit');
      butonMotion.textContent = oprit ? 'Pornește animațiile' : 'Oprește animațiile';
    });
  }

  const formular = selector('[data-demo-form]');

  if (formular) {
    formular.addEventListener('submit', (eveniment) => {
      eveniment.preventDefault();
      const status = selector('[data-form-status]', formular);
      status.textContent = 'Simulare finalizată local. Nicio informație nu a fost transmisă.';
    });
  }

  const dialogGalerie = selector('[data-gallery-dialog]');
  const imagineGalerie = selector('[data-gallery-image]');
  const legendaGalerie = selector('[data-gallery-caption]');

  if (dialogGalerie && imagineGalerie && legendaGalerie) {
    selectori('[data-gallery-open]').forEach((buton) => {
      buton.addEventListener('click', () => {
        const indice = buton.dataset.galleryOpen;
        imagineGalerie.src = '/imagini/vizual-0' + indice + '.webp';
        imagineGalerie.alt = 'Cadru editorial INDEX ZERO ' + indice;
        legendaGalerie.textContent = 'Cadru editorial INDEX ZERO ' + indice;
        dialogGalerie.showModal();
      });
    });

    selector('[data-gallery-close]', dialogGalerie).addEventListener('click', () => dialogGalerie.close());
    dialogGalerie.addEventListener('click', (eveniment) => {
      if (eveniment.target === dialogGalerie) {
        dialogGalerie.close();
      }
    });
  }
})();
