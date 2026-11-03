import assert from "node:assert/strict";
import test from "node:test";
import { capturePhoto } from "./camera.mjs";

function fixture(options = {}) {
  const buttons = new Map();
  const camera = {
    stopped: 0,
    requests: [],
    turns: [],
    drawn: [],
    bitmaps: [],
    track: {
      label: "Front camera",
      getSettings: () => ({ facingMode: "user", deviceId: "front" }),
      stop() { camera.stopped += 1; },
    },
  };
  const stream = {
    getTracks: () => [camera.track],
    getVideoTracks: () => [camera.track],
  };
  const element = () => ({
    disabled: false,
    hidden: false,
    classList: { toggle() {} },
    style: { setProperty(name, value) { this[name] = value; } },
    clientWidth: 800,
    clientHeight: 600,
    setAttribute(name, value) { this[name] = value; },
    removeAttribute() {},
    replaceChildren(...children) { this.children = children; },
    focus() {},
  });
  const video = {
    ...element(),
    videoWidth: 2,
    videoHeight: 2,
    readyState: 2,
    play: async () => {},
  };
  const photo = element();
  const select = { ...element(), replaceChildren() {}, value: "front" };
  const status = { textContent: "", style: {} };
  const nodes = new Map();
  const listeners = new Map();
  const dialog = {
    ...element(),
    querySelector(query) {
      if (query === "video") return video;
      if (query === "img") return photo;
      if (query === "select") return select;
      if (query === "[role=status]") return status;
      if (query.startsWith(".ff-camera-")) {
        if (!nodes.has(query)) nodes.set(query, element());
        return nodes.get(query);
      }
      const action = query.match(/data-action=(\w+)/)?.[1];
      if (!buttons.has(action)) buttons.set(action, element());
      return buttons.get(action);
    },
    showModal() {},
    close() {},
    remove() { camera.removed = true; },
  };
  const old = {
    document: globalThis.document,
    navigator: Object.getOwnPropertyDescriptor(globalThis, "navigator"),
    isSecureContext: globalThis.isSecureContext,
    Option: globalThis.Option,
    createObjectURL: URL.createObjectURL,
    revokeObjectURL: URL.revokeObjectURL,
    performance: Object.getOwnPropertyDescriptor(globalThis, "performance"),
  };
  globalThis.document = {
    activeElement: element(),
    body: { append() {} },
    head: { append() {} },
    hidden: false,
    addEventListener(name, listener) { listeners.set(name, listener); },
    removeEventListener(name) { listeners.delete(name); },
    getElementById: () => null,
    createElementNS: () => element(),
    createElement(name) {
      if (name === "dialog") return dialog;
      if (name === "canvas") return {
        width: 0,
        height: 0,
        getContext: () => ({
          translate(...args) { camera.turns.push(["translate", ...args]); },
          rotate(angle) { camera.turns.push(["rotate", angle]); },
          scale() {},
          drawImage(source) { camera.drawn.push(source); },
          getImageData() {
            return { data: new Uint8ClampedArray(16) };
          },
        }),
        toBlob(callback) {
          if (options.deferBlob) {
            camera.finishBlob = () => callback(new Blob(["png"]));
          } else {
            callback(new Blob(["png"]));
          }
        },
      };
      return element();
    },
  };
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: {
    mediaDevices: {
      getUserMedia: async request => {
        camera.requests.push(request);
        return stream;
      },
      enumerateDevices: async () => [
        { kind: "videoinput", label: "Front", deviceId: "front" },
        ...(options.twoCameras ? [
          { kind: "videoinput", label: "Rear", deviceId: "rear" },
        ] : []),
      ],
    },
  } });
  globalThis.isSecureContext = true;
  if (options.clock) globalThis.performance = options.clock;
  const oldBitmap = globalThis.createImageBitmap;
  globalThis.createImageBitmap = () => {
    const bitmap = {
      width: 2,
      height: 2,
      closed: false,
      close() { this.closed = true; },
    };
    camera.bitmaps.push(bitmap);
    if (options.deferBitmap) {
      return new Promise(resolve => {
        camera.finishBitmap = () => resolve(bitmap);
      });
    }
    return Promise.resolve(bitmap);
  };
  globalThis.Option = class { constructor(label, value) {
    this.label = label;
    this.value = value;
  } };
  URL.createObjectURL = () => "blob:photo";
  URL.revokeObjectURL = () => {};
  return {
    camera,
    photo,
    video,
    select,
    status,
    node: selector => nodes.get(selector),
    button: action => buttons.get(action),
    visibility(hidden) {
      document.hidden = hidden;
      listeners.get("visibilitychange")?.();
    },
    restore() {
      globalThis.document = old.document;
      if (old.navigator) {
        Object.defineProperty(globalThis, "navigator", old.navigator);
      } else {
        delete globalThis.navigator;
      }
      globalThis.isSecureContext = old.isSecureContext;
      globalThis.createImageBitmap = oldBitmap;
      globalThis.Option = old.Option;
      URL.createObjectURL = old.createObjectURL;
      URL.revokeObjectURL = old.revokeObjectURL;
      if (old.performance) {
        Object.defineProperty(globalThis, "performance", old.performance);
      }
    },
  };
}

