(function () {
  "use strict";
  /* PANA BLOCK PROVIDER: slider */
  var runtime = window.PanaBlockRuntime;
  if (!runtime) throw new Error("PanaBlockRuntime core lipsește pentru providerul slider.");

  runtime.register("slider", {
    mount: function (root, makeBag) {
      var bag = makeBag();
      var track = root.querySelector("[data-pana-slider-track]");
      var slides = Array.prototype.slice.call(root.querySelectorAll("[data-pana-slider-slide]"));
      var previous = root.querySelector("[data-pana-slider-previous]");
      var next = root.querySelector("[data-pana-slider-next]");
      var indicators = root.querySelector("[data-pana-slider-indicators]");
      var autoplayButton = root.querySelector("[data-pana-slider-autoplay]");
      if (!track || !slides.length || !previous || !next || !indicators || !autoplayButton) {
        return bag.dispose;
      }
      if (!root.hasAttribute("tabindex")) root.setAttribute("tabindex", "0");
      var loop = root.getAttribute("data-loop") !== "false";
      var autoplayConfigured = root.getAttribute("data-autoplay") === "true";
      var interval = parseInt(root.getAttribute("data-interval") || "5000", 10);
      var pauseOnHover = root.getAttribute("data-pause-hover") !== "false";
      var pauseOnFocus = root.getAttribute("data-pause-focus") !== "false";
      var pauseOnInteraction = root.getAttribute("data-pause-interaction") !== "false";
      var reducedMotion = window.matchMedia ? window.matchMedia("(prefers-reduced-motion: reduce)") : null;
      var hovered = false;
      var focused = false;
      var userPaused = false;
      var timer = 0;
      var index = parseInt(root.getAttribute("data-initial-slide") || "0", 10);
      if (!isFinite(interval) || interval < 1000) interval = 5000;
      if (!isFinite(index) || index < 0 || index >= slides.length) index = 0;

      indicators.textContent = "";
      var indicatorButtons = slides.map(function (_, slideIndex) {
        var button = document.createElement("button");
        button.type = "button";
        button.className = "slider__indicator";
        button.setAttribute("aria-label", "Slide " + (slideIndex + 1));
        bag.listen(button, "click", function () { manualGo(slideIndex); });
        indicators.appendChild(button);
        return button;
      });

      function rotationAllowed() {
        return autoplayConfigured && slides.length > 1 && !userPaused &&
          !(pauseOnHover && hovered) && !(pauseOnFocus && focused) &&
          !document.hidden && !(reducedMotion && reducedMotion.matches);
      }

      function updateAutoplayControl() {
        autoplayButton.hidden = !autoplayConfigured;
        var running = rotationAllowed();
        autoplayButton.textContent = running ? "Opreste" : "Porneste";
        autoplayButton.setAttribute("aria-label", running ? "Opreste rotatia" : "Porneste rotatia");
        autoplayButton.setAttribute("aria-pressed", userPaused ? "true" : "false");
        track.setAttribute("aria-live", running ? "off" : "polite");
      }

      function cancelTimer() {
        if (timer) window.clearTimeout(timer);
        timer = 0;
      }

      function schedule() {
        cancelTimer();
        updateAutoplayControl();
        if (!rotationAllowed()) return;
        timer = window.setTimeout(function () {
          timer = 0;
          go(index + 1);
        }, interval);
      }

      function go(nextIndex) {
        if (loop) nextIndex = (nextIndex % slides.length + slides.length) % slides.length;
        else nextIndex = Math.max(0, Math.min(slides.length - 1, nextIndex));
        index = nextIndex;
        slides.forEach(function (slide, slideIndex) {
          var active = slideIndex === index;
          slide.hidden = !active;
          slide.setAttribute("aria-hidden", active ? "false" : "true");
          slide.setAttribute("aria-label", (slideIndex + 1) + " din " + slides.length);
        });
        indicatorButtons.forEach(function (button, buttonIndex) {
          button.setAttribute("aria-current", buttonIndex === index ? "true" : "false");
        });
        previous.disabled = slides.length < 2 || (!loop && index === 0);
        next.disabled = slides.length < 2 || (!loop && index === slides.length - 1);
        schedule();
      }

      function manualGo(nextIndex) {
        if (pauseOnInteraction) userPaused = true;
        go(nextIndex);
      }

      bag.listen(previous, "click", function () { manualGo(index - 1); });
      bag.listen(next, "click", function () { manualGo(index + 1); });
      bag.listen(autoplayButton, "click", function () {
        userPaused = !userPaused;
        schedule();
      });
      bag.listen(root, "keydown", function (event) {
        if (event.target !== root) return;
        if (["ArrowLeft", "ArrowRight", "Home", "End"].indexOf(event.key) < 0) return;
        event.preventDefault();
        manualGo(event.key === "Home" ? 0 : event.key === "End" ? slides.length - 1
          : event.key === "ArrowLeft" ? index - 1 : index + 1);
      });
      bag.listen(root, "mouseenter", function () { hovered = true; schedule(); });
      bag.listen(root, "mouseleave", function () { hovered = false; schedule(); });
      bag.listen(root, "focusin", function () { focused = true; schedule(); });
      bag.listen(root, "focusout", function () {
        bag.timer(function () {
          focused = root.contains(document.activeElement);
          schedule();
        }, 0);
      });
      bag.listen(document, "visibilitychange", schedule);
      if (reducedMotion) {
        if (reducedMotion.addEventListener) bag.listen(reducedMotion, "change", schedule);
        else if (reducedMotion.addListener) {
          reducedMotion.addListener(schedule);
          bag.add(function () { reducedMotion.removeListener(schedule); });
        }
      }
      bag.add(cancelTimer);
      go(index);
      return bag.dispose;
    }
  });
})();
