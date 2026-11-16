# Windows releases of this fork

The `Windows release` workflow builds **Windows x64, English, Microsoft Edge only**.
There is one Windows build job and no CI test execution, geolocation generation,
symbol upload, browser-store deployment or other-platform packaging. The optimized
Cargo release profile is unchanged.

Inherited CI, multi-platform release, website, screenshot and repository-maintenance
jobs are explicitly gated to `anefzaoui/misterdownloadmanager`, so fork pushes/tags
cannot start their runners. Only the fork's Windows release job builds here; no
upstream-sync automation merges commits into the fork.

A manual run retains a `windows-x64` artifact for seven days. A `vX.Y.Z` tag matching
the workspace version publishes:

- `MisterDownloadManager-X.Y.Z-setup.exe`
- `MisterDownloadManager-X.Y.Z-portable.zip`
- `MisterDownloadManager-X.Y.Z-extension-chromium.zip` (unzip, then Edge **Load unpacked**)
- `SHA256SUMS.txt`, optional `SHA256SUMS.txt.sig`, and `SOURCE.txt`

The setup and portable archive retain the app, native messaging host, CLI, MCP server,
English Edge extension, license and attribution. Setup retains the existing upgrade
AppId and registers only Microsoft's Edge native-host registry key. The fork's native
host restricts `--register` and startup `--repair` to Edge; migration removes old
non-Edge entries only when they point at this installation. The portable copy registers no host.
The plain `x64.zip`, Firefox/Safari packages and store bundles are no longer produced.
Old plain-zip installations must switch manually to setup or portable.

`SOURCE.txt` identifies the exact repository commit, source archive and workflow run.
Checksums cover all payloads and this metadata. Tagged releases also carry GitHub build
provenance attestations. Package-content validation checks ZIP integrity, x64 programs,
English-only extension resources and required licenses without running application tests.

## Required setup for automatic updates

Self-updates target
[`PrudhviBojanapu/misterdownloadmanager`](https://github.com/PrudhviBojanapu/misterdownloadmanager/releases),
not upstream. Changing the publisher must not trust upstream's release signing key.
`crates/tools/keys/fork-release.asc` contains this fork's public signing key:
**`E7029305EBD69B1B16FB13960AC0D17698AC4E20`**. Matching owner-repository Actions
secrets `MDM_RELEASE_KEY` and `MDM_RELEASE_KEY_PASSPHRASE` are configured. The encrypted
private-key backup is retained outside the checkout in a restricted maintainer session
folder; its passphrase backup uses Windows DPAPI for that Windows account. Back these
up securely before deleting the session or Windows profile, and keep an offline copy.
No private key or plaintext passphrase belongs in this repository.

The signing configuration proof fixture is signed by the actual fork key, and the
production verifier checks it. It deliberately contains no asset checksums and cannot
authorize a download. Unsigned releases can be installed manually; checksums alone
never authorize an automatic update.

## Automatic app and Edge updates

The app checks this fork's latest GitHub release at startup and once a day while running.
With **Help > Check for updates > Automatically update app and extension** enabled
(the default), a newer signed release is downloaded and installed automatically.
Installation/restart waits for 30 continuous seconds without active downloads/seeding,
unfinished running queues or file moves/renames. Failures appear in the existing update
window; signature or checksum failures never install anything. **Later/Cancel** cancels
that attempt. `control {"op":"set_settings","patch":{"update_check":false}}` disables
automatic checking/installation; manual **Update now** remains available.

The installer and portable package include the matching English Edge extension.
Load the installed app's `extension\chromium` folder once in Edge's developer mode.
After an app update replaces those files, the extension checks its own local manifest
every five minutes (and when its worker starts), waits for a stable newer version, then
reloads itself with its settings intact. A separate copy extracted elsewhere from the
extension ZIP is not updated by the app; switch it once to the bundled folder.

This deliberately does not pretend an `update_url` works for unpacked extensions.
Microsoft requires an Active Directory domain for self-hosted CRX installs on Windows;
ordinary Windows PCs must use Edge Add-ons for independent store-managed updates.
No browser policy is changed, no browser protection is bypassed, and no remote code is
executed by the extension. The signed app release is the update trust boundary.
Older installed builds need a one-time bootstrap installation of this fork's new setup
(or portable ZIP), since their old key/build/version cannot authenticate this trust root.
Reload the extension once in `edge://extensions` (or restart Edge) after that first upgrade
so its new automatic-reload code is running.
All-users installations may still require Windows' UAC approval for an upgrade.

To reproduce the setup or deliberately rotate the signing key:

1. Generate a fork-owned OpenPGP signing key offline, for example with GnuPG:
   `gpg --quick-generate-key "MDM fork releases" ed25519 sign 2y`.
   The verifier accepts v4 binary-document signatures using SHA-256/SHA-512 and
   RSA or legacy Ed25519/EdDSA (algorithm 22); do not use v6 keys, algorithm 27
   signatures or text-mode signatures.
2. Export its **public** key with `gpg --armor --export <fingerprint>` and replace
   the entire `crates/tools/keys/fork-release.asc` file. Commit only the
   public key. Distribute this trust-root build manually first; already-distributed
   older upstream-key or placeholder-key builds cannot learn a key from an untrusted release.
3. Store the ASCII-armored private key (`gpg --armor --export-secret-keys <fingerprint>`)
   in the repository Actions secret **`MDM_RELEASE_KEY`**. For a passphrase-protected
   key, also set **`MDM_RELEASE_KEY_PASSPHRASE`**. Never commit either secret.
4. The workflow imports the pinned public key into an isolated keyring and signs
   `SHA256SUMS.txt` with that fingerprint. A missing/mismatched private key or invalid
   public key fails signing, rather than producing a misleading signature. A missing
   `MDM_RELEASE_KEY` explicitly produces an unsigned, manual-update-only release.
5. Test a signed release with the distributed trust-root build before relying on
   automatic updates. Keep the matching key backed up offline; changing it requires
   a separately authenticated trust-root update.

The historical `mdm-release.asc` key remains only for upstream signature fixtures.
It is not the production self-update trust root. The `test-update` feature's test-key
override remains test-only and is not enabled by the release build. Verification still
fails closed if the signing secret is removed, mismatched or a downloaded asset is changed.

## Optional Windows Authenticode

Set **`CODESIGN_PFX_BASE64`** and **`CODESIGN_PASSWORD`** to sign the application
programs, setup and uninstaller. The portable archive then contains those same signed
programs. Without this certificate the workflow warns and builds unsigned binaries;
SmartScreen may warn too. Authenticode is separate from the mandatory signature
verification performed by the self-updater.

Signing material and the imported certificate are removed in an always-run cleanup
step and are never included in the artifacts or Cargo cache.

## Local packaging

```powershell
.\tools\installer\build.ps1 -Arch x64
node tools\extension\check-pack.mjs
python tools\installer\check-pack.py target\dist
```

Use `-NoBuild` to package existing x64 release binaries. Inno Setup 6.3+ is required.
`tools/extension/pack.ps1` can independently build the English Edge extension.
Cargo cache entries restore by toolchain/configuration/lockfile, then save compiler
outputs under a commit-specific key. Dist archives and whole target trees are not cached.
