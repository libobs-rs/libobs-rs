# libobs-rs improvement roadmap status

This file is the durable implementation and verification record for the reliability/API/workflow roadmap completed on 2026-09-05. An item is marked complete only when the public behavior, failure semantics, tests, documentation, and applicable native validation are accounted for.

## Baseline and validation environment

- [x] Read the repository instructions, `README.md`, `docs/api_orientation.md`, `docs/how_it_works.md`, and the wrapper/simple crate documentation before editing.
- [x] Preserved the clean sandbox baseline at `f5d2ce610fbf2066b19b8f7dfbd94f85784d892f`; no unrelated source-checkout changes were imported.
- [x] Kept the repository's pinned libobs ABI requirement intact.
- [x] Built the pinned OBS Studio 32.1.0 release from source into the non-committed `/workspace/obs-install` test runtime and used its `libobs.pc`, plugins, data files, and helper executables for native validation.
- [x] Ran Linux graphics/capture tests under Xvfb using the real OBS OpenGL path (Mesa llvmpipe in this sandbox).

## Phase 1 — reliable output lifecycle

### Replay saving

- [x] Subscribe to the native `saved` signal before initiating the replay-buffer save procedure.
- [x] Serialize overlapping saves so a successful return is associated with the initiating request.
- [x] Use one bounded deadline that includes lock acquisition and native completion, with explicit inactive/procedure/timeout/runtime errors.
- [x] Cover synchronous completion, overlap/serialization, lock timeout, and missing completion deterministically.
- [x] Validate native replay saving against OBS 32.1.0 and confirm the returned media path exists after the completion signal.

### Output deadlines/completion

- [x] Expose configurable pause, unpause, graceful-stop, and force-stop deadlines.
- [x] Make already-applied pause/stop states idempotent and serialize overlapping lifecycle/configuration operations.
- [x] Use one overall deadline for multi-stage stop (`stop` plus `deactivate`).
- [x] Document that a timeout only bounds the caller's wait and does not claim to cancel native work.
- [x] Expose force-stop as the explicit data-loss-tolerant recovery path.
- [x] Cover subscribe-before-initiate completion, missing completion, queue/runtime failure, repetition, and bounded lock contention.

## Phase 2 — audio configuration and mixing

### Startup audio

- [x] Expose `StartupInfo::set_audio_info` and requested startup configuration inspection.
- [x] Expose typed `ObsAudioInfo` fields and `ObsContext::audio_info()` for the effective native configuration.
- [x] Native-test 48 kHz stereo/fixed buffering. OBS 32.1.0 correctly preserved the format/policy while normalizing the requested 50 ms fixed buffer to its effective 64 ms value.

### Safe mixer controls

- [x] Add typed source linear volume, mute, sync offset, mix routing, and monitoring mode APIs.
- [x] Add managed fader and volume-meter objects with callback userdata retained through deregistration/destruction.
- [x] Add global monitoring-backend/device discovery and selection.
- [x] Distinguish `AudioMixIndex`/`AudioMixMask` from `AudioEncoderSlot` at the type level.
- [x] Add a multi-track audio example/workflow.
- [x] Native-test combined plus separate audio tracks and inspect the recorded file's audio streams.

## Phase 3 — encoding and diagnostics

### Effective encoder configuration reports

- [x] Report selected output/video/audio plugin IDs, applied/ignored settings, fallback reasons, and creation/start attempts.
- [x] Support permissive and strict configuration modes.
- [x] Apply backend-aware generic preset/settings handling with deterministic policy tests.

### Encoder selection/fallback

- [x] Distinguish automatic hardware preference, required hardware, and explicit backend selection.
- [x] Bound managed creation/start fallback and record each attempt while safely rebuilding failed candidate graphs.
- [x] Restrict start fallback to encoder/backend availability or initialization failures; path, permission, disk, connection, authentication, and stream-key failures are not retried as encoder fallback.
- [x] Add deterministic fallback-policy tests and migration/API notes.

### Output status/statistics

- [x] Expose active/paused/reconnecting/error state, bytes, output frames, and network drops with known-vs-indeterminate metric semantics.
- [x] Add a restart/reset-aware `OutputBitrateSampler`.
- [x] Add deterministic sampler/counter tests and the `examples/output-health` workflow.

## Phase 4 — application workflows

### Scene persistence/history

