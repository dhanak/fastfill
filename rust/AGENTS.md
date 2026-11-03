# Build isolation

Repository `target/` directories belong to the host developer. Never use them
for agent builds, checks or tests, and never clean, remove or move them.

Set `CARGO_TARGET_DIR` explicitly on every Cargo or Trunk invocation:

- Native: `/tmp/fastfill-codex-native-target`.
- WASM: `/tmp/fastfill-codex-target`.

Keep these caches separate from host build output. Do not copy host caches.