const tick = () => new Promise(resolve => setImmediate(resolve));

test("camera rejects an insecure origin before opening a stream", async () => {
  const ui = fixture();
  try {
    globalThis.isSecureContext = false;
    await assert.rejects(capturePhoto(50_000_000), /HTTPS or localhost/);
    assert.equal(ui.camera.stopped, 0);
  } finally {
    ui.restore();
  }
});

test("cancel releases the camera without returning a photo", async () => {
  const ui = fixture();
  try {
    const result = capturePhoto(50_000_000);
    await tick();
    assert.equal(ui.button("capture").disabled, false);
    ui.button("cancel").onclick();
    assert.equal(await result, null);
    assert.equal(ui.camera.stopped, 1);
    assert.equal(ui.camera.removed, true);
  } finally {
    ui.restore();
  }
});

test("capture returns PNG only after Use photo", async () => {
  const ui = fixture();
  try {
    const result = capturePhoto(50_000_000);
    await tick();
    await ui.button("capture").onclick();
    assert.equal(ui.photo.hidden, false);
    assert.equal(ui.button("use").disabled, false);
    assert.equal(ui.camera.stopped, 1);
    await ui.button("use").onclick();
    assert.deepEqual(await result, {
      bytes: new TextEncoder().encode("png"), crop: null,
    });
    assert.equal(ui.camera.removed, true);
  } finally {
    ui.restore();
  }
});

test("over-budget photo stays in preview for retry or cancel", async () => {
  const ui = fixture();
  try {
    const result = capturePhoto(3);
    await tick();
    await ui.button("capture").onclick();
    assert.match(ui.status.textContent, /remaining project image memory/);
    assert.equal(ui.button("capture").disabled, false);
    assert.equal(ui.photo.hidden, true);
    ui.button("cancel").onclick();
    assert.equal(await result, null);
  } finally {
    ui.restore();
  }
});

test("a retake discards a still-encoding frame", async () => {
  const ui = fixture({ deferBlob: true });
  try {
    const result = capturePhoto(50_000_000);
    await tick();
    const encoding = ui.button("capture").onclick();
    ui.button("retry").onclick();
    await tick();
    ui.camera.finishBlob();
    await encoding;
    assert.equal(ui.photo.hidden, true);
    assert.equal(ui.button("capture").disabled, false);
    ui.button("cancel").onclick();
    assert.equal(await result, null);
    assert.equal(ui.camera.stopped, 2);
  } finally {
    ui.restore();
  }
});

test("switching cameras stops the old stream", async () => {
  const ui = fixture({ twoCameras: true });
  try {
    const result = capturePhoto(50_000_000);
    await tick();
    assert.equal(ui.select.disabled, false);
    ui.select.value = "rear";
    ui.select.onchange();
    await tick();
    assert.equal(ui.camera.stopped, 1);
    assert.equal(ui.camera.requests[1].video.deviceId.exact, "rear");
    ui.button("cancel").onclick();
    assert.equal(await result, null);
    assert.equal(ui.camera.stopped, 2);
  } finally {
    ui.restore();
  }
});

test("rotate changes preview and captured pixel orientation", async () => {
  const ui = fixture();
  try {
    const result = capturePhoto(50_000_000);
    await tick();
    ui.button("rotate").onclick();
    assert.match(ui.video.style.transform, /rotate\(90deg\)/);
    await ui.button("capture").onclick();
    assert.deepEqual(ui.camera.turns.at(-1), ["rotate", Math.PI / 2]);
    assert.equal(ui.button("use").hidden, false);
    assert.equal(ui.button("capture").hidden, true);
    ui.button("cancel").onclick();
    assert.equal(await result, null);
  } finally {
    ui.restore();
  }
});

test("pause and resume release and restart the stream", async () => {
  const ui = fixture();
  try {
    const result = capturePhoto(50_000_000);
    await tick();
    ui.button("pause").onclick();
    assert.equal(ui.camera.stopped, 1);
    assert.equal(ui.button("play").hidden, false);
    ui.button("play").onclick();
    await tick();
    assert.equal(ui.camera.requests.length, 2);
    assert.equal(ui.button("capture").disabled, false);
    ui.button("cancel").onclick();
    assert.equal(await result, null);
  } finally {
    ui.restore();
  }
});