- [x] Add a versioned managed scene-collection format covering shared sources, filters, scenes, groups, native order, transforms/state, settings, hotkey snapshots, and persistent source UUIDs.
- [x] Preserve unknown JSON fields and report missing plugins/files with strict and best-effort restore modes.
- [x] Add explicit portable redaction for likely secrets and absolute machine paths plus atomic file replacement.
- [x] Add document history/undo-redo for supported item state, ordering, grouping, and ungrouping/replacement semantics.
- [x] Cover JSON/version/malformed/reference/redaction/atomic-save/history behavior and native round-trip/shared-identity/group restoration; add `examples/scene-collection`.

### Transitions and media

- [x] Add managed transition current/in-flight destination retention, non-blocking `begin_to`, bounded completion waits, blocking convenience, reconciliation, and explicit force-stop semantics.
- [x] Add capability-aware media play/pause/restart/stop/next/previous/seek/state/position/duration APIs.
- [x] Native-test natural transition completion and same-thread non-blocking interruption against OBS 32.1.0.
- [x] Native-test real FFmpeg media opening/state/pause/play/restart/stop and deterministic exact seek delivery through a registered controllable-media fixture. Media seek documentation explicitly reflects libobs's asynchronous/plugin-specific completion semantics.

### Preview integration

- [x] Add managed program/scene/source preview targets.
- [x] Add Fit/Fill/Stretch physical-pixel geometry, logical/DPI pointer conversion, resize semantics, and geometry round-trip tests.
- [x] Update the preview example and native-test a real OBS display/render target for five seconds under Xvfb.

### Settings actions and hotkeys

- [x] Add instance-bound source/filter/output/encoder/service button invocation with enabled/type checks and post-action metadata/settings refresh.
- [x] Surface URL buttons to the application instead of opening external URLs implicitly.
- [x] Add managed frontend/source/output/encoder/service hotkey registration, bounded event delivery, native binding snapshot save/load, safe unregistration, and typed OBS event injection.
- [x] Native-test property invocation through a deterministic registered OBS filter callback and frontend hotkey binding/event delivery without depending on plugin GUI teardown.

## Phase 5 — docs and verification

- [x] Replace the stale symbol-level `TODO.md` list with implemented foundations, partial follow-up, high-value gaps, and platform-specific remaining work.
- [x] Reconcile root/wrapper/simple README platform claims with current CI: Linux and Windows have the broadest native runtime coverage; macOS CI prepares the official OBS framework and validates compilation/source contracts but has narrower real capture/runtime coverage.
- [x] Update API orientation, v10 migration notes, crate READMEs, and examples for the new lifecycle, audio, diagnostics, persistence, media/transition, hotkey/action, and preview APIs.
- [x] Keep custom Dylint safety/runtime annotations clean for the new FFI callbacks and deferred actor cleanup paths.

### Final Linux verification against OBS 32.1.0

The following commands passed on the final tree with `PKG_CONFIG_PATH`/`LD_LIBRARY_PATH` pointing at `/workspace/obs-install` and `DISPLAY=:99` for native graphics tests:

- [x] `cargo fmt -- --check`
- [x] `git diff --check`
- [x] `cargo clippy -- -D warnings`
- [x] `cargo dylint --all --workspace`
- [x] `cargo hack check --feature-powerset --skip generate_bindings,include_win_bindings,__test_environment` — all 80 generated feature combinations passed.
- [x] `cargo nextest r --profile ci --no-default-features --features "__test_environment"` — 160/160 tests passed, 2 profile-skipped, no flaky retries.
- [x] `cargo nextest r --profile ci --all-features --target-dir target/nextest-all-features` — 160/160 tests passed, 2 profile-skipped, no flaky retries.
- [x] `cargo test -p libobs-simple --features __test_environment --test gpu_integration_preview -- --nocapture` — real OBS X11 display/preview render path passed under Xvfb.

Expected native-environment warnings during the headless Linux runs included unavailable NVENC hardware, unavailable PipeWire portal capture, Xvfb's lack of EWMH, and Mesa software rendering. Those optional environment limitations did not prevent the corresponding software/native lifecycle, recording, replay, scene, audio, media/transition, managed-control, or preview validation above.

## Remaining scope

The roadmap above is complete. Remaining feature/product work is intentionally tracked in [`TODO.md`](../TODO.md), notably OBS Studio frontend-profile compatibility, richer missing-file relink UX, source interaction forwarding, additional telemetry, convenience builders for newer streaming transports/services, and broader physical-device/macOS runtime validation.
