(function () {
  "use strict";
  /* PANA BLOCK PROVIDER: offcanvas */
  var runtime = window.PanaBlockRuntime;
  if (!runtime) throw new Error("PanaBlockRuntime core lipsește pentru providerul offcanvas.");

  function instanceToken(root, fallback) {
    return root.getAttribute("data-pana-instance") || fallback;
  }

  function overlayDefinition(kind) {
    var prefix = "data-pana-" + kind;
    return {
      mount: function (root, makeBag) {
        var bag = makeBag();
        var token = instanceToken(root, kind);
        var openers = Array.prototype.slice.call(root.querySelectorAll("[" + prefix + "-open]"));
        var closers = Array.prototype.slice.call(root.querySelectorAll("[" + prefix + "-close]"));
        var overlay = root.querySelector("[" + prefix + "-overlay]");
        var panel = root.querySelector("[" + prefix + "-panel]");
        var title = root.querySelector("[" + prefix + "-title]");
        var previousActive = null;
        var previousOverflow = "";
        var openFrame = 0;
        var closeOnBackdrop = root.getAttribute("data-close-outside") !== "false";
        var closeOnEscape = root.getAttribute("data-close-escape") !== "false";
        if (!overlay || !panel) return bag.dispose;
        panel.id = panel.id || token + "-panel";
        panel.setAttribute("role", "dialog");
        panel.setAttribute("aria-modal", "true");
        if (!panel.getAttribute("tabindex")) panel.setAttribute("tabindex", "-1");
        if (title) {
          title.id = title.id || token + "-title";
          panel.setAttribute("aria-labelledby", title.id);
        }
        openers.forEach(function (opener) {
          if (opener.localName === "button" && !opener.getAttribute("type")) opener.setAttribute("type", "button");
          opener.setAttribute("aria-haspopup", "dialog");
          opener.setAttribute("aria-controls", panel.id);
        });
        closers.forEach(function (closer) {
          if (closer.localName === "button" && !closer.getAttribute("type")) closer.setAttribute("type", "button");
        });
        function expanded(open) {
          openers.forEach(function (opener) { opener.setAttribute("aria-expanded", open ? "true" : "false"); });
        }
        function show(opener) {
          previousActive = document.activeElement;
          previousOverflow = document.body.style.overflow || "";
          overlay.hidden = false;
          document.body.style.overflow = "hidden";
          expanded(true);
          openFrame = window.requestAnimationFrame(function () {
            root.setAttribute("data-open", "");
            var focus = panel.querySelector("button:not([disabled]),[href],input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex='-1'])") || panel;
            if (focus && focus.focus) focus.focus();
          });
        }
        function hide(restoreFocus) {
          if (openFrame) window.cancelAnimationFrame(openFrame);
          root.removeAttribute("data-open");
          document.body.style.overflow = previousOverflow;
          expanded(false);
          overlay.hidden = true;
          if (restoreFocus !== false && previousActive && previousActive.focus && document.contains(previousActive)) {
            previousActive.focus();
          }
        }
        openers.forEach(function (opener) { bag.listen(opener, "click", function () { show(opener); }); });
        closers.forEach(function (closer) { bag.listen(closer, "click", function () { hide(true); }); });
        bag.listen(overlay, "click", function (event) {
          if (closeOnBackdrop && event.target === overlay) hide(true);
        });
        bag.listen(overlay, "keydown", function (event) {
          if (closeOnEscape && event.key === "Escape") hide(true);
        });
        bag.add(function () {
          if (openFrame) window.cancelAnimationFrame(openFrame);
          if (root.hasAttribute("data-open")) document.body.style.overflow = previousOverflow;
        });
        expanded(!overlay.hidden);
        return bag.dispose;
      }
    };
  }

  runtime.register("offcanvas", overlayDefinition("offcanvas"));
})();
