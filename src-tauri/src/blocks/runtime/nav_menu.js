(function () {
  "use strict";
  /* PANA BLOCK PROVIDER: nav-menu */
  var runtime = window.PanaBlockRuntime;
  if (!runtime) throw new Error("PanaBlockRuntime core lipsește pentru providerul nav-menu.");

  function instanceToken(root, fallback) {
    return root.getAttribute("data-pana-instance") || fallback;
  }

  runtime.register("nav-menu", {
    mount: function (root, makeBag) {
      var bag = makeBag();
      var token = instanceToken(root, "nav-menu");
      var toggle = root.querySelector("[data-pana-nav-menu-toggle]");
      var list = root.querySelector("[data-pana-nav-menu-list]");
      if (!toggle || !list) return bag.dispose;
      var media = window.matchMedia ? window.matchMedia("(max-width: 720px)") : null;
      var closeOnSelect = root.getAttribute("data-close-on-select") !== "false";
      list.id = list.id || token + "-list";
      toggle.setAttribute("aria-controls", list.id);
      if (toggle.localName === "button" && !toggle.getAttribute("type")) toggle.setAttribute("type", "button");
      function compact() { return media ? media.matches : false; }
      function setOpen(open) {
        toggle.hidden = !compact();
        if (open) root.setAttribute("data-open", "");
        else root.removeAttribute("data-open");
        toggle.setAttribute("aria-expanded", open ? "true" : "false");
        list.hidden = compact() ? !open : false;
      }
      bag.listen(toggle, "click", function () { setOpen(!root.hasAttribute("data-open")); });
      bag.listen(root, "keydown", function (event) {
        if (event.key !== "Escape" || !root.hasAttribute("data-open")) return;
        setOpen(false);
        if (toggle.focus) toggle.focus();
      });
      Array.prototype.forEach.call(list.querySelectorAll("a[href]"), function (link) {
        bag.listen(link, "click", function () {
          if (closeOnSelect && compact()) setOpen(false);
        });
      });
      if (media) {
        var sync = function () { setOpen(root.hasAttribute("data-open")); };
        if (media.addEventListener) bag.listen(media, "change", sync);
        else if (media.addListener) {
          media.addListener(sync);
          bag.add(function () { media.removeListener(sync); });
        }
      }
      setOpen(root.hasAttribute("data-open"));
      return bag.dispose;
    }
  });
})();
