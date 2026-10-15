// Runnable browser-adapter contract check, without a DOM/test dependency.
import assert from "node:assert/strict";
import { downloadFile } from "./download.mjs";

const pending = [];
const events = [];
let payload;
const link = {
  click() { events.push("click"); },
  remove() { events.push("remove"); },
};
globalThis.document = {
  body: {
    appendChild(element) {
      assert.equal(element, link);
      events.push("append");
    },
  },
  createElement(tag) {
    assert.equal(tag, "a");
    return link;
  },
};
globalThis.URL = {
  createObjectURL(blob) {
    payload = blob;
    return "blob:download";
  },
  revokeObjectURL(url) {
    assert.equal(url, "blob:download");
    events.push("revoke");
  },
};
globalThis.setTimeout = (callback, delay) => {
  assert.ok(delay > 0, "URL must survive asynchronous download startup");
  pending.push(callback);
};

const source = new Uint8Array([0, 1, 2, 3, 4]);
for (const [name, mime] of [
  ["Árvíztűrő.pdf", "application/pdf"],
  ["form.docscan", "application/zip"],
]) {
  events.length = 0;
  downloadFile(name, source.subarray(1, 4), mime);
  assert.equal(link.download, name);
  assert.equal(link.href, "blob:download");
  assert.equal(link.hidden, true);
  assert.equal(payload.type, mime);
  assert.deepEqual(new Uint8Array(await payload.arrayBuffer()),
    new Uint8Array([1, 2, 3]));
  assert.deepEqual(events, ["append", "click", "remove"]);
  assert.equal(pending.length, 1);
  pending.pop()();
  assert.deepEqual(events, ["append", "click", "remove", "revoke"]);
}

for (const phase of ["append", "click"]) {
  events.length = 0;
  const target = phase === "append" ? document.body : link;
  const method = phase === "append" ? "appendChild" : "click";
  const previous = target[method];
  target[method] = () => { throw new Error("Browser refused download"); };
  assert.throws(() => downloadFile("form.pdf", source, "application/pdf"),
    /Browser refused/);
  assert.ok(events.includes("remove"));
  assert.equal(pending.length, 1);
  pending.pop()();
  assert.ok(events.includes("revoke"));
  target[method] = previous;
}
console.log("Browser download adapter checks passed.");
