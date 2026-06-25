# Cortex Mobile — native Android shell

A thin Android **WebView shell** that wraps the Cortex mobile web client
(`app/src/mobile_bridge/client.html` + the vendored xterm bundle) into an installable,
self-contained APK. It is the native milestone after the PWA-ify work: the web client
is now offline-capable (no CDN), so the shell just bundles it and loads it over a
**secure** local origin.

This project is **not** part of the Cortex Rust workspace — it's a separate Gradle/Kotlin
build living at `cortex/mobile/`. It reads (does not duplicate) the web client straight
from `../app/src/mobile_bridge/` at build time, so the APK and the desktop bridge can
never drift.

> **Status:** **on device** (2026-06-24). The APK installs, pairs, and mirrors live panes on
> the Pixel; iteration is now per-feature build-and-sideload. A cold command-line
> `assembleDebug` produces a 3.7 MB `app-debug.apk` with the web client bundled (no
> dependency/version nudge needed). Use [`build-apk.ps1`](build-apk.ps1) for the one-line
> build (it pins the right JDK + invocation).
>
> Recent input fix: the soft keyboard now rises on the **first** tap of the compose box after
> a cold launch (it previously took two). See [Soft keyboard on first tap](#soft-keyboard-on-first-tap).

---

## Why a native shell (vs. just "Add to Home Screen")

Loading the bundled client through `WebViewAssetLoader`'s
`https://appassets.androidplatform.net/` virtual host makes the page a **secure context**,
which natively unlocks the two web APIs that no-op over plain-http Tailscale:

- `navigator.wakeLock` — keep the screen on while mirroring a pane.
- `navigator.clipboard` — real copy-out (instead of the `execCommand` fallback).

The desktop bridge still speaks plain `ws://` over the tailnet (already WireGuard-encrypted),
so the WebView is configured with `mixedContentMode = ALWAYS_ALLOW` to permit that one
insecure socket from the secure page. Net: secure-context APIs **and** a plain-ws bridge.

The shell also adds: hardware **Back** = close the top overlay (drawer/settings/loading)
via the client's `history` guard, then exit; QR/manual **pairing** so a fresh install
knows which desktop to reach; and edge-to-edge so the client's safe-area insets apply.

---

## Prerequisites

1. **Android Studio** (latest stable). It bundles the Android SDK, `adb`, and a compatible
   JDK (JBR 21).
   - This machine's system JDK is **25**, which AGP 8.7.3 + Gradle 8.11.1 **reject** (confirmed).
     Build with Android Studio's embedded JBR instead: in Studio set **Settings → Build,
     Execution, Deployment → Build Tools → Gradle → Gradle JDK = (Embedded JDK)**; on the CLI,
     point `JAVA_HOME` at `…\Android Studio\jbr`. `build-apk.ps1` does this for you.
2. **Generate the Gradle wrapper jar.** Only `gradle-wrapper.properties` is committed (the
   `.jar` is a binary we don't check in). On first **Open** + Gradle sync, Android Studio
   regenerates it automatically. (Or, with a standalone Gradle installed:
   `gradle wrapper --gradle-version 8.11.1`.)

## Build

```powershell
# Windows — pins the JBR + the full-path gradlew invocation this box needs:
pwsh mobile\build-apk.ps1
# → app/build/outputs/apk/debug/app-debug.apk  (~3.7 MB)
```

```bash
# macOS / Linux (or Windows once JAVA_HOME points at the JBR):
./gradlew assembleDebug
```

The `syncWebAssets` Gradle task copies `client.html`, `xterm.js`, `xterm.css`, the Cortex
`icon.png`, and `manifest.webmanifest` into the app's assets before each build — so a
client edit in the Rust repo flows into the next APK with no manual copy.

## Install on the Pixel (sideload)

```bash
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

…or copy the APK to the phone and tap it (enable *Install unknown apps* for your file
manager). Single user, no Play Store.

## Pair the app

First launch opens the pairing screen. Either:

- **Scan a QR** (recommended). Accepted payloads:
  - `cortex://pair?host=<H>&port=<P>&token=<T>` (optional `&label=<L>&secure=1`)
  - `http://<H>:<P>/?token=<T>` — the exact URL the bridge already serves
  - `{"host":"<H>","port":"<P>","token":"<T>"}`
- **Type it** — host (Tailscale MagicDNS name or `100.x` address), port (**9280** dev /
  **9278** prod), token.

The token is stored only in the app's private storage; it is never in the APK or the repo.

> **Desktop QR generator is a follow-up.** Until the desktop shows a pairing QR, generate
> one yourself from a `cortex://pair?…` or `http://<host>:<port>/?token=…` string (any QR
> generator), or just use manual entry.

**Re-pair / switch desktop:** open the web client's settings menu (the ⚙ in the sidebar)
and tap **"Scan QR / Re-pair…"** — it relaunches the native QR scanner via the shell's
`window.CortexShell.rescan()` hook, and on a successful scan the shell reloads the client
pre-paired to the new desktop. (That button is shown only inside the native shell — a plain
browser, which has no native scanner, hides it.) You can still edit host/port/token by hand
in the same menu, or clear the app's storage to force the first-run pairing screen.

---

## How it fits together

```
PairingActivity ──(host/port/token)──▶ PairingStore (SharedPreferences)
                                              │
MainActivity ── WebView ── WebViewAssetLoader │  loads
   https://appassets.androidplatform.net/assets/client.html?host=&port=&token=
        (secure context; mixedContentMode=ALWAYS_ALLOW)
              │
              └── client.html  ── ws://<host>:<port>/ws?token=  ──▶  desktop bridge
                  seedFromPairingQuery() reads ?host=&port=&token= and connects
```

Files:

- `app/src/main/java/com/cortex/mobile/MainActivity.kt` — WebView host, asset loader,
  mixed-content + zoom config, back-button → overlay close, `CortexShell` JS interface.
- `app/src/main/java/com/cortex/mobile/PairingActivity.kt` — QR scan (ZXing) + manual entry.
- `app/src/main/java/com/cortex/mobile/PairingStore.kt` — persisted pairing + appassets URL.
- `app/src/main/res/xml/network_security_config.xml` — permit cleartext (ws://) to the
  tailnet (WireGuard already encrypts it).
- `app/build.gradle.kts` — `syncWebAssets` copy task (single source of truth for the client).

### Soft keyboard on first tap

After a **cold launch**, the Android WebView used to skip the soft keyboard on the *first* tap
of the compose box — the user had to tap twice. The cause is a WebView IME race: the `<textarea>`
does receive DOM focus on the first tap (the shell pre-focuses the WebView container in
`onPageFinished` / `loadClient` / an `ACTION_DOWN` touch listener), but on the very first focus
the WebView's native input connection isn't live yet at the moment it would raise the IME, so
`showSoftInput` is effectively dropped. Every later tap works because the connection now exists.

Fix — the web client tells the shell to raise the keyboard the instant the box gains focus:

- `client.html` adds a `focus` listener on `#compose-input` that calls
  `window.CortexShell.showKeyboard()` (guarded on the bridge existing, so a plain browser / PWA
  keeps its own IME).
- `MainActivity.ShellBridge.showKeyboard()` runs on the UI thread and posts
  `InputMethodManager.showSoftInput(webView, SHOW_IMPLICIT)` immediately **plus one ~60 ms retry**
  to win the first-focus connection race. `SHOW_IMPLICIT` is a no-op when a hardware keyboard is
  attached, so it never pops the on-screen keyboard wrongly.

It's idempotent, so taps after the first (already working) are unaffected. Verify on a *true*
cold start: `adb shell am force-stop com.cortex.mobile.debug`, reopen, tap the compose box once.

## Versions (bump during first sync if Studio flags them)

AGP 8.7.3 · Gradle 8.11.1 · Kotlin 2.0.21 · compileSdk/targetSdk 35 · minSdk 26 ·
androidx.webkit 1.12.1 · zxing-android-embedded 4.3.0.
