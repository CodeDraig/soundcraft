# SoundCraft — instructions for agents

SoundCraft is a digital audio workstation targeting Avid Pro Tools parity — and superiority (speed, openness, agent control).

## Failure handling and session preservation

Changes must not introduce known panic paths in the operation being modified. Invalid input and unsupported operations must return an explicit error rather than silently substituting session data.
If a reproducible crash prevents completing or verifying the requested change, fix it first and add a regression test. Report unrelated crashes without expanding the task.
Failed commands and imports must preserve the last valid session state. Do not claim this is verified solely because a panic was caught.
Follow the repository's enforced restrictions on panic-producing APIs, indexing, arithmetic, recursion, allocation and unsafe code. The existing CLAP, VST3 and Audio Unit FFI boundaries do not permit unsafe code elsewhere.
Keep the audio callback non-blocking and free of steady-state allocation and panic paths. A successful offline bounce does not verify real-time recording or playback.

## Required capabilities

Keep session editing, audio processing, and file operations usable without the
graphical interface. The desktop UI, CLI, and automation must invoke the same
implementation of each operation they expose. Live playback and offline bounce
must use the same mixing implementation.

Verify these requirements through the affected headless, UI, playback, or export
paths. Do not introduce dependency restrictions solely to preserve an architectural
classification. Independent publication of individual crates is not a requirement
unless the user explicitly requests it.

## Commands and implementation boundaries

- **Expose session operations as commands.** Implement session operations in
  `crates/engine/src/cmd/*`. Each command declares its ID, label, menu path, shortcut
  where applicable, parameter documentation, availability check (`enabled`), and
  implementation (`run`), with tests for its behavior. Menus, panels, tools, the CLI
  and automation must use the same command IDs for the same operation. New command
  modules provide `specs()` and are registered in
  `crates/engine/src/cmd/mod.rs` by declaring the module and extending
  `command_specs()` with its specs. Preserve the existing
  gesture preview, cancellation and undo boundaries when changing interactive tools.

- **Keep interface commands accessible to agents.** Register UI operations in
  `UI_COMMANDS` in `crates/ui-egui/src/menus.rs`. Engine and UI commands must be
  discoverable and callable through the running app's control channel and connected
  MCP. UI commands require the application UI; they are unavailable in headless MCP.
  “UI-only” identifies implementation location and runtime requirements, not a
  restriction to human use. Parameterized `engine.execute` calls do not open dialogs;
  menu-style invocation through `ui.menu.invoke` may open them.

- **Put session behavior in the engine and presentation behavior in the UI.**
  Panels read engine state and invoke `app.run(id, params)`, which dispatches engine
  operations through `engine.execute(id, params)`. They must not independently
  implement session mutation, validation or undo behavior. Focus, scrolling and
  dialog state belong in the UI. Use `theme::Tokens` from
  `crates/ui-egui/src/theme.rs` for application-interface
  colours; user-selected track and clip colours retain their own values.

- **Implement application behavior in Rust.** Do not add handwritten JavaScript
  or TypeScript. Toolchain-generated browser glue is permitted. Existing build,
  packaging and development scripts are outside the application-language restriction.

`crates/engine/catalog/menus.txt` lists the incumbent's menu leaves (names only).
`cargo xtask parity` regenerates and overwrites `docs/parity.md`; preserve any local
edits before running it. To inspect the report without writing a file, use
`cargo run -p soundcraft-cli -- parity`. The engine test `parity_floor` in
`crates/engine/src/catalog.rs` checks fixed minimum counts and percentages; it does
not automatically raise those thresholds or verify feature completeness. The
running UI includes additional menu mappings in its `engine.parity` response.

## Running and looking at the app

Run these commands from the repository root. Cargo builds and runs the requested
binary; these examples do not require `soundcraft-cli` to be installed on PATH.
All repository paths below are relative to the repository root. `PORT` is a
placeholder for the actual numeric control port, not a literal argument.
Choose output paths that do not overwrite files you need to preserve.

- **Launch the desktop app with the demo session and control server:**

  ```sh
  cargo run --release -p soundcraft -- --demo --control 0
  ```

  Port `0` selects a free localhost port. Use the port printed by the application
  in subsequent TCP requests and in place of `PORT` below. The application prints
  `SOUNDCRAFT_CONTROL_PORT=<number>` to stdout after binding successfully. Run CLI
  control requests from another terminal while the app remains open.

