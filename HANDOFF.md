# HANDOFF — Laptop MCP Windows platform RESOLVED (2026-08-16 ~23:50 CEST)

**Subject project:** `/home/hendrik/Documents/rust/laptop-mcp` (source: `src/windows_sandbox.rs`)
**Session workspace:** `/home/hendrik/Documents/rust/libobs-rs` (this file lives here)

## STATUS: FIXED END-TO-END ✅

The Dockur Windows 11 sandbox platform now works: template built (VS Build Tools + OpenSSH), the
session overlay boots a companion, SSH becomes ready, and `sandbox_run platform=windows` executes
PowerShell commands over the SSH control channel (verified: exit 0, `whoami`, rule shows
`Domain,Private,Public`). Prep state: `ready` / `phase=ready` / 100%.

## Root causes (two, both fixed)

1. **OpenSSH firewall rule covered only Domain/Private.** Windows OpenSSH's built-in
   `OpenSSH-Server-In-TCP` rule applies to Domain/Private profiles. Every companion boots with a
   regenerated/“unidentified” network → **Public** profile → inbound 22 silently dropped.
   **Fix** (in `install.ps1`, source): idempotent
   `netsh advfirewall firewall add rule name="OpenSSH-Server-In-TCP" dir=in action=allow protocol=TCP localport=22 profile=any`.
   Also hardened `sshd_config` (`PasswordAuthentication no`), wrapped winget Git step in try/catch.

2. **Stale session overlay carried bad firewall state.** The session overlay
   (`.../v10-core-overhaul/windows-storage/data.qcow2`) had captured dirty sectors from
   pre-fix companion boots, permanently overriding the (later-fixed) template base → SSH kept
   failing with `kex_exchange_identification: read: Connection reset by peer` even after the base
   fix. **Recovery:** delete `windows-storage/` so `prepare_windows_session_storage` recreates a
   fresh overlay → first fresh-companion boot immediately got SSH. This is a one-off from the
   pre-fix era; fresh sessions start from the fixed base.

## Symptom → cause mapping (diagnostics you'll want again)
- SSH **timeout** on guest port 22 = firewall **DROP** (Public profile, rule not covering it).
- SSH **"Connection reset by peer"** at kex = guest **closed/rejected** port 22 (RST), NOT a
  forwarder issue. dockur/windows uses **pure iptables DNAT** (`QEMU_DNAT` chain, no socat);
  verify counters with `iptables -t nat -L -n -v` inside the container.
- Guest ping failing is NOT meaningful — Windows blocks ICMP on Public.

## Verified facts / current live state
- Windows account password = **bare** value `Lm7!WnFkjuwzrKwgTm4RtzXPBALrj31j`
  (`.../.cache/windows/password`). The old `+ "Password"` suffix theory is WRONG (SMB C$ login
  works with the bare password). Test SSH with `id_ed25519`, user `LaptopMCP`.
- Template dir: `~/.local/share/laptop-mcp/sandboxes/.cache/windows/templates/ad03b530d6359ea366c0/storage/`
  (data.qcow2 20.9 GB, ready file `dockur-v4-openssh-before-vs-buildtools` intact).
  Backup: `.../.cache/windows/tmpl-backup-ad03b530d6359ea366c0/`.
- Companion: `laptop-mcp-3f8b491a-v10-core-overhaul-windows` currently Up, SSH on a published port.
- Worker registration: workspaces are discovered from JSON files in `~/.local/share/laptop-mcp/instances/`
  filtered to live processes; there is NO add-workspace API. A dead worker drops out of the router
  registry (the router won't respawn it). Respawn manually:
  `/home/hendrik/.cargo/bin/laptop-mcp --repo /home/hendrik/Documents/rust/libobs-rs --worker-mode --background`
  (registers as `libobs-rs-3f8b491a`, binds 127.0.0.1:8771).

## Recommended next steps
- **Robustness fix (recommended):** make laptop-mcp recover from a permanently-bad session overlay —
  on companion SSH-wait failure, delete + recreate `windows-storage/` and retry once, instead of
  failing permanently. Without it, a session can be bricked by a bad overlay.
- **Minor:** `disable_windows_autologon` still doesn't persist (guest still auto-logs-in); unrelated
  to SSH, low priority.
- Re-test from scratch occasionally to make sure the source fix produces a working template without
  the manual overlay wipe.

## Key files / locations
- Source: `laptop-mcp/src/windows_sandbox.rs` (netsh fix ~1482-1487, companion boot ~613-747,
  wait_for_windows_ssh ~944, session storage ~561).
- Deployed binary: `/home/hendrik/.cargo/bin/laptop-mcp` (`cargo install --path . --force`).
- Session storage: `.../instances/libobs-rs-3f8b491a/v10-core-overhaul/windows-storage/`.
- Credentials/keys: `.../.cache/windows/{password,id_ed25519,id_ed25519.pub}`.
- MCP polling helper: `/tmp/opencode/mcp_call.sh` (re-initializes the session each call).