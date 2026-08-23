(/** @type {Window & { __panaUnsafeExternalRan?: boolean }} */ (window)).__panaUnsafeExternalRan = true;

const workerSource = "while (true) {}";
const workerUrl = URL.createObjectURL(new Blob([workerSource], { type: "text/javascript" }));
new Worker(workerUrl);

queueMicrotask(function saturateMicrotasks() {
  queueMicrotask(saturateMicrotasks);
});