const crop = {
  topLeft: { x: 0.1, y: 0.1 },
  topRight: { x: 0.9, y: 0.1 },
  bottomRight: { x: 0.9, y: 0.9 },
  bottomLeft: { x: 0.1, y: 0.9 },
};

test("auto capture reviews the exact analyzed frame and its crop", async () => {
  const ui = fixture();
  try {
    const analyze = pixels => {
      assert.ok(pixels instanceof Uint8Array);
      assert.equal(pixels.length, 16);
      return JSON.stringify({
        corners: crop, capture: crop, progress: 1,
        status: "Hold steady.",
      });
    };
    const result = capturePhoto(50_000_000, analyze);
    await new Promise(resolve => setTimeout(resolve, 160));
    assert.equal(ui.photo.hidden, false);
    assert.equal(ui.camera.drawn.length, 2);
    assert.equal(ui.camera.drawn[0], ui.camera.drawn[1]);
    assert.equal(ui.camera.bitmaps[0].closed, true);
    assert.equal(ui.node(".ff-camera-outline").children.length, 5);
    await ui.button("use").onclick();
    const accepted = await result;
    assert.deepEqual(accepted.bytes, new TextEncoder().encode("png"));
    assert.deepEqual(JSON.parse(accepted.crop), crop);
  } finally {
    ui.restore();
  }
});

test("auto capture keeps its review outline after hiding the tab", async () => {
  const ui = fixture();
  try {
    const analyze = () => JSON.stringify({
      corners: crop, capture: crop, progress: 1, status: "Hold steady.",
    });
    const result = capturePhoto(50_000_000, analyze);
    await new Promise(resolve => setTimeout(resolve, 160));
    const outline = ui.node(".ff-camera-outline");
    const points = outline.children[0].points;
    assert.equal(outline.children.length, 5);
    for (const hidden of [true, false]) {
      ui.visibility(hidden);
      assert.equal(ui.photo.hidden, false);
      assert.equal(outline.children.length, 5);
      assert.equal(outline.children[0].points, points);
    }
    assert.equal(ui.camera.requests.length, 1);
    await ui.button("use").onclick();
    assert.deepEqual(JSON.parse((await result).crop), crop);
  } finally {
    ui.restore();
  }
});

test("pausing discards an unfinished auto analysis frame", async () => {
  const ui = fixture({ deferBitmap: true });
  try {
    const analyze = () => JSON.stringify({
      corners: crop, capture: crop, progress: 1,
      status: "Hold steady.",
    });
    const result = capturePhoto(50_000_000, analyze);
    await new Promise(resolve => setTimeout(resolve, 160));
    assert.equal(typeof ui.camera.finishBitmap, "function");
    ui.button("pause").onclick();
    ui.camera.finishBitmap();
    await tick();
    assert.equal(ui.photo.hidden, true);
    assert.equal(ui.camera.bitmaps[0].closed, true);
    ui.button("cancel").onclick();
    assert.equal(await result, null);
  } finally {
    ui.restore();
  }
});

test("live detection draws the outline and countdown ring", async () => {
  const ui = fixture();
  try {
    const analyze = () => JSON.stringify({
      corners: crop, capture: null, progress: 0.5,
      status: "Hold steady.",
    });
    const result = capturePhoto(50_000_000, analyze);
    await new Promise(resolve => setTimeout(resolve, 160));
    assert.equal(ui.node(".ff-camera-outline").children.length, 5);
    assert.equal(ui.button("capture").style["--progress-angle"],
      "180deg");
    assert.equal(ui.status.textContent, "Hold steady.");
    ui.button("cancel").onclick();
    assert.equal(await result, null);
  } finally {
    ui.restore();
  }
});

test("slow analysis discards stale capture advice", async () => {
  const times = [0, 100, 1_200];
  const ui = fixture({ clock: { now: () => times.shift() ?? 1_200 } });
  try {
    const analyze = () => JSON.stringify({
      corners: crop, capture: crop, progress: 1,
      status: "Hold steady.",
    });
    const result = capturePhoto(50_000_000, analyze);
    await new Promise(resolve => setTimeout(resolve, 160));
    assert.equal(ui.photo.hidden, true);
    assert.match(ui.status.textContent, /catching up/);
    assert.equal(ui.button("capture").style["--progress-angle"],
      "0deg");
    ui.button("cancel").onclick();
    assert.equal(await result, null);
  } finally {
    ui.restore();
  }
});
