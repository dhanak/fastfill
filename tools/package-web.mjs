// Archive the Trunk output at the web root and check the extracted site.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  copyFileSync, existsSync, mkdtempSync, readFileSync, rmSync, statSync,
} from "node:fs";
import { join, resolve, sep } from "node:path";

const temp = process.env.RUNNER_TEMP;
assert.ok(temp, "RUNNER_TEMP must name the build directory");
const dist = join(temp, "fastfill-web-dist");
const archive = join(temp, "FastFill-web.tar.gz");

function checkSite(root) {
  const html = readFileSync(join(root, "index.html"), "utf8");
  assert.ok(statSync(join(root, "licenses/liberation/LICENSE")).isFile(),
    "Missing bundled-font license");
  const icon = html.match(/<link\b[^>]*\brel="icon"[^>]*\bhref="([^"]+)"/);
  assert.ok(icon, "Missing favicon");
  assert.ok(icon[1].startsWith("./"), "Favicon URL is not relative");
  const iconPath = resolve(root, icon[1]);
  assert.ok(iconPath.startsWith(`${resolve(root)}${sep}`),
    "Favicon URL escapes web root");
  assert.ok(statSync(iconPath).isFile(), "Missing favicon asset");
  for (const name of ["x", "rotate-cw",
    "triangles-centerline-dashed-vertical", "camera", "pause", "play",
    "undo-2", "settings", "switch-camera", "check"]) {
    assert.ok(statSync(join(root, "icons", `${name}.svg`)).isFile(),
      `Missing camera icon: ${name}`);
  }
  const refs = [...html.matchAll(/['"]([^'"<>]+\.(?:js|mjs|wasm))['"]/g)]
    .map((match) => match[1]);
  assert.ok(refs.some((ref) => ref.endsWith(".js")), "Missing JS");
  const wasm = refs.find((ref) => ref.endsWith(".wasm"));
  assert.ok(wasm, "Missing WASM");
  for (const ref of refs) {
    assert.ok(ref.startsWith("./"), `Asset URL is not relative: ${ref}`);
    const path = resolve(root, ref);
    assert.ok(path.startsWith(`${resolve(root)}${sep}`), ref);
    assert.ok(statSync(path).isFile(), `Missing asset: ${ref}`);
  }
  const magic = readFileSync(resolve(root, wasm)).subarray(0, 4);
  assert.ok(magic.equals(Buffer.from([0, 97, 115, 109])));
  const js = refs.find((ref) => ref.endsWith(".js"));
  const imports = readFileSync(resolve(root, js), "utf8")
    .matchAll(/from ['"](\.[^'"]+)['"]/g);
  for (const [, ref] of imports) {
    const path = resolve(root, ref);
    assert.ok(path.startsWith(`${resolve(root)}${sep}`), ref);
    assert.ok(statSync(path).isFile(), `Missing JS import: ${ref}`);
  }
}

checkSite(dist);
copyFileSync(new URL("../docs/WEB.md", import.meta.url),
  join(dist, "README.md"));
copyFileSync(new URL("../LICENSE", import.meta.url),
  join(dist, "LICENSE.txt"));
copyFileSync(new URL("../assets/icons/lucide/LICENSE", import.meta.url),
  join(dist, "LUCIDE-LICENSE.txt"));
execFileSync("tar", ["-czf", archive, "-C", dist, "."]);
assert.ok(existsSync(archive) && statSync(archive).size > 0);
const extracted = mkdtempSync(join(temp, "fastfill-web-smoke-"));
try {
  execFileSync("tar", ["-xzf", archive, "-C", extracted]);
  checkSite(extracted);
  for (const file of ["README.md", "LICENSE.txt", "LUCIDE-LICENSE.txt"]) {
    assert.ok(statSync(join(extracted, file)).isFile());
  }
  assert.ok(!existsSync(join(extracted, "dist")));
} finally {
  rmSync(extracted, { recursive: true, force: true });
}
process.stdout.write("Browser archive extraction and assets passed.\n");
