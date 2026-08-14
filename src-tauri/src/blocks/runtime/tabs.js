(function () {
  "use strict";
  /* PANA BLOCK PROVIDER: tabs */
  var runtime = window.PanaBlockRuntime;
  if (!runtime) throw new Error("PanaBlockRuntime core lipsește pentru providerul tabs.");

  function instanceToken(root, fallback) {
    return root.getAttribute("data-pana-instance") || fallback;
  }

  runtime.register("tabs", {
    mount: function (root, makeBag) {
      var bag = makeBag();
      var token = instanceToken(root, "tabs");
      var tabs = Array.prototype.slice.call(root.querySelectorAll("[data-pana-tabs-tab]"));
      var panels = Array.prototype.slice.call(root.querySelectorAll("[data-pana-tabs-panel]"));
      if (!tabs.length || !panels.length) return bag.dispose;
      function activate(index, focus) {
        tabs.forEach(function (tab, tabIndex) {
          var active = tabIndex === index;
          tab.setAttribute("aria-selected", active ? "true" : "false");
          tab.setAttribute("tabindex", active ? "0" : "-1");
          if (active && focus && tab.focus) tab.focus();
        });
        panels.forEach(function (panel, panelIndex) { panel.hidden = panelIndex !== index; });
      }
      var selected = parseInt(root.getAttribute("data-default-tab") || "0", 10);
      if (!isFinite(selected) || selected < 0 || selected >= Math.min(tabs.length, panels.length)) {
        selected = 0;
      }
      tabs.forEach(function (tab, index) {
        var panel = panels[index];
        if (!panel) return;
        tab.id = tab.id || token + "-tab-" + index;
        panel.id = panel.id || token + "-panel-" + index;
        if (tab.localName === "button" && !tab.getAttribute("type")) tab.setAttribute("type", "button");
        tab.setAttribute("role", "tab");
        tab.setAttribute("aria-controls", panel.id);
        panel.setAttribute("role", "tabpanel");
        panel.setAttribute("aria-labelledby", tab.id);
        bag.listen(tab, "click", function () { activate(index, false); });
        bag.listen(tab, "keydown", function (event) {
          if (["ArrowRight", "ArrowLeft", "Home", "End"].indexOf(event.key) < 0) return;
          event.preventDefault();
          var next = event.key === "Home" ? 0
            : event.key === "End" ? tabs.length - 1
              : event.key === "ArrowRight" ? (index + 1) % tabs.length
                : (index - 1 + tabs.length) % tabs.length;
          activate(next, true);
        });
      });
      activate(selected, false);
      return bag.dispose;
    }
  });
})();
