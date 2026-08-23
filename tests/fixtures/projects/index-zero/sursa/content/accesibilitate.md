+++
title = "Accesibilitate"
description = "Obiectivele și limitele de accesibilitate ale proiectului INDEX ZERO."
template = "page.html"
[extra]
eticheta = "ACCES / WCAG"
+++

## Obiectiv

Interfața urmărește nivelul WCAG 2.2 AA pentru navigare, contrast, structură semantică și operare de la tastatură. Fiind un proiect de stres, unele laboratoare produc intenționat volume neobișnuit de mari; ele sunt etichetate clar și separate de traseul editorial principal.

## Tastatură și focus

Ordinea de tab urmează ordinea vizuală. Linkul „Sari la conținut” devine vizibil la focus, meniul raportează starea prin `aria-expanded`, iar filtrele sunt butoane reale. Focusul nu este eliminat prin CSS.

## Mișcare

Animațiile folosesc transformări și opacitate și sunt dezactivate automat când sistemul solicită reducerea mișcării. Laboratorul motion include și un control explicit de oprire.

## Media și contrast

Textele nu sunt suprapuse direct peste imaginile cele mai aglomerate. Paleta principală folosește os, grafit, portocaliu și verde acid; dimensiunile și greutățile tipografice compensează condensarea fontului display.

## Limită cunoscută

Harta spațiilor este o reprezentare abstractă, nu un instrument de orientare real. Aceeași informație este disponibilă în lista semantică alăturată.
