(() => {
  const buton = document.querySelector('.buton-meniu');
  const navigatie = document.querySelector('#navigatie-principala');
  const eticheta = buton?.querySelector('.sr-only');

  if (!(buton instanceof HTMLButtonElement) || !(navigatie instanceof HTMLElement)) {
    return;
  }

  const inchideMeniul = () => {
    buton.setAttribute('aria-expanded', 'false');
    navigatie.classList.remove('este-deschisa');
    if (eticheta instanceof HTMLElement) {
      eticheta.textContent = 'Deschide meniul';
    }
  };

  buton.addEventListener('click', () => {
    const esteDeschis = buton.getAttribute('aria-expanded') === 'true';
    buton.setAttribute('aria-expanded', String(!esteDeschis));
    navigatie.classList.toggle('este-deschisa', !esteDeschis);
    if (eticheta instanceof HTMLElement) {
      eticheta.textContent = esteDeschis ? 'Deschide meniul' : 'Închide meniul';
    }
  });

  navigatie.addEventListener('click', (eveniment) => {
    if (eveniment.target instanceof HTMLAnchorElement) {
      inchideMeniul();
    }
  });

  window.addEventListener('resize', () => {
    if (window.innerWidth > 768) {
      inchideMeniul();
    }
  });

  document.addEventListener('keydown', (eveniment) => {
    if (eveniment.key === 'Escape' && buton.getAttribute('aria-expanded') === 'true') {
      inchideMeniul();
      buton.focus();
    }
  });
})();
