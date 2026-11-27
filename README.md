# لوحة المسار اليومي V6 (`abdullah-masar-v6`)

Offline-first Arabic (RTL) **Personal Discipline OS** built on **Tauri v2 + Rust + SQLite + Android Kotlin Foreground Service** with a zero-CDN Vanilla JS frontend.

## What's Inside V6
- **100% Offline-First, Zero External CDNs:** Runs completely without internet; local SQLite (`rusqlite` bundled WAL mode) in Tauri, plus a browser fallback (`BroadcastChannel` single-instance guard + `localStorage`) if you open `src/index.html` directly.
- **4:00 AM Logical Day Cutoff:** Between `00:00` and `03:59`, shows `🌙 امتداد يوم [...]` and keeps the active day locked until `04:00`.
- **Anti-Skip Discipline:** No free "Skip" button — tasks can only be **Completed**, **Rescheduled** (mandatory reason ≥3 chars logged in `reschedule_events`), or **Cancelled** (mandatory excuse category + text ≥5 chars logged in `cancellation_excuses`).
- **4-Layer Web Audio API Escalating Ring (`1–20+` Tasks):** Replaces V5's `نغمة الإنجاز اليوم` with a procedural 4-layer pentatonic synthesizer (`audio_ring.js`).
- **Preserved V5 Core:** 82 curated classical Arabic quotes, 5 Daily Prayers (45% weight), Fair Shift-Day Balance (45/55 + 10% Focus Bonus), Weekly Excuse Shield, Dual WhatsApp Export, `.ics` Export, Calm Mode (`🌙`), Zen Focus Studio (with `Date.now()`, 2-min auto-checkpoints, brown/rain/wind audio, Distraction Inbox), and One-Click V5 → V6 Migration.
- **Deterministic GitHub Gist Sync:** Conditional `GET (If-None-Match / ETag)` → Item-Level LWW + Append-Only Union Merge → `PATCH`.
- **Android Foreground Service Plugin:** `src-tauri/plugins/tauri-plugin-masar-timer` keeps the Zen Focus countdown alive with a persistent notification and `PARTIAL_WAKE_LOCK` (Android 14+ compliant).

## Quick Build Commands

### 1. Build Windows `.exe` / `.msi` Locally
Requires Node 20+, Rust (`rustup`), and Visual Studio C++ Build Tools:
```bash
npm install
npx tauri icon src-tauri/icons/icon.svg
npm run build
```
Output: `src-tauri/target/release/bundle/nsis/*.exe`

### 2. Build Android `.apk` Locally
Requires Android Studio SDK + NDK (`ANDROID_HOME`, `NDK_HOME`) + Rust Android targets:
```bash
rustup target add aarch64-linux-android armv7-linux-androideabi
npm install
npx tauri android init
npm run android:build
```

### 3. Automatic Cloud Builds (Easiest — Zero Local Setup)
Push this folder to a GitHub repo (`main` branch). The included `.github/workflows/build.yml` automatically compiles **both** the Windows `.exe` and Android `.apk` and attaches them as downloadable workflow artifacts.
