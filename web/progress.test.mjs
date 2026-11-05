import assert from "node:assert/strict";
import test from "node:test";
import { yieldForPaint } from "./progress.mjs";

async function withScheduler(check) {
  const saved = new Map(["setTimeout", "clearTimeout",
    "requestAnimationFrame", "cancelAnimationFrame"]
    .map(name => [name, globalThis[name]]));
  const frames = new Map();
  const timers = new Map();
  let id = 0;
  globalThis.requestAnimationFrame = callback => {
    frames.set(++id, callback);
    return id;
  };
  globalThis.cancelAnimationFrame = frame => frames.delete(frame);
  globalThis.setTimeout = (callback, delay) => {
    assert.equal(delay, 50);
    timers.set(++id, callback);
    return id;
  };
  globalThis.clearTimeout = timer => timers.delete(timer);
  try {
    await check({ frames, timers, paint() {
      const batch = [...frames.values()];
      frames.clear();
      for (const callback of batch) callback();
    } });
  } finally {
    for (const [name, value] of saved) {
      if (value === undefined) delete globalThis[name];
      else globalThis[name] = value;
    }
  }
}

test("progress yields through a paint before continuing CPU work", () =>
  withScheduler(async ({ frames, timers, paint }) => {
    let finished = false;
    const waiting = yieldForPaint().then(() => { finished = true; });
    assert.equal(finished, false);
    paint();
    await Promise.resolve();
    assert.equal(finished, false, "first callback still precedes painting");
    assert.equal(frames.size, 1);
    paint();
    await waiting;
    assert.equal(finished, true);
    assert.equal(frames.size, 0);
    assert.equal(timers.size, 0);
  }));

test("hidden-tab fallback continues and cancels queued animation callbacks",
  async () => {
    for (const firstPaint of [false, true]) {
      await withScheduler(async ({ frames, timers, paint }) => {
        const waiting = yieldForPaint();
        if (firstPaint) paint();
        assert.equal(timers.size, 1);
        [...timers.values()][0]();
        await waiting;
        assert.equal(frames.size, 0);
        assert.equal(timers.size, 0);
      });
    }
  });