- **Control the running app:** send newline-delimited JSON over TCP to
  `127.0.0.1:PORT`. For example, create two stereo tracks and capture the window:

  ```json
  {"id":1,"method":"engine.execute","params":{"command":"track.new","params":{"count":2,"format":"stereo"}}}
  {"id":2,"method":"ui.screenshot","params":{"path":"/tmp/soundcraft-window.png"}}
  ```

  Inspect each response for errors. Dispatch is in `crates/ui-egui/src/control.rs`,
  screenshot handling is in `crates/ui-egui/src/control/screenshots.rs`, and methods
  are documented in `docs/control-protocol.md`. A CLI request to the same app is:

  ```sh
  cargo run -p soundcraft-cli -- app --port PORT session.inspect
  ```

- **Verify UI changes with a whole-window image:** request `ui.screenshot`, then
  open and inspect the resulting PNG, including the relevant panels and dialogs.
  Capture requires a presented frame; a hidden window or locked screen may prevent
  it. The old comparison references under `plan/protools/screenshots/` are absent
  from this checkout. Do not claim comparison against them unless those files are
  available and have actually been inspected.

- **Render the whole UI offscreen when a window screenshot is unavailable:**

  ```sh
  soundcraft_check_dir=$(mktemp -d "${TMPDIR:-/tmp}/soundcraft-check.XXXXXX")
  cat > "$soundcraft_check_dir/script.jsonl" <<'JSONL'
  {"method":"engine.execute","params":{"command":"track.new","params":{"count":2,"format":"stereo"}}}
  {"method":"session.inspect","params":{}}
  {"steps":12}
  JSONL
  cargo run -p soundcraft-ui-egui --example ui_shot -- \
    "$soundcraft_check_dir/ui.png" "$soundcraft_check_dir/script.jsonl"
  ```

  This creates a fresh output directory and a script before running the example.
  The output PNG is the first argument; the optional JSON-lines script is second,
  before any flags. Omit the script argument entirely for an unmodified demo image.
  The implementation is `crates/ui-egui/examples/ui_shot.rs`; it silently treats an
  unreadable script as empty, so verify that your script exists and check the command
  responses. The sample script above should add two tracks to the nine-track demo.
  This uses the offscreen wgpu test harness and requires a working wgpu backend.
  It creates a separate app instance with the demo session by default and no audio
  device; it does not capture the current desktop session. Add `--blank` to start
  empty or `--size 960x600` to check a specific logical window size. Script lines
  accept control requests, `{"steps": 12}` to advance frames, and
  `{"shot": "/tmp/soundcraft-intermediate.png"}` to save an intermediate image.
  Use `shot` for harness captures, rather than the desktop viewport
  `ui.screenshot` request. Relative script and output paths resolve from the process
  working directory, not from the script directory. Create output parent directories
  first. Inspect the output for errors, confirm the PNG
  was created, and open it to verify the result; exit status alone is insufficient.

- **Verify session and audio output separately from the interface:**

  ```sh
  soundcraft_audio_dir=$(mktemp -d "${TMPDIR:-/tmp}/soundcraft-audio.XXXXXX")
  cargo run --release -p soundcraft-cli -- run --demo \
    --cmd 'mix.volume={"track":"Kick","db":-6}' \
    --start 0 --end 2 --bounce "$soundcraft_audio_dir/mix.wav" --inspect
  ```

  Bare numeric `--start` and `--end` values are seconds; this example renders two
  seconds. Session inspection and an offline bounce verify state and rendered audio, not
  UI appearance, interaction, device playback or recording. Inspect audio properties
  and listen when the requested change concerns audible behavior. Do not claim
  listening or real-time verification from a successful file write alone.

- **Choose the MCP runtime explicitly:**

  ```sh
  cargo run --release -p soundcraft-cli -- mcp --demo
  cargo run --release -p soundcraft-cli -- mcp --connect PORT
  ```

  The first starts an independent headless engine on stdio; the second controls
  the already-running desktop app and exposes its UI operations. Both are MCP
  servers using JSON-RPC on stdin/stdout, not interactive shell prompts; configure
  an MCP client to launch the selected command. Method/tool mappings are in
  `crates/automation/src/mcp/tool_calls.rs`; see `docs/mcp.md`.

- **Shell commands:** do not assume `mv` or `cp` is aliased. Check with `type mv cp`
  if behavior is unclear; do not add `-f` merely to bypass an assumed alias.

- **When using parallel agents:** assign separate `CARGO_TARGET_DIR` values and
  explicit file/crate ownership; write manifests atomically. Only remove a target
  directory after verifying that it was created exclusively for that agent's
  disposable build output and that no build is still using it. Do not remove shared
  or pre-existing targets.

## Roadmap
`ROADMAP.md` (committed) tracks status, milestones and estimates. Update it whenever a milestone lands.
