  function boundsForElements(elements) {
    var rect = null;
    elements.forEach(function (element) {
      if (!document.contains(element)) return;
      var next = element.getBoundingClientRect();
      if (next.width <= 0 && next.height <= 0) return;
      if (!rect) {
        rect = {
          left: next.left,
          top: next.top,
          right: next.right,
          bottom: next.bottom
        };
        return;
      }
      rect.left = Math.min(rect.left, next.left);
      rect.top = Math.min(rect.top, next.top);
      rect.right = Math.max(rect.right, next.right);
      rect.bottom = Math.max(rect.bottom, next.bottom);
    });
    if (!rect) return null;
    return {
      left: rect.left,
      top: rect.top,
      width: rect.right - rect.left,
      height: rect.bottom - rect.top
    };
  }

  function borderRadiusForElements(elements) {
    if (!elements || elements.length !== 1 || !document.contains(elements[0])) {
      return "0px";
    }
    return window.getComputedStyle(elements[0]).borderRadius || "0px";
  }
