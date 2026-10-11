# Rift

Fast, lightweight, native dual-pane file-transfer client for local files and remote files over SSH/SFTP. Rust + GPUI, WinSCP-inspired workflow, Zed-aligned visual design.

Left pane browses the local filesystem; right pane browses a remote server over SFTP. Panes navigate independently, with transfer queue, bookmarks, connection profiles, and OS-backed credential storage.

## Features

- Dual-pane local/remote browsing with breadcrumbs, history, and quick path jump
- Sorting (name, extension, size, modified, permissions), filtering, hidden-file toggle
- Upload/download queue with progress, speed, cancellation, and retry-friendly errors
- SSH host-key verification with explicit trust flow (no silent auto-trust, no trust-on-mismatch)
- Saved connection profiles; passwords only in the OS credential store, never in config files
- Session and persistent bookmarks, guarded for disconnected remote targets
- Keyboard-first: command palette, shortcuts, context menus with accurate hints

## Architecture

| Module | Responsibility |
|---|---|
| `ui` | GPUI components, layout, keyboard input, rendering (`pane_view`, `dialogs`, `drawer`, `top_bar`, `context_menu_view`) |
| `core` (in `app.rs`) | Application state, navigation, async orchestration |
| `filesystem` (`fs/`) | Local listing/sorting/filtering, cross-platform path utils |
| `sftp` (`remote/`) | SSH sessions, SFTP ops, host-key verification, profiles, credentials |
| `transfers` (`transfer/`) | Bounded-concurrency queue, progress, cancellation, history |
| `bookmarks` | Session + persistent bookmarks with render-safe caching |
| `config` | JSON profiles/bookmarks under the OS config dir (see below) |
| `platform` | OS credential store via `keyring`, native path handling via `Path`/`PathBuf` |

## Keyboard shortcuts

| Shortcut | Action |
|---|---|
| `Tab` | Switch active pane |
| `F5` | Transfer selected to other pane |
| `F7` | New folder |
| `F8` / `Delete` / `Shift+Backspace` | Delete selected |
| `F2` | Rename |
| `Ctrl+P` / `F1` | Command palette |
| `Ctrl+F` / `/` | Filter files |
| `Ctrl+L` | Edit path |
| `Ctrl+R` | Refresh pane |
| `Ctrl+H` | Toggle hidden files |
| `Ctrl+A` | Select all |
| `Ctrl+C` | Copy focused/selected path |
| `Ctrl+B` | Bookmarks |
| `Ctrl+Shift+B` | Add bookmark for current path |
| `Ctrl+T` | Toggle dark/light theme |
| `Ctrl+J` | Transfer drawer (via palette: toggle transfers) |
| `Backspace` | Parent directory |
| `Alt+Left` / `Alt+Right` | Back / forward |
| `↑` / `↓` (+`Shift`) | Move cursor (extend selection) |
| `Home` / `End` / `PgUp` / `PgDn` | Jump |
| `Enter` | Open directory |

## Benchmarks

All numbers measured, not estimated. Profile: `dev` (unoptimized + debuginfo) unless noted. Method: a scratch integration test driving rift's own `LocalFs`, `sort_entries`, `normalize_path`, and `RemoteClient` against temp dirs and a loopback OpenSSH server (deleted after capture; reproduce with `cargo test -- --nocapture` on an equivalent scratch file).

### Environment

| Item | Value |
|---|---|
| OS | Arch Linux, kernel 7.2.8, x86_64 |
| CPU / RAM | 8 cores / 15 GiB |
| Toolchain | rustc 1.98.1 |
| SSH server | System OpenSSH, `internal-sftp`, ed25519 loopback on 127.0.0.1 |
| Cipher/conditions | Loopback (no real network); 64 KiB transfer chunks |
| Note | Box load averaged ~8.6/8 during runs, so ranges below reflect real variance; re-run on an idle machine for tighter numbers |

### Local directory listing (`LocalFs::read_dir`, name sort, dirs-first)

| Entries | Time |
|---|---|
| ~1,020 (1,000 files + 20 dirs) | 2.7–2.9 ms |
| ~5,100 (5,000 files + 100 dirs) | 10.0–14.1 ms |
| ~20,400 (20,000 files + 400 dirs) | 36.5–59.1 ms |

Scales roughly linearly (~1.8–2.9 µs/entry); listings run synchronously for local paths and stay far below a frame budget at typical directory sizes.

### Sorting 20,000 entries (`sort_entries`, avg of 2 runs)

