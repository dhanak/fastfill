// Indicative source size, not a Rust parser or an arbitrary acceptance gate.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

function sourceLines(source) {
  const production = source.split(/#\[cfg\(test\)\]\r?\nmod tests\b/, 1)[0];
  return production.split(/\r?\n/).filter(line =>
    line.trim() && !line.trimStart().startsWith("//")).length;
}

function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    if (entry.name === "tests" || /(^tests|_tests)\.rs$/.test(entry.name)) {
      return [];
    }
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    return entry.isFile() && entry.name.endsWith(".rs") ? [path] : [];
  });
}

assert.equal(sourceLines("// note\r\n\r\nfn x() {}\r\n"), 1);
assert.equal(sourceLines("fn x() {}\n#[cfg(test)]\nmod tests {\n0\n}"), 1);
assert.equal(sourceLines("let url = \"https://example.test\";\n"), 1);

const root = fileURLToPath(new URL("../src/", import.meta.url));
const areas = ["annotations", "app", "page_canvas", "project", "all"];
console.log(JSON.stringify(areas.map(area => {
  const files = sourceFiles(area === "all" ? root : join(root, area));
  return { area, files: files.length,
    sourceLines: files.reduce((sum, path) =>
      sum + sourceLines(readFileSync(path, "utf8")), 0) };
}), null, 2));
