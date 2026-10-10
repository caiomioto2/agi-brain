# Validation

Windows x64 release validation, 2026-10-10. Native Rust / egui / wgpu rendering,
with Rust 1.99.0, MSVC 14.44 and Windows SDK 10.0.26100 in the development environment.

## Reproducible checks

- `cargo fmt -- --check`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo test --locked --all-targets`: 18 unit tests; hosted integration excluded by default.
- `cargo build --locked --release --bins`: GUI and MCP executables.
- `scripts/verify-native.ps1`: synthetic demo, default Constellation, explicit Gephi,
  focus, replay, search, settings, help, appearance and explicit Constellation.
- `scripts/verify-context.ps1`: initialization, five-tool catalog, synthetic identity,
  recall, combined context and explanation using the packaged executable.
- `scripts/verify-window.ps1`: real Windows startup with a saved 64-pixel window,
  then restoration/resizing and relaunch; both launches must be visible and maximized.

Tests cover graph normalization, duplicate identities, filters, stable type colors,
replay, point-before-line animation, neighbor insertion, deletion, stable positions,
selection across reordering, retained camera/favorites/type filters, debounced file
changes, invalid exports and recovery, settings migration, explicit style persistence,
atomic writes, safe Markdown paths, export budgets and SSH validation.

Native checks render PNGs with the actual GPU renderer, verify reports, Windows
tray creation, language and style, and sample pixels to distinguish canvas backgrounds.
The default case uses fresh preferences and no style override. Verification artifacts
are ignored by Git and do not belong in a release.

## Optional hosted checks

The existing private PostgreSQL reader was tested with the opt-in `live_aws` test:
graph and page reads, rejection of arbitrary SQL, `transaction_read_only=on`, two
polling cycles and recovery after terminating only the test-owned SSH process.
Native scenarios cover fresh/cached hosted maps, manual refresh retaining selection,
page preview, context export and Constellation. The MCP check also ran against the
hosted reader. No GBrain records were changed to provoke remote graph updates.

Run these only with your configured SSH alias and deployed reader; see README.
Hosted results prove that tested installation, not compatibility with every GBrain
schema or AWS account. Remote change detection polls at the configured interval;
it is not server-pushed notification. Local controlled files exercise graph changes.

## Boundaries

The portable Windows package is unsigned. Opening in Obsidian requires a local vault.
Search is lexical metadata search. Timeline replay uses creation dates in the current
graph, rather than historical snapshots of deleted pages or links. Gephi is a visual
style, not ForceAtlas2. Native screenshots are not a frame-rate benchmark.

GitHub Actions separately verifies clean Windows builds; its current result is linked
from the README badge. Personal preferences, hosted graph data and page bodies are
excluded from the public source and package.
