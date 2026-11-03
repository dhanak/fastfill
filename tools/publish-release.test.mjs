// Run the publication script; never call GitHub or need a token.
import assert from "node:assert/strict";
import {
  cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const workflow = readFileSync(new URL(
  "../.github/workflows/build.yml", import.meta.url,
), "utf8");
const script = readFileSync(new URL(
  "publish-release.sh", import.meta.url,
), "utf8");
assert.equal(spawnSync("bash", ["-n"], { input: script }).status, 0);
const macosPackage = readFileSync(
  new URL("package-macos.sh", import.meta.url), "utf8",
);
const webPackage = fileURLToPath(new URL(
  "package-web.mjs", import.meta.url,
));

test("browser archive has web-root files and relative assets", () => {
  const directory = mkdtempSync(join(tmpdir(), "fastfill-web-test-"));
  try {
    const dist = join(directory, "fastfill-web-dist");
    mkdirSync(dist);
    writeFileSync(join(dist, "index.html"), `
<link rel="icon" href="./AppIcon.png">
<script type="module">
import init from './app.js';
await init({ module_or_path: './app.wasm' });
</script>`);
    writeFileSync(join(dist, "app.js"), "export default () => {};\n");
    writeFileSync(join(dist, "app.wasm"),
      Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]));
    writeFileSync(join(dist, "AppIcon.png"), "favicon");
    cpSync(fileURLToPath(new URL("../assets/icons/lucide",
      import.meta.url)), join(dist, "icons"), { recursive: true });
    cpSync(new URL("../assets/fonts/license", import.meta.url),
      join(dist, "licenses", "liberation"), { recursive: true });
    const run = () => spawnSync(process.execPath, [webPackage], {
      env: { ...process.env, RUNNER_TEMP: directory },
      encoding: "utf8",
    });
    const result = run();
    assert.equal(result.status, 0, result.stderr);
    const archive = join(directory, "FastFill-web.tar.gz");
    const contents = spawnSync("tar", ["-tzf", archive], {
      encoding: "utf8",
    });
    assert.equal(contents.status, 0);
    assert.match(contents.stdout, /\.\/index\.html/);
    assert.match(contents.stdout, /\.\/AppIcon\.png/);
    assert.match(contents.stdout, /\.\/icons\/camera\.svg/);
    assert.match(contents.stdout, /\.\/licenses\/liberation\/LICENSE/);
    assert.doesNotMatch(contents.stdout, /dist\/index\.html/);
    writeFileSync(join(dist, "index.html"),
      `<link rel="icon" href="./AppIcon.png">
<script>import init from '/app.js';</script>`);
    assert.notEqual(run().status, 0, "Reject root-bound asset URLs");
    writeFileSync(join(dist, "index.html"),
      `<script>import init from './app.js';</script>`);
    assert.notEqual(run().status, 0, "Reject missing favicon");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("macOS archive signs the universal bundle and checks extraction", () => {
  assert.equal(spawnSync("bash", ["-n"], {
    input: macosPackage,
  }).status, 0);
  assert.match(macosPackage, /lipo -create -output/);
  assert.match(macosPackage,
    /lipo "\$binary" -verify_arch x86_64 arm64/);
  assert.match(macosPackage,
    /lipo "\$executable" -verify_arch x86_64 arm64/);
  assert.match(macosPackage, /iconutil -c icns/);
  assert.match(macosPackage, /codesign --force --sign - /);
  assert.match(macosPackage, /ditto -c -k --sequesterRsrc/);
  assert.match(macosPackage, /ditto -x -k/);
  assert.match(macosPackage, /Signature=adhoc/);
  assert.ok(macosPackage.indexOf("lipo -create") <
    macosPackage.indexOf("codesign --force"));
  assert.ok(macosPackage.indexOf("codesign --force") <
    macosPackage.indexOf("ditto -c -k"));
});

test("CI jobs are grouped as checks, packages, then publication", () => {
  const jobs = [...workflow.split("\njobs:\n")[1]
    .matchAll(/^  ([a-z][a-z-]*):\n/gm)]
    .map((match) => match[1]);
  assert.deepEqual(jobs, [
    "test-native", "package-web",
    "package-windows", "package-linux", "package-macos",
    "publish", "deploy-pages",
  ]);
});

test("Pages deploys only published version tags", () => {
  const pages = workflow.split("\n  deploy-pages:\n")[1];
  assert.ok(pages, "Find the Pages job");
  assert.match(pages, /startsWith\(github\.ref, 'refs\/tags\/v'\)/);
  assert.doesNotMatch(pages, /refs\/heads\/master/);
  assert.match(pages, /needs: publish/);
  assert.match(pages, /name: FastFill-web\.tar\.gz/);
  assert.match(pages, /skip-decompress: true/);
  assert.match(pages, /tar -xzf FastFill-web\.tar\.gz -C site/);
  assert.match(pages, /path: site/);
  assert.match(pages, /actions\/deploy-pages@v4/);
});

test("Publication waits for all four tested packages", () => {
  const native = workflow.split("\n  test-native:\n")[1]
    ?.split(/\n  \w/)[0];
  assert.ok(native?.includes("- ubuntu-latest"));
  assert.ok(native.includes("- windows-latest"));
  const macos = workflow.split("\n  package-macos:\n")[1]
    ?.split(/\n  \w/)[0];
  assert.ok(macos?.includes("runs-on: macos-15-intel"));
  assert.match(macos, /cargo \+1\.99\.0 test --all-features --all-targets/);
  assert.match(macos, /--target x86_64-apple-darwin/);
  assert.match(macos, /--target aarch64-apple-darwin/);
  assert.match(macos, /bash \.\/tools\/package-macos\.sh/);
  const web = workflow.split("\n  package-web:\n")[1]
    ?.split(/\n  \w/)[0];
  assert.match(web, /trunk" build --release --locked --all-features/);
  assert.match(web, /--public-url \.\//);
  assert.match(web, /node tools\/package-web\.mjs/);
  assert.match(web, /\b[0-9a-f]{64}\b/,
    "Pin Trunk with a complete SHA-256 checksum");
  assert.match(web, /sha256sum -c -/);
  const publish = workflow.split("\n  publish:\n")[1]
    ?.split(/\n  \w/)[0];
  const needs = publish?.match(/needs: \[([^\]]+)\]/)?.[1]
    .split(",").map((job) => job.trim());
  assert.deepEqual(needs, [
    "test-native", "package-web", "package-windows", "package-linux",
    "package-macos",
  ],
    "Require shared checks and all four tested archives");
  assert.ok(!publish.includes("always()"),
    "Do not bypass failed or cancelled required checks");
});

test("Download preserves all four raw archives for publication", () => {
  const upload = workflow.split(
    "      - name: Upload portable Windows ZIP\n",
  )[1]?.split(/\n  \w/)[0];
  assert.match(upload ?? "", /archive: false/);
  const download = workflow.split(
    "      - name: Download tested Windows ZIP\n",
  )[1]?.split(/\n      - /)[0];
  assert.match(download ?? "", /name: FastFill-windows-x64\.zip/);
  assert.match(download ?? "", /skip-decompress: true/,
    "Keep the uploaded ZIP instead of extracting its contents");
  const linuxUpload = workflow.split(
    "      - name: Upload portable Linux archive\n",
  )[1]?.split(/\n  \w/)[0];
  assert.match(linuxUpload ?? "", /archive: false/);
  const linuxDownload = workflow.split(
    "      - name: Download tested Linux archive\n",
  )[1]?.split(/\n      - /)[0];
  assert.match(linuxDownload ?? "",
    /name: FastFill-linux-x64\.tar\.gz/);
  assert.match(linuxDownload ?? "", /skip-decompress: true/,
    "Keep the uploaded tarball instead of extracting its contents");
  const macosUpload = workflow.split(
    "      - name: Upload portable macOS ZIP\n",
  )[1]?.split(/\n  \w/)[0];
  assert.match(macosUpload ?? "", /archive: false/);
  const macosDownload = workflow.split(
    "      - name: Download tested macOS ZIP\n",
  )[1]?.split(/\n      - /)[0];
  assert.match(macosDownload ?? "",
    /name: FastFill-macos-universal\.zip/);
  assert.match(macosDownload ?? "", /skip-decompress: true/);
  const webUpload = workflow.split(
    "      - name: Upload deployable browser archive\n",
  )[1]?.split(/\n  \w/)[0];
  assert.match(webUpload ?? "", /archive: false/);
  const webDownload = workflow.split(
    "      - name: Download tested browser archive\n",
  )[1]?.split(/\n      - /)[0];
  assert.match(webDownload ?? "", /name: FastFill-web\.tar\.gz/);
  assert.match(webDownload ?? "", /skip-decompress: true/);
});

const archives = {
  windows: "FastFill-windows-x64.zip",
  linux: "FastFill-linux-x64.tar.gz",
  macos: "FastFill-macos-universal.zip",
  web: "FastFill-web.tar.gz",
};

function publish(mode, ref = "refs/heads/master", version = "1.2.3") {
  const directory = mkdtempSync(join(tmpdir(), "fastfill-publish-test-"));
  try {
    const log = join(directory, "calls.txt");
    writeFileSync(join(directory, "Cargo.toml"),
      `[package]\nname = "fastfill"\nversion = "${version}"\n`);
    for (const [platform, archive] of Object.entries(archives)) {
      if (mode !== `missing-${platform}`) {
        writeFileSync(join(directory, archive),
          mode === `empty-${platform}` ? "" : "mock archive");
      }
    }
    writeFileSync(join(directory, "gh"), `#!/bin/sh
echo "$*" >> "$TEST_LOG"
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
        GITHUB_REF: ref,
        GITHUB_REPOSITORY: "example/fastfill",
        GITHUB_SHA: "0123456789abcdef",
      },
    });
    return { ...result, calls: existsSync(log)
      ? readFileSync(log, "utf8").trim().split("\n") : [] };
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

for (const mode of [
  "new", "existing", "upload-failure",
  "missing-windows", "empty-windows", "missing-linux", "empty-linux",
  "missing-macos", "empty-macos", "missing-web", "empty-web",
]) {
  test(`Publish development release: ${mode}`, () => {
    const result = publish(mode);
    if (mode.startsWith("missing-") || mode.startsWith("empty-")) {
      assert.equal(result.status, 1, result.stderr);
      assert.match(result.stderr, /Missing or empty archive/);
      assert.deepEqual(result.calls, [],
        "Never call GitHub without all four nonempty archives");
      return;
    }
    const calls = result.calls;
    assert.equal(calls[0], "release view dev");
    assert.equal(result.status, mode === "upload-failure" ? 1 : 0,
      result.stderr);
    if (mode === "new") {
      assert.equal(calls.length, 2);
      assert.match(calls[1], /^release create dev /);
      for (const archive of Object.values(archives)) {
        assert.ok(calls[1].includes(archive));
      }
      assert.match(calls[1], /--prerelease=true --latest=false/);
      assert.match(calls[1], /--target 0123456789abcdef/);
    } else {
      assert.equal(calls[1], "release upload dev " +
        Object.values(archives).join(" ") + " --clobber");
      if (mode === "upload-failure") {
        assert.equal(calls.length, 2,
          "Never advance the tag after failed upload");
      } else {
        assert.equal(calls.length, 4);
        assert.match(calls[2], /git\/refs\/tags\/dev /);
        assert.match(calls[2], /-f sha=0123456789abcdef -F force=true/);
        assert.match(calls[3], /^release edit dev /);
        assert.match(calls[3], /--prerelease --latest=false/);
      }
    }
  });
}

for (const version of ["1.2.3", "1.2.3-alpha.1", "1.2.3+build-alpha"]) {
  for (const mode of ["new", "existing", "upload-failure"]) {
    test(`Publish version v${version}: ${mode}`, () => {
      const result = publish(mode, `refs/tags/v${version}`, version);
      assert.equal(result.status, mode === "upload-failure" ? 1 : 0,
        result.stderr);
      assert.equal(result.calls[0], `release view v${version}`);
      assert.ok(result.calls.every(call => !call.startsWith("api ")),
        "Version tags must never move");
      const prerelease = version.split("+", 1)[0].includes("-");
      const flags = `--prerelease=${prerelease} --latest=${!prerelease}`;
      if (mode === "new") {
        assert.equal(result.calls.length, 2);
        assert.ok(result.calls[1].startsWith(`release create v${version} `));
        assert.ok(result.calls[1].includes(flags));
        assert.match(result.calls[1], /--verify-tag --generate-notes/);
        for (const archive of Object.values(archives)) {
          assert.ok(result.calls[1].includes(archive));
        }
      } else if (mode === "existing") {
        assert.equal(result.calls.length, 3);
        assert.ok(result.calls[1].startsWith(`release upload v${version} `));
        assert.ok(result.calls[2].startsWith(`release edit v${version} `));
        assert.ok(result.calls[2].includes(flags));
      } else {
        assert.equal(result.calls.length, 2,
          "Never edit a release after failed upload");
      }
    });
  }
}

for (const [ref, version] of [
  ["refs/tags/v1.2.4", "1.2.3"],
  ["refs/tags/v1.2.3", ""],
  ["refs/heads/feature", "1.2.3"],
  ["", "1.2.3"],
]) {
  test(`Reject invalid publication ref/version: ${ref || "empty ref"}`, () => {
    const result = publish("new", ref, version);
    assert.equal(result.status, 1, result.stderr);
    assert.deepEqual(result.calls, []);
  });
}
