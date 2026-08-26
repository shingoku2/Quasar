# Release Signing & Auto-Updates

Quasar's release pipeline (`.github/workflows/release.yml`, triggered by pushing a `v*` tag)
builds signed, notarized installers for Windows and macOS, and produces signed update
artifacts that the in-app updater (Settings → About → "Check for Updates", and the
startup banner) verifies before installing.

None of this activates on its own — it depends on the secrets below being present in the
repository (**Settings → Secrets and variables → Actions**). Until they're added, the
Windows/macOS jobs still build unsigned installers, but `TAURI_SIGNING_PRIVATE_KEY` is
required unconditionally: `bundle.createUpdaterArtifacts` in `tauri.conf.json` is `true`
for every platform, so a release build with that secret missing will fail at the signing
step on all three OSes, not just fall back to unsigned.

## 1. Updater signing key (required for every platform)

The public half is already committed in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).
The matching private key was generated once for this repo and must be added as secrets —
it was **not** committed anywhere; whoever generated it is responsible for handing it to
you securely (it isn't recoverable from the public key).

| Secret | Value |
|--------|-------|
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of the private key file |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | The password chosen when the key was generated |

To rotate the key (e.g. if it's ever exposed): run `npx tauri signer generate -w quasar-updater.key`
locally, replace `pubkey` in `tauri.conf.json` with the new `quasar-updater.key.pub` contents,
update both secrets above, and ship the next release — older installs can no longer verify
future updates signed with a different key, so this is a breaking change for anyone who
hasn't updated past the last release signed with the old key.

## 2. Windows code signing

Requires an OV or EV code-signing certificate (from a CA such as DigiCert, SSL.com, etc.)
exported as a password-protected `.pfx`.

| Secret | Value |
|--------|-------|
| `WINDOWS_CERTIFICATE` | Base64 of the `.pfx` file (`certutil -encode cert.pfx cert.b64` on Windows, or `base64 -w0 cert.pfx` on Linux/macOS) |
| `WINDOWS_CERTIFICATE_PASSWORD` | The `.pfx` export password |

The workflow imports the certificate into the runner's certificate store and patches its
thumbprint into `tauri.conf.json` at build time (`certificateThumbprint` isn't settable via
an env var, unlike everything else here) — nothing to configure in-repo beyond the two
secrets above. This step is skipped entirely when `WINDOWS_CERTIFICATE` is unset, so the
Windows job still produces an (unsigned) build without it.

## 3. macOS code signing & notarization

Requires an active Apple Developer Program membership, a "Developer ID Application"
certificate, and an app-specific password for notarization.

| Secret | Value |
|--------|-------|
| `APPLE_CERTIFICATE` | Base64 of the exported `.p12` certificate (`base64 -i cert.p12 \| pbcopy`) |
| `APPLE_CERTIFICATE_PASSWORD` | The `.p12` export password |
| `APPLE_SIGNING_IDENTITY` | The certificate's common name, e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | Apple ID email used for notarization |
| `APPLE_PASSWORD` | An [app-specific password](https://support.apple.com/en-us/102654) for that Apple ID — not the account password |
| `APPLE_TEAM_ID` | Apple Developer Team ID (found at [developer.apple.com/account](https://developer.apple.com/account) under Membership) |

These are read directly by the Tauri bundler; unlike Windows, no `tauri.conf.json` field or
extra workflow step is needed. They're passed to every matrix job but are simply unused on
the Windows/Linux runners.

## 4. Linux

AppImage/`.deb` builds are not code-signed — Linux has no OS-level equivalent to Authenticode
or notarization gatekeeping. The updater still works there: `latest.json` and the signed
`.AppImage.tar.gz` are produced and verified the same way as on Windows/macOS.