| Column | Time | Notes |
|---|---|---|
| Name | 2.1–4.7 ms | Case-insensitive, dirs-first |
| Size | 2.4–5.4 ms | Numeric, no allocation |
| Extension | 35–82 ms | Known hotspot: lowercase alloc per comparison; only matters at 10k+ rows, per-refresh cost is sub-ms at typical sizes |

A decorate-sort-undecorate variant was tried and reverted: its setup allocations made the common Name/Size cases slower in dev builds. Deterministic name tiebreak keeps repeated sorts stable regardless of OS enumeration order.

### Path utilities

| Operation | Throughput |
|---|---|
| `normalize_path` (mixed drive/UNC/POSIX sample, 200k iters) | 0.9–1.6 µs/op |

### SFTP over loopback (`RemoteClient` → local `sshd`)

| Operation | Result |
|---|---|
| Connect + key auth + known-hosts trust | 153–161 ms |
| `list_dir`, 2,000 entries (fetch + parse + sort) | 15–27 ms |
| Upload 10 MiB | 0.12–0.18 s → 55–86 MiB/s |
| Upload 50 MiB | 0.58–0.70 s → 72–86 MiB/s |
| Download 10 MiB | ~0.39 s → ~26 MiB/s |
| Download 50 MiB | 1.55–1.90 s → 26–32 MiB/s |

Loopback numbers measure protocol + client overhead (64 KiB chunks, per-chunk progress callback), not network speed. Downloads trail uploads here due to synchronous per-chunk progress dispatch; both saturate typical broadband many times over. WAN numbers depend on latency/bandwidth and are left for operators to measure — see `docs`/reproduce method above.

### Build & test

| Metric | Value |
|---|---|
| Cold `cargo build` (wall, 8 cores, loaded box) | 2 m 07 s |
| Warm `cargo test` (26 tests) | ~0.5 s |
| Debug binary size | ~543 MB (GPUI + deps, unoptimized; release stripping not measured yet) |
| Clippy | 0 warnings (`cargo clippy --all-targets`) |

## Platform support

| Platform | Status | Notes |
|---|---|---|
| Linux (KDE/GNOME) | Primary | Absolute paths, mount points, dotfile hiding, Unix permission display |
| Windows 10/11 | Primary | Drive letters, UNC paths, reserved-name validation; hidden-attribute filtering is a known gap (dot-prefix rule is Unix-only) |
| macOS | Secondary | Builds where GPUI + deps allow; not regularly tested |

## Config & secrets

| Data | Location | Plaintext secrets? |
|---|---|---|
| Connection profiles | OS config dir (`ProjectDirs`), `profiles.json` | Never — metadata only |
| Persistent bookmarks | OS config dir, `bookmarks.json` | n/a (paths + labels) |
| Passwords/passphrases | OS credential store via `keyring` (Credential Manager / Secret Service / Keychain) | Never on disk; never logged |
| SSH host keys | `~/.ssh/known_hosts` | n/a; mismatches always refuse, unknown keys require explicit trust |

## Transfer safety

- Local copies never silently overwrite: existing destinations and directories are skipped and counted in the status message.
- Remote operations validate paths; failures report per-file errors instead of vanishing.
- Cancellation aborts worker loops promptly; a cancelled item stays `Cancelled` even if the worker was mid-completion.
- Concurrency is bounded (default 2) via semaphore; queued items wait instead of thundering the server.

## Getting started

Prerequisites: Rust stable (1.88+ for let-chains-era syntax used in deps), system OpenSSL/SSH client libs per `ssh2`/`keyring` platform notes.

```sh
cargo build        # debug binary at target/debug/rift
cargo run          # launch the app
cargo test         # 26 tests, offline, no server needed
cargo clippy --all-targets  # must stay at 0 warnings
```

Connect: `+ Connect SFTP` (or `Ctrl+P` → *Connect to SFTP Server*), pick password / private key / SSH agent auth, verify the host key on first connect. `F5` transfers the selection to the opposite pane.

## Known limitations

- SFTP sessions share one blocking handle per connection; large listings briefly serialize concurrent transfers (per-transfer sessions are the planned fix).
- No recursive directory transfer yet — directory entries are skipped with a count, not silently half-copied.
- No transfer conflict-resolution queue yet (overwrite/skip/rename prompts); the safe defaults are skip + report.
- Windows hidden-attribute files ignore the hidden toggle (Unix dot-rule only).
- Release-binary size and WAN throughput are not measured yet.
