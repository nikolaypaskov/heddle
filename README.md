# Heddle

```text
     │  │  │  │  │  │  │  │
   ╔═╪══╪══╪══╪══╪══╪══╪══╪═╗
   ║ ●  │  ●  │  ●  │  ●  │ ║       h e d d l e  /ˈhɛd(ə)l/  n.
   ╚═╪══╪══╪══╪══╪══╪══╪══╪═╝       the loom component that lifts and separates
     │  │  │  │  │  │  │  │         the warp threads — the part that controls
     ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄       the warp
```

**A terminal with no account, no telemetry, and no company server behind it.**

Heddle is a fork of the [Warp](https://github.com/warpdotdev/Warp) terminal with everything
commercial taken out. There is no sign-in, nothing is uploaded, and Warp's web addresses are not
in the app at all — not switched off, not present.

---

## ⬇ Download

### **[Get the latest release →](https://github.com/nikolaypaskov/heddle/releases/latest)**

**macOS, Apple Silicon (M1 and later).** Signed with an Apple Developer ID and notarized by Apple,
so it opens with a double-click — no security warning to click past.

| File | What it is |
|---|---|
| `Heddle-aarch64-apple-darwin.app.zip` | **The app. Start here.** |
| `heddle-aarch64-apple-darwin.tar.gz` | Terminal-only version, runs in an existing window |

Unzip it with **Archive Utility** (double-click) or `ditto`, then drag `Heddle.app` to your
Applications folder. Avoid third-party unzip tools — some strip the Apple signature, after which
macOS refuses to open the app.

<details>
<summary>Prefer the command line? Verify the download and unpack it here</summary>

```bash
shasum -a 256 -c Heddle-aarch64-apple-darwin.app.zip.sha256
ditto -x -k Heddle-aarch64-apple-darwin.app.zip .
open Heddle.app
```

</details>

### Two things to know before you download

- **The built-in AI agent does not work.** It was designed to talk to Warp's server, and this
  version has no server to talk to. You can still use **Claude Code, Codex, Gemini CLI** and
  similar — they run as ordinary programs in the terminal and are unaffected.
- **Apple Silicon Macs only.** No Intel Mac, no Windows. Linux is built automatically but not
  published as a ready-to-run download yet.

---

## What you get

| | |
|---|---|
| **No account** | Nothing to sign up for, nothing to sign in to |
| **No telemetry** | Nothing about how you use it leaves your machine |
| **No cloud** | Your history, settings and sessions stay on your computer |
| **Free and open** | AGPL-3.0. Read it, change it, build it yourself |

Everything else is the Warp terminal you may already know: the same blocks, the same editor, the
same keyboard shortcuts.

## What is not here

Warp's paid features ran on Warp's servers. Removing the sign-in screen does not move them to your
machine — **this is not a way to get Warp's paid plan for free.** What you get instead is a terminal
that works entirely on its own.

Removed: sign-in and accounts, Warp Drive cloud sync, usage analytics, crash reporting, remote
configuration, billing and upgrade prompts, and every `warp.dev` address.

## How much of it works today

| | |
|---|---|
| The terminal | ✅ Works |
| Themes, settings, keyboard shortcuts | ✅ Works |
| Claude Code, Codex and other CLI agents | ✅ Works |
| Warp's built-in AI agent | ❌ Needs Warp's server |
| Cloud sync / Warp Drive | ❌ Removed on purpose |
| Windows, Intel Mac | ❌ Not built |

Heddle is a terminal today, not an AI environment. A replacement agent that talks to a local
program instead of a company server is [designed but not
built](docs/HOW-IT-WORKS.md#the-agent-story); until it is finished and safe, it will not be
described as working.

## Is this really private?

It is checked automatically, not just claimed. Every time the code changes, a program reads the
raw bytes of the built app and fails the build if any Warp address, credential or analytics
destination appears in it:

```console
$ ./script/heddle/verify-no-warp-endpoints
PASS: no Warp endpoints, credentials, or telemetry destinations found.
```

[![privacy gate](https://github.com/nikolaypaskov/heddle/actions/workflows/heddle-privacy-gate.yml/badge.svg)](https://github.com/nikolaypaskov/heddle/actions/workflows/heddle-privacy-gate.yml)
[![license: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-8250df)](LICENSE-AGPL)

**Being straight about the limits:** this proves those addresses are not in the app. It does not
prove the app opens no connections at all — a service *you* configure yourself is contacted
because you asked for it. Fully proving network silence needs system-call tracing that has not
been done yet, and that is stated plainly rather than glossed over.

## Build it yourself

You do not have to trust the download. It is the same code:

```bash
cargo build --release -p warp_tui --bin heddle-tui
./script/heddle/verify-no-warp-endpoints target/release/heddle-tui
```

On macOS you also need the Metal Toolchain (`xcodebuild -downloadComponent MetalToolchain`).
Full instructions are in [CONTRIBUTING.md](CONTRIBUTING.md).

## More detail

- **[How it works](docs/HOW-IT-WORKS.md)** — what was removed and how, the server-supplied privacy
  setting that shaped the design, how the checks work and what they prove, and the bug pattern that
  cost the most time
- **[Questions](FAQ.md)** — is this legal, why not just use Warp logged out, what about updates
- **[Contributing](CONTRIBUTING.md)** — building, testing, and what changes are in scope
- **[Reporting a security issue](SECURITY.md)**

## Licence

AGPL-3.0, inherited from Warp and unchanged. The `warpui` and `warpui_core` crates remain MIT, as
upstream licensed them.

Copyright © 2026 Denver Technologies, Inc. Modified work © 2026 Heddle contributors.

"Warp" is a trademark of Denver Technologies, Inc. Heddle is an independent fork and is **not**
affiliated with, endorsed by, or supported by them. Please report Heddle problems
[here](https://github.com/nikolaypaskov/heddle/issues), never to Warp.

---

```text
┄┄┄┄┄┄┄ the warp is only half the cloth ┄┄┄┄┄┄┄
```
