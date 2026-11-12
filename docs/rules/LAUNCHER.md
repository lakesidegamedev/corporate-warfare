# The Lakeside launcher handoff

How the desktop game (Windows and Linux, the Compose app) behaves when the Lakeside launcher
starts it. Code: `Launcher.kt`, `Main.kt`, `Updater.kt`. Tests: `LauncherTest`, `UpdaterDigestTest`.

## What the game promises

1. **Always offline.** Nothing in this page is needed to play. The game never asks whether you
   own it. Ownership gates the download and the launcher install only.
2. **An account is optional.** Without the launcher, or when the handoff fails in any way, the
   game carries on, silently, as if no account existed.
3. **No account control that does nothing.** The account row is hidden unless the launcher handed
   over a signed-in account.
4. **Telemetry is unchanged.** Anonymous snapshots with a random install id. The account id, the
   name and the handoff code are never in that payload (tested).
5. **Nothing is kept.** The handoff code is never logged or written to disk. The display name and
   account id live in memory for the session only. No tokens exist in v1.

## The contract (v1) as the game implements it

The launcher starts the game with environment variables:

| Variable | Meaning |
|---|---|
| `LAKESIDE_LAUNCHER=1` | The launcher started this copy. Anything else means it did not. |
| `LAKESIDE_LAUNCHER_VERSION` | The launcher's version, shown in Settings, Updates. |
| `LAKESIDE_HANDOFF_CODE` | Set only when the player is signed in. Single use, valid 60 seconds, bound to client id `corporate-warfare-desktop`. Ignored unless `LAKESIDE_LAUNCHER=1`. |

Redeem, once, at start:

```
POST https://lakesidegames.net/account/launcher/handoff/redeem
{"code": "<code>", "clientId": "corporate-warfare-desktop"}
-> 200 {"accountId": "...", "displayName": "...", "entitlements": [...]}
```

Rules the game follows: off the main thread, so the menu never waits; a 5 second limit for the
whole call; no redirects followed; never retried, even on failure, because the code is single use;
any non-200, malformed, slow or failed answer is "not signed in". The game reads `accountId` and
`displayName` only (name trimmed, control characters removed, cut at 40 characters) and ignores
`entitlements`. A reply with no account id is a failure. In-game sign-in without the launcher is a
later stage and is not built.

## When the launcher started the game

- The built-in updater is off: no check at launch, no update prompts, no channel or version
  picker. The first-run setup skips its Updates step. Settings, Updates reads "The Lakeside
  launcher keeps this copy up to date." and the main menu says the same.
- When signed in, the main menu and Settings show "Signed in as <name>" and a line that it came
  from the Lakeside launcher.

## The built-in updater (no launcher)

- It checks the public release list and, for a download, the SHA-256 GitHub publishes for the file
  (`digest: sha256:<hex>`). A mismatch deletes the file, installs nothing and says so.
- When a release publishes no digest (older releases, or another algorithm), it installs after the
  byte-count check only, and the update panel says "This release publishes no checksum, so only the
  file size is checked". Refusing would block going back to older versions.
- A release with no files at all, or without this platform's installer, shows "delivered through
  the Lakeside launcher or your store" rather than an error. Full releases are published that way
  (docs/RELEASING.md).

## What the Mac app needs (Swift, not built here)

The Mac app is a store or disk-image app, not started by the launcher on desktop in this version.
When the launcher installs and starts it on macOS, it needs the same, in `apple/`:
read `LAKESIDE_LAUNCHER`, `LAKESIDE_LAUNCHER_VERSION` and `LAKESIDE_HANDOFF_CODE` from
`ProcessInfo.processInfo.environment`; redeem once with `URLSession` (5 second timeout, no
redirects, no retry, off the main actor, code never logged); show the account row only on success,
display name and account id in memory only; hide any updater UI when `LAKESIDE_LAUNCHER=1`; keep
telemetry free of the account. The launcher must also pass the variables when it opens the app
bundle (`open --env` or by running the executable inside the bundle), which the Mac launcher side
has to confirm. iPhone and iPad stay store apps with no launcher.
