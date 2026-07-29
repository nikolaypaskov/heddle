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

**Linux, x86_64.** The same app, as an AppImage, built by CI on every release tag.

| File | What it is |
|---|---|
| `heddle-x86_64-unknown-linux-gnu.tar.gz` | Contains `Heddle-x86_64.AppImage`. |

Unpack it, make the AppImage executable, and run it. It is not signed — verify the published
SHA-256 before running it, or build from source (below).

```bash
shasum -a 256 -c heddle-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf heddle-x86_64-unknown-linux-gnu.tar.gz
cd heddle-x86_64-unknown-linux-gnu
chmod +x Heddle-x86_64.AppImage
./Heddle-x86_64.AppImage
```

### Two things to know before you download

- **The built-in AI agent does not work.** It was designed to talk to Warp's server, and this
  version has no server to talk to. You can still use **Claude Code, Codex, Gemini CLI** and
  similar — they run as ordinary programs in the terminal and are unaffected.
- **Apple Silicon Macs and x86_64 Linux only.** No Intel Mac, no Windows, no Linux on ARM.
  The macOS build is signed and notarized; the Linux AppImage is not signed at all — Linux has
  no equivalent of notarization here, and saying otherwise would overstate it.

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

## Finding out about new versions

Heddle asks **once**, on first run, whether it may check for updates — and does nothing until you
answer. Change your mind any time in **Settings → Privacy**.

If you say yes, it fetches a small file from this repository's releases over HTTPS. It sends no
account, no identifier and no usage data — there is nothing to send, and the request deliberately
does not use the app's normal HTTP client, which would have attached a client ID.

If you say no, it never contacts anything, and you can watch the
[releases page](https://github.com/nikolaypaskov/heddle/releases) instead.

Before anything is installed, a downloaded build must be notarized by Apple **and** signed by this
project's Developer ID — a validly-signed build from anyone else is refused — and its version is
read from the downloaded bundle itself, so a manifest cannot advertise one version and ship
another. You are told what is available before the download starts, not after.

Update notifications are macOS only. The Linux AppImage does not check for updates at all — watch
the releases page.

## What is not here

Warp's paid features ran on Warp's servers. Removing the sign-in screen does not move them to your
machine — **this is not a way to get Warp's paid plan for free.** What you get instead is a terminal
that works entirely on its own.

Removed: sign-in and accounts, Drive's cloud sync and sharing, usage analytics, crash reporting,
remote configuration, billing and upgrade prompts, and every `warp.dev` address.

**Drive itself stayed.** Its workflows, notebooks and environment variables are local information,
so removing the cloud sync it used to travel over did not mean removing the library. It is on by
default and everything in it lives on your machine.

## How much of it works today

| | |
|---|---|
| The terminal | ✅ Works |
| Themes, settings, keyboard shortcuts | ✅ Works |
| Claude Code, Codex and other CLI agents | ✅ Works |
| Drive — workflows, notebooks, environment variables | ✅ Works, stored on your machine |
| Warp's built-in AI agent | ❌ Needs Warp's server |
| Cloud sync and object sharing | ❌ Removed on purpose |
| Windows, Intel Mac, Linux on ARM | ❌ Not built |

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
cargo build --release -p warp --bin heddle \
  --features release_bundle,extern_plist,gui,nld_classifier_v3,nld_heuristic_v2
./script/heddle/verify-no-warp-endpoints target/release/heddle
```

That is the binary that ships. On Linux, `./script/bundle -c oss --packages appimage
--release-tag vX.Y.Z` wraps the same build into the published AppImage.

On macOS you also need the Metal Toolchain (`xcodebuild -downloadComponent MetalToolchain`).
Full instructions are in [CONTRIBUTING.md](CONTRIBUTING.md).

## More detail

- **[How it works](docs/HOW-IT-WORKS.md)** — what was removed and how, the server-supplied privacy
  setting that shaped the design, how the checks work and what they prove, and the bug pattern that
  cost the most time
- **[Questions](FAQ.md)** — is this legal, why not just use Warp logged out
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
