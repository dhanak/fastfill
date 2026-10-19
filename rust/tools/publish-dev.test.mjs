// Run the workflow's actual shell block; never call GitHub or need a token.
import assert from "node:assert/strict";
import {
  existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const workflow = readFileSync(new URL(
  "../../.github/workflows/rust.yml", import.meta.url,
), "utf8");
const step = workflow.split(
  "      - name: Publish separate Rust development build\n",
)[1];
assert.ok(step, "Find the release step in the workflow");
const block = step.match(/        run: \|\n([\s\S]*?)(?=\n  \w|$)/);
assert.ok(block, "Find the release step in the workflow");
const script = block[1].replace(/^ {10}/gm, "");
assert.equal(spawnSync("bash", ["-n"], { input: script }).status, 0);

test("Windows publication does not wait for macOS runner capacity", () => {
  const native = workflow.split("\n  native:\n")[1]?.split(/\n  \w/)[0];
  assert.ok(native?.includes("- ubuntu-latest"));
  assert.ok(native.includes("- windows-latest"));
  assert.ok(!native.includes("macos"), "Keep macOS out of the release gate");
  const macos = workflow.split("\n  macos:\n")[1]?.split(/\n  \w/)[0];
  assert.ok(macos?.includes("runs-on: macos-15-intel"));
  assert.ok(native.includes("steps: &native-tests"));
  assert.ok(macos.includes("steps: *native-tests"),
    "Keep the same cached test suite on macOS");
  assert.ok(!macos.includes("continue-on-error"),
    "Keep real macOS failures visible in workflow status");
  const publish = workflow.split("\n  publish-rust-dev:\n")[1]
    ?.split(/\n  \w/)[0];
  const needs = publish?.match(/needs: \[([^\]]+)\]/)?.[1]
    .split(",").map((job) => job.trim());
  assert.deepEqual(needs, ["native", "wasm", "windows-portable"],
    "Still require Windows/Linux tests, WASM checks and the packaged app");
  assert.ok(!publish.includes("always()"),
    "Do not bypass failed or cancelled required checks");
});

test("Download preserves the raw portable ZIP for publication", () => {
  const upload = workflow.split(
    "      - name: Upload portable Windows ZIP\n",
  )[1]?.split(/\n  \w/)[0];
  assert.match(upload ?? "", /archive: false/);
  const download = workflow.split(
    "      - name: Download tested portable ZIP\n",
  )[1]?.split(/\n      - /)[0];
  assert.match(download ?? "", /name: FastFill-Rust-windows-x64\.zip/);
  assert.match(download ?? "", /skip-decompress: true/,
    "Keep the uploaded ZIP instead of extracting its contents");
});

for (const mode of [
  "new", "existing", "upload-failure", "missing-zip", "empty-zip",
]) {
  test(`Publish Rust prerelease: ${mode}`, () => {
    const directory = mkdtempSync(join(tmpdir(), "fastfill-publish-test-"));
    try {
      const log = join(directory, "calls.txt");
      if (mode !== "missing-zip") {
        writeFileSync(join(directory, "FastFill-Rust-windows-x64.zip"),
          mode === "empty-zip" ? "" : "mock portable ZIP");
      }
      writeFileSync(join(directory, "gh"), `#!/bin/sh
printf '%s\\n' "$*" >> "$TEST_LOG"
if [ "$1 $2" = "release view" ]; then
  [ "$TEST_MODE" != new ]
elif [ "$1 $2" = "release upload" ]; then
  [ "$TEST_MODE" != upload-failure ]
else
  exit 0
fi
`, { mode: 0o755 });
      const result = spawnSync("bash", [
        "-e", "-u", "-o", "pipefail", "-c", script,
      ], {
        cwd: directory,
        encoding: "utf8",
        timeout: 10_000,
        env: {
          ...process.env,
          PATH: `${directory}${delimiter}${process.env.PATH}`,
          TEST_LOG: log,
          TEST_MODE: mode,
          GITHUB_REPOSITORY: "example/fastfill",
          GITHUB_SHA: "0123456789abcdef",
        },
      });
      if (mode === "missing-zip" || mode === "empty-zip") {
        assert.equal(result.status, 1, result.stderr);
        assert.match(result.stderr, /Missing or empty portable ZIP/);
        assert.ok(!existsSync(log),
          "Never call GitHub without a nonempty portable ZIP");
        return;
      }
      const calls = readFileSync(log, "utf8").trim().split("\n");
      assert.equal(calls[0], "release view rust-dev");
      assert.equal(result.status, mode === "upload-failure" ? 1 : 0,
        result.stderr);
      if (mode === "new") {
        assert.equal(calls.length, 2);
        assert.match(calls[1], /^release create rust-dev /);
        assert.match(calls[1], /FastFill-Rust-windows-x64\.zip/);
        assert.match(calls[1], /--prerelease --latest=false/);
        assert.match(calls[1], /--target 0123456789abcdef/);
      } else {
        assert.equal(calls[1], "release upload rust-dev " +
          "FastFill-Rust-windows-x64.zip --clobber");
        if (mode === "upload-failure") {
          assert.equal(calls.length, 2,
            "Never advance tag after failed upload");
        } else {
          assert.equal(calls.length, 4);
          assert.match(calls[2], /git\/refs\/tags\/rust-dev /);
          assert.match(calls[2], /-f sha=0123456789abcdef -F force=true/);
          assert.match(calls[3], /^release edit rust-dev /);
          assert.match(calls[3], /--prerelease --latest=false/);
        }
      }
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
}
