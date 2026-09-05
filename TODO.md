# libobs-rs feature status / remaining work

This file tracks meaningful library gaps rather than listing every libobs symbol that could someday
be wrapped. Features marked implemented are available through safe `libobs-wrapper` APIs or an
opinionated `libobs-simple` workflow. Plugin/frontend-only functionality is intentionally out of
scope unless it is useful to embedders.

## Implemented application foundations

- **Reliable output lifecycle** — bounded pause/resume/stop waits, serialized lifecycle operations,
  one overall stop deadline, explicit timeout/disconnect errors, and force-stop recovery.
- **Replay saving** — subscribe-before-save, serialized saves, bounded waits, completed-path lookup,
  and native replay artifact validation.
- **Runtime capability discovery** — sources, filters, transitions, outputs, encoders, services,
  protocols, modules, settings metadata, compatibility planning, and descriptor-driven creation.
- **Dynamic settings** — recursive property metadata, ranges/lists, visibility/enabled state,
  current/default values, validation, modified callbacks, editable lists, fonts, frame rates,
  checkable groups, paths and colors.
- **Instance property actions** — enabled button properties can be invoked on the concrete owning
  source/filter/output/encoder/service; metadata/settings are refreshed afterward. URL buttons are
  returned to the application rather than opened implicitly.
- **Hotkeys** — managed frontend/source/output/encoder/service registrations, bounded callback
  delivery, binding save/load, safe unregistration, and typed synthetic OBS hotkey injection.
- **Scene composition** — nested scenes, native groups, native ordering, transforms, crop, bounds,
  scale filters, blending, visibility/locking, and explicit replacement semantics when ungrouping.
- **Scene collection persistence** — versioned JSON, persistent source UUIDs, shared-source identity,
  filters/groups/order/transforms/settings/hotkeys, strict/best-effort restore diagnostics, unknown
  field preservation, portable secret/path redaction, atomic save, and undo/redo for supported
  document edits.
- **Transitions and media** — managed transition begin/wait/force-stop semantics plus capability-aware
  media play/pause/restart/stop/seek/state/duration controls.
- **Preview/editor geometry** — program/scene/source targets, Fit/Fill/Stretch mapping, physical vs
  logical/DPI coordinate conversion, resize support, and managed render target lifetimes.
- **Audio configuration** — public startup audio info, typed mixer indices/masks and output slots,
  per-source volume/mute/sync/routing/monitoring, managed faders/meters, global monitoring-device
  discovery/selection, and multi-track recording routing.
- **Encoding diagnostics** — strict/permissive effective settings reports, hardware preference vs
  requirement vs explicit backend selection, conservative software fallback, attempt diagnostics,
  output health snapshots, dropped-network-frame reporting, and restart-aware bitrate sampling.
- **Common file/stream outputs** — simple recording, RTMP streaming, replay buffer, FFmpeg muxer and
  OBS output discovery allow applications to use installed HLS/MPEG-TS/fragmented formats when the
  corresponding OBS plugin exposes them.

## Partially covered / worthwhile follow-up

- **Profile management compatible with OBS Studio frontend files** — `SceneCollectionDocument`
  intentionally persists the libobs-rs managed model, not OBS Studio's complete frontend profile or
  scene-collection file format.
- **Missing-file recovery UI/workflow** — persistence detects obvious missing file/path settings and
  reports them, but automatic search/relink and backup rotation remain application concerns.
- **Hotkey activation contexts / richer input mapping** — registrations and native binding
  persistence are present; higher-level conditional contexts, mouse-button UX, and key-name helper
  catalogs could be added.
- **Audio panning / surround helpers** — arbitrary native mixer routing and startup speaker layouts
  exist, but ergonomic balance-law/EQ/ducking helpers are not yet provided.
- **Output telemetry** — bytes, frames, reconnecting/error state, network drops and sampled bitrate
  are exposed. Rendering/encoding lag metrics that are not output-plugin counters need separate
  global/video statistics APIs.
- **Transition templates / quick transitions** — transition source control exists; frontend-style
  named template management remains an application-level feature.
- **Source interaction events** — lower-level libobs interaction APIs (mouse/keyboard/touch sent into
  interactive sources) still need a safe managed wrapper if embedders require them.

## Still missing high-value wrappers/workflows

- WHIP/WebRTC convenience output and service setup.
- HLS/adaptive-streaming convenience builders beyond generic output discovery/configuration.
- Explicit stream-delay configuration helpers.
- Automatic ingest discovery and service-specific Twitch/YouTube/Amazon IVS helpers beyond generic
  OBS service/plugin settings.
- Show/hide transition helpers attached directly to scene items.
- Deinterlacing convenience APIs.
- Rich missing-file relink/search workflow and collection backup rotation.
- Higher-level source interaction/input forwarding APIs.
- Additional encoder-specific convenience tuning where generic settings metadata is insufficient.

## Platform-specific follow-up

- Continue real-device/runtime validation on Windows capture backends (game/window/display capture),
  including GPU/hardware-encoder fallback paths on representative NVIDIA/AMD/Intel machines.
- Continue macOS runtime/capture validation on physical CI/hardware. The repository currently checks
  the wrapper/simple crates and macOS source contracts against an official OBS framework, but Linux
  and Windows have broader native integration coverage.
- Linux PipeWire portal behavior depends on a desktop portal/session and cannot be fully exercised by
  a plain Xvfb CI environment; X11 capture and software encoding are covered there.
