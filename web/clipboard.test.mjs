import assert from "node:assert/strict";
import test from "node:test";
import { readClipboard, writeClipboard } from "./clipboard.mjs";

test("browser clipboard transfers text, enforces limits and reports refusal",
  async (t) => {
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, "navigator");
    t.after(() => {
      if (descriptor) {
        Object.defineProperty(globalThis, "navigator", descriptor);
      } else {
        delete globalThis.navigator;
      }
    });
    let contents = "árvíztűrő 🙂\nSecond line";
    const clipboard = {
      readText: async () => contents,
      writeText: async (text) => { contents = text; },
    };
    Object.defineProperty(globalThis, "navigator", {
      configurable: true, value: { clipboard },
    });
    assert.equal(await readClipboard(), contents);
    await writeClipboard("Copied selection");
    assert.equal(contents, "Copied selection");
    contents = "🙂".repeat(5_000);
    assert.equal(await readClipboard(), contents);
    contents += "x";
    await assert.rejects(readClipboard(),
      (error) => String(error).includes("10,000 characters"));
    clipboard.readText = clipboard.writeText = async () => {
      throw new Error("Permission denied");
    };
    await assert.rejects(readClipboard(),
      (error) => error === "Permission denied");
    await assert.rejects(writeClipboard("Keep original selection"),
      (error) => error === "Permission denied");
    delete navigator.clipboard;
    await assert.rejects(readClipboard(),
      (error) => String(error).includes("HTTPS or localhost"));
  });
