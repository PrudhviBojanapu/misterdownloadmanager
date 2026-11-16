# Mister Download Manager — lean Windows fork

<p align="center">
  <img src="docs/images/logo.png" alt="Mister Download Manager's logo" width="128">
</p>

**Windows x64 · Microsoft Edge · English · Metro theme**

This is the personal, streamlined
[`PrudhviBojanapu/misterdownloadmanager`](https://github.com/PrudhviBojanapu/misterdownloadmanager)
fork of [Ahmed Nefzaoui's Mister Download Manager](https://github.com/anefzaoui/misterdownloadmanager).
It is **not an official upstream release**. The upstream website, browser stores and
WinGet package distribute upstream, not this fork.

**[Download this fork](https://github.com/PrudhviBojanapu/misterdownloadmanager/releases/latest)**
· **[CLI and automation](docs/command-line.md)**
· **[AI assistants / MCP](docs/ai-assistants.md)**
· **[Release and signing details](docs/windows-release.md)**

The goal is less optional UI and background machinery, **not a slower downloader**.
The optimized Cargo release profile is retained. The existing window-activation and
duplicate-download fixes are kept, including reuse of the duplicate-download dialog.

## What stays

- **Segmented HTTP/HTTPS downloads**, adaptive connections, pause/resume, retries,
  integrity checks and per-download/global bandwidth controls.
- **The download list, categories, properties and basic queues**, including ordered
  downloads and queue concurrency. Start and stop queues manually or through automation.
- **Microsoft Edge integration** through the bundled Chromium extension and native
  messaging host; capture rules, cookies, proxies and the video button remain.
- **Video and audio downloads**, playlists and subtitles with yt-dlp/ffmpeg;
  **image galleries** with gallery-dl.
- **Torrents and magnet links**, file selection, download/seeding controls and
  **TorBox** account integration.
- **Sites Logins**, proxies, encrypted DNS, network-adapter binding and VPN-only
  downloads, with their existing safety controls.
- **Local CLI/API/MCP automation**, settings and download management, and
  **paired remote control**. The app, CLI, MCP server and native host are all packaged.
- **Portable mode**, keyboard accessibility and screen-reader support.

See the [command-line guide](docs/command-line.md) for the extended typed automation
operations and [MCP guide](docs/ai-assistants.md) for assistant setup. Sensitive
credentials should be supplied through the documented local/standard-input mechanisms,
not placed in shell history.

## What is removed or out of scope

| Area | Lean profile |
| --- | --- |
| Release platforms | Windows x64 only; no Linux/macOS, ARM64 or 32-bit release builds. |
| Browser packages | Microsoft Edge only; no Firefox/Safari or browser-store bundles. |
| Appearance | English and Metro only; no language picker, alternate themes or custom-theme controls. |
| Transfer protocols | HTTP/HTTPS, FTP/FTPS and torrents. |
| Debrid providers | TorBox retained; Real-Debrid, AllDebrid, Premiumize and Mega-Debrid removed. |
| Optional automation | No RSS feed management, Click'n'Load listener, automatic archive extraction, timed scheduler or quota/usage-limit automation. Basic queues remain. |
| Desktop extras | No clipboard-triggered automatic starts, floating drop target, sounds or server-country/geolocation indicators. |
| Release pipeline | No CI test execution, geolocation table generation, crash-symbol publishing, redundant plain ZIP or store deployment. |

Some upstream help pages, source crates and developer utilities remain for reference
and easier rebasing. Their presence does **not** make a removed feature or another
platform part of this fork's supported profile. Upstream screenshots may also show
controls that are absent here.

## Download and install

On the [fork's release page](https://github.com/PrudhviBojanapu/misterdownloadmanager/releases/latest):

| File | Purpose |
| --- | --- |
| `MisterDownloadManager-<version>-setup.exe` | Installs the app, CLI, MCP server, native host and English Edge extension. The upgrade AppId is unchanged. |
| `MisterDownloadManager-<version>-portable.zip` | Unzip and run; the included `data` folder keeps settings and download state beside the programs. Browser-host registration requires setup. |
| `MisterDownloadManager-<version>-extension-chromium.zip` | Standalone English extension for Edge's **Load unpacked**. |
| `SHA256SUMS.txt`, optional `.sig`, `SOURCE.txt` | Checksums, optional fork-key signature, exact source commit and build correspondence. |

The portable ZIP contains the same four programs as setup. There is no separate
`x64.zip`. Old plain-ZIP installations should switch manually to setup or portable.

### Set up Microsoft Edge

1. Install with **Set up browser integration (Microsoft Edge)** selected.
2. Open `edge://extensions` and enable **Developer mode**.
3. Choose **Load unpacked** and select `extension\chromium` inside the installation
   folder, or the extracted standalone extension package.
4. Keep that folder in place. The extension's existing ID is retained so it can
   communicate with the bundled native messaging host.

The setup registers Microsoft's Edge host key only, not Chrome, Brave or Firefox.
An unsigned setup may trigger Windows SmartScreen; verify its source and checksum
before deciding whether to run it.

### Updates and trust

The in-app updater targets **this fork**, not upstream, and still requires a valid
signature over the release checksum list followed by a matching download hash.
The fork's pinned public key and matching Actions signing secrets are configured.
Its fingerprint is `E7029305EBD69B1B16FB13960AC0D17698AC4E20`. Builds containing this
trust root can accept later fork releases with matching signatures; unsigned,
tampered or upstream-signed releases are rejected. Install the first trust-root
build manually if using upstream or an older fork build. Never bypass signature
verification to make updates work.

[Release documentation](docs/windows-release.md) explains the public-key bootstrap,
`MDM_RELEASE_KEY`/optional passphrase secret, optional Windows Authenticode certificate,
checksums, licenses and GitHub build provenance.

## CLI and AI assistants

Examples from a terminal in the installation folder:

```powershell
.\misterdownloadmanager-cli.exe get https://example.com/file.iso -o D:\Downloads
.\misterdownloadmanager-cli.exe api on
.\misterdownloadmanager-cli.exe status
.\misterdownloadmanager-cli.exe settings
.\misterdownloadmanager-cli.exe queues
```

The standalone downloader works without the desktop app. Commands controlling the
app require it to be running and its local automation API to be enabled.
`misterdownloadmanager-mcp.exe` exposes the app to compatible assistants; API/MCP
operations retain validation and local authentication rather than bypassing them.

- [Full command-line, JSON and typed API reference](docs/command-line.md)
- [MCP setup and supported assistant operations](docs/ai-assistants.md)
- [Remote control](docs/remote-control.md)
- [Portable mode](docs/portable.md)

## Build and package

Requires Rust stable **1.98+**, the Windows MSVC build tools, CMake, LLVM/libclang
and NASM for the browser-like TLS connection. Packaging needs **Inno Setup 6.3+**;
Node.js and Python run the focused package checks.

```powershell
rustup target add x86_64-pc-windows-msvc
.\tools\installer\build.ps1 -Arch x64
node tools\extension\check-pack.mjs
python tools\installer\check-pack.py target\dist
```

Outputs are in `target\dist`. `-NoBuild` packages already-built x64 release binaries.
For extension-only packaging, run `.\tools\extension\pack.ps1`.
Local tests can still be run as needed; the release workflow does not execute them.
Inherited upstream workflow jobs are repository-gated and do not run tests,
multi-platform builds, website deployments or other maintenance jobs in this fork.

The **Windows release** workflow has one Windows build job. A manual run retains
artifacts for seven days; a `vX.Y.Z` tag matching `Cargo.toml` publishes a release.
Its cache restores Cargo dependencies/compiler outputs by toolchain, configuration
and lockfile, then saves under a commit-specific key. Dist archives and a whole
`target` tree are not cached. See [release details](docs/windows-release.md).

## Upstream maintenance

The intended branch shape is **one custom fork commit ahead of the selected upstream
commit**. Maintaining that shape means fetching upstream, rebasing the fork changes,
resolving conflicts, validating and squashing the custom changes again. It is a
maintenance convention, **not automatic upstream tracking or a guarantee of
conflict-free upgrades**. New upstream changes are not adopted until that work is done.

Keep the fork profile and security checks explicit when rebasing: removed features
should not silently return, and updater trust must stay tied to the fork's publisher.
The upstream repository remains the source for upstream history, general architecture,
benchmarks and broader-platform development.

## Credits and license

Mister Download Manager is an independent open-source download manager by
**Ahmed Nefzaoui**. This fork retains upstream copyright notices and attribution.
It is not affiliated with Tonec Inc. or Internet Download Manager.

Licensed under **GPL-3.0-or-later**; see [LICENSE](LICENSE) and
[ATTRIBUTION.md](ATTRIBUTION.md). Source corresponding to every release is identified
by its `SOURCE.txt` metadata and repository tag/commit.
