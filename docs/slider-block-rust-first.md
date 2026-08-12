# Slider/Carousel Rust-first

Slider este un bloc nativ `Js`, clasificat `BlockScale::Composition`, cu rădăcină HTML `div`. Nu este o secțiune: poate fi inserat într-o secțiune sau într-un container compatibil. Un eventual Hero Slider va fi un wrapper de tip Section peste această compoziție, nu o schimbare a contractului de bază.

## Autoritate și structură

`NativeBlockRegistry` definește markupul, stilurile, opțiunile și slotul `slides`. Rust randează atât instanța inițială, cât și fiecare slide adăugat; frontendul transmite doar intenția tipizată `nativeBlockSlotItem`, fără HTML. `UiBlockGraphSnapshot` publică starea slotului, ancorele Source Graph și limitele 1–32.

Operațiile Insert, Duplicate și Delete folosesc executorii structurali existenți și patch-urile lor Canvas reversibile. Move trece prin Editor Move, păstrând planul/commitul atomic. Toate cele patru operații includ providerul, slotul, rădăcina și `expectedModelRevision`; validatorul Rust verifică apartenența, limitele, revizia și interzice Slider în Slider. Scaffold-ul administrat nu poate fi mutat prin acțiunile HTML generice. Conținutul obișnuit dintr-un slide rămâne editabil.

Editorul structural există exclusiv în `BlockPropertiesPane`: listă ordonată, selectare, adăugare, duplicare, mutare și ștergere. `HtmlPane` nu deține controale Slider.

## Runtime și accesibilitate

Runtime-ul nativ afișează un singur slide, oferă Previous/Next, indicatori, Home/End și săgeți. Loop, slide-ul inițial, autoplay, intervalul și politicile de pauză sunt opțiuni Rust validate. Autoplay este oprit implicit; când este activ există un control Start/Stop explicit. Rotația se oprește la focus, conform opțiunilor de hover/interacțiune, la `document.hidden` și pentru `prefers-reduced-motion`.

Rădăcina folosește `role="group"`, `aria-roledescription="carousel"` și o etichetă editabilă. Slide-urile folosesc grupuri etichetate „X din Y”. `aria-live` este `polite` când rotația nu rulează și `off` în timpul rotației.

Runtime-ul calculează separat semnătura opțiunilor și semnătura structurală. Inserarea, mutarea, duplicarea ori ștergerea unui item remontează instanța și curăță listener-ele vechi înainte să atașeze unele noi.

## Extensibilitate

Contractul de slot conține `itemKind`, minimum și maximum și este independent de Slider. Accordion și Tabs își publică deja aceeași metadată; pot primi ulterior inspectori structurali fără a schimba protocolul. V1 nu include fade, image-only carousel sau multiple slides per view.
