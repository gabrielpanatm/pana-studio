(function () {
  "use strict";
  /* PANA BLOCK PROVIDER: counter */
  var runtime = window.PanaBlockRuntime;
  if (!runtime) throw new Error("PanaBlockRuntime core lipsește pentru providerul counter.");

  runtime.register("counter", {
    mount: function (element, makeBag) {
      var bag = makeBag();
      var observerHandle = null;
      var animationFrame = 0;
      var started = false;
      function run() {
        if (started) return;
        started = true;
        var target = parseInt(element.getAttribute("data-tinta") || "0", 10);
        var duration = parseInt(element.getAttribute("data-durata") || "1800", 10);
        var suffix = element.getAttribute("data-sufix") || "";
        if (!isFinite(target)) target = 0;
        if (!isFinite(duration) || duration < 1) duration = 1800;
        var start = null;
        function tick(timestamp) {
          if (start === null) start = timestamp;
          var progress = Math.min((timestamp - start) / duration, 1);
          element.textContent = String(Math.floor(target * progress)) + suffix;
          if (progress < 1) animationFrame = window.requestAnimationFrame(tick);
          else element.textContent = String(target) + suffix;
        }
        animationFrame = window.requestAnimationFrame(tick);
      }
      if ("IntersectionObserver" in window) {
        observerHandle = new IntersectionObserver(function (entries) {
          entries.forEach(function (entry) {
            if (!entry.isIntersecting) return;
            run();
            observerHandle.unobserve(entry.target);
          });
        }, { threshold: 0.3 });
        observerHandle.observe(element);
      } else {
        run();
      }
      bag.add(function () {
        if (observerHandle) observerHandle.disconnect();
        if (animationFrame) window.cancelAnimationFrame(animationFrame);
      });
      return { state: { started: function () { return started; } }, dispose: bag.dispose };
    }
  });
})();
