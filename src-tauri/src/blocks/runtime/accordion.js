(function () {
  "use strict";
  /* PANA BLOCK PROVIDER: accordion */
  var runtime = window.PanaBlockRuntime;
  if (!runtime) throw new Error("PanaBlockRuntime core lipsește pentru providerul accordion.");

  function instanceToken(root, fallback) {
    return root.getAttribute("data-pana-instance") || fallback;
  }

  runtime.register("accordion", {
    mount: function (root, makeBag) {
      var bag = makeBag();
      var allowMultiple = root.getAttribute("data-multiple") === "true";
      var token = instanceToken(root, "accordion");
      var items = Array.prototype.slice.call(root.querySelectorAll("[data-pana-accordion-item]"));
      function setOpen(item, trigger, panel, open) {
        trigger.setAttribute("aria-expanded", open ? "true" : "false");
        panel.hidden = !open;
        if (open) item.setAttribute("data-open", "");
        else item.removeAttribute("data-open");
      }
      items.forEach(function (item, index) {
        var trigger = item.querySelector("[data-pana-accordion-trigger]");
        var panel = item.querySelector("[data-pana-accordion-panel]");
        if (!trigger || !panel) return;
        trigger.id = trigger.id || token + "-trigger-" + index;
        panel.id = panel.id || token + "-panel-" + index;
        if (trigger.localName === "button" && !trigger.getAttribute("type")) trigger.setAttribute("type", "button");
        trigger.setAttribute("aria-controls", panel.id);
        panel.setAttribute("role", "region");
        panel.setAttribute("aria-labelledby", trigger.id);
        setOpen(item, trigger, panel, trigger.getAttribute("aria-expanded") === "true" || item.hasAttribute("data-open"));
        bag.listen(trigger, "click", function () {
          var shouldOpen = trigger.getAttribute("aria-expanded") !== "true";
          if (shouldOpen && !allowMultiple) {
            items.forEach(function (other) {
              if (other === item) return;
              var otherTrigger = other.querySelector("[data-pana-accordion-trigger]");
              var otherPanel = other.querySelector("[data-pana-accordion-panel]");
              if (otherTrigger && otherPanel) setOpen(other, otherTrigger, otherPanel, false);
            });
          }
          setOpen(item, trigger, panel, shouldOpen);
        });
      });
      return bag.dispose;
    }
  });
})();
