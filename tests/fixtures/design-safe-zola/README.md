# Design Safe organic fixture

Acest proiect Zola conține intenționat JavaScript ostil pentru un WebView:
buclă sincronă infinită, furtună de microtask-uri, worker și handler-e inline.
Nu deschide fixture-ul prin **Run extern** decât într-un browser/tab care poate
fi închis forțat. Proba Pană Studio trebuie făcută în Preview embedded, unde
politica Design Safe elimină aceste suprafețe înainte de execuție.
