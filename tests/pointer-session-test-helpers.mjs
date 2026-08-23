export class TrackedEventTarget extends EventTarget {
  #listeners = new Map();

  addEventListener(type, listener, options) {
    super.addEventListener(type, listener, options);
    const listeners = this.#listeners.get(type) ?? new Set();
    listeners.add(listener);
    this.#listeners.set(type, listeners);
  }

  removeEventListener(type, listener, options) {
    super.removeEventListener(type, listener, options);
    this.#listeners.get(type)?.delete(listener);
  }

  listenerCount(type) {
    return this.#listeners.get(type)?.size ?? 0;
  }

  totalListenerCount() {
    return Array.from(this.#listeners.values())
      .reduce((total, listeners) => total + listeners.size, 0);
  }
}

export function pointerEvent(type, fields = {}) {
  const event = new Event(type, { cancelable: true });
  for (const [key, value] of Object.entries({
    pointerId: 1,
    clientX: 0,
    clientY: 0,
    button: 0,
    ...fields,
  })) {
    Object.defineProperty(event, key, { configurable: true, value });
  }
  return event;
}

export function keyboardEvent(key) {
  const event = new Event("keydown", { cancelable: true });
  Object.defineProperty(event, "key", { configurable: true, value: key });
  return event;
}

export function createPointerHarness() {
  const windowTarget = new TrackedEventTarget();
  const documentTarget = new TrackedEventTarget();
  documentTarget.visibilityState = "visible";
  let nextHandle = 1;
  const frames = new Map();
  const timers = new Map();
  const capture = {
    captured: new Set(),
    released: [],
    setPointerCapture(pointerId) {
      this.captured.add(pointerId);
    },
    hasPointerCapture(pointerId) {
      return this.captured.has(pointerId);
    },
    releasePointerCapture(pointerId) {
      this.captured.delete(pointerId);
      this.released.push(pointerId);
    },
  };
  const environment = {
    windowTarget,
    documentTarget,
    requestAnimationFrame(callback) {
      const handle = nextHandle++;
      frames.set(handle, callback);
      return handle;
    },
    cancelAnimationFrame(handle) {
      frames.delete(handle);
    },
    setTimeout(callback) {
      const handle = nextHandle++;
      timers.set(handle, callback);
      return handle;
    },
    clearTimeout(handle) {
      timers.delete(handle);
    },
  };
  return {
    windowTarget,
    documentTarget,
    environment,
    capture,
    frames,
    timers,
    runFrames() {
      const pending = Array.from(frames.entries());
      frames.clear();
      for (const [, callback] of pending) callback(0);
    },
    runTimers() {
      const pending = Array.from(timers.values());
      timers.clear();
      for (const callback of pending) callback();
    },
  };
}
