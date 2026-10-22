// Check the real workflow without a YAML package or GitHub access.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const workflow = readFileSync(new URL(
  "../../.github/workflows/rust.yml", import.meta.url,
), "utf8");

for (const [job, target] of [
  ["test-native", "rust/target"],
  ["check-wasm", "rust/target"],
  ["package-windows", "${{ runner.temp }}/fastfill-rust-target"],
  ["package-linux", "${{ runner.temp }}/fastfill-rust-linux-target"],
]) {
  test(`${job}: restore compiled dependencies before Cargo runs`, () => {
    const block = workflow.split(`\n  ${job}:\n`)[1]?.split(/\n  \w/)[0];
    assert.ok(block, `Find job ${job}`);
    const cache = block.match(
      /uses: actions\/cache@v6\n([\s\S]*?)(?=\n      - |$)/,
    );
    assert.ok(cache, "Cache downloaded AND compiled dependencies");
    for (const path of ["~/.cargo/registry", "~/.cargo/git", target]) {
      assert.ok(cache[1].includes(path), `Cache ${path}`);
    }
    const key = cache[1].match(/key: >-\n([\s\S]*?)\n          restore/);
    assert.ok(key, "Keep dependency changes in the primary cache key");
    assert.ok(key[1].includes("hashFiles('rust/Cargo.toml',"));
    assert.ok(key[1].includes("'rust/Cargo.lock')"));
    const prefix = key[1].replace(/\s+/g, " ").trim()
      .split("${{ hashFiles")[0].trimEnd();
    const restore = cache[1].match(/restore-keys: \|\n +([^\n]+)/);
    assert.equal(restore?.[1], prefix,
      "Reuse compatible artifacts after dependency changes");
    assert.match(prefix, /rust199/);
    if (job !== "package-windows") {
      assert.ok(prefix.includes("${{ runner.os }}-${{ runner.arch }}"));
    }
    assert.ok(block.indexOf(cache[0]) < block.indexOf("cargo +1.99.0"),
      "Restore before the first Cargo command");
  });
}

test("CI caches omit debug symbols and incremental compiler state", () => {
  for (const name of [
    "CARGO_INCREMENTAL", "CARGO_PROFILE_DEV_DEBUG", "CARGO_PROFILE_TEST_DEBUG",
  ]) {
    assert.ok(workflow.includes(`  ${name}: "0"`), name);
  }
  assert.ok(!workflow.includes("CARGO_PROFILE_TEST_DEBUG_ASSERTIONS"),
    "Do not disable test assertions");
});

test("Linux target path is set after runner assignment", () => {
  const block = workflow.split("\n  package-linux:\n")[1]
    ?.split(/\n  \w/)[0];
  assert.ok(block, "Find Linux package job");
  assert.doesNotMatch(block.split("\n    steps:")[0], /\$\{\{\s*runner\./,
    "Job-level env cannot use runner context");
  assert.match(block,
    /echo "CARGO_TARGET_DIR=\$RUNNER_TEMP\/fastfill-rust-linux-target"/,
    "Configure build output after the runner starts");
});
