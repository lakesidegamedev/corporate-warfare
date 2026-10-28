# Notes for Review

Paste into App Store Connect → App Review Information → Notes for the first submission, and
update it whenever the notes would otherwise describe an older build.

Corporate Warfare is a single-player economic simulation. No account is created and no sign-in
is required: everything the reviewer sees is local to the device.

- Opening the app goes to the main menu. Start a new career, choose a scenario and difficulty,
  then use the guided walkthrough. The scored route ends at game week 260 (five in-game years);
  the same career can be continued afterwards as an open-ended sandbox.
- Nothing in the app is purchased. There are no in-app purchases, subscriptions, advertisements,
  loot boxes or user-generated content.
- The only network request during play is an anonymous market snapshot every five game weeks,
  an HTTPS POST of simulated economy figures to https://ops.lakesidegames.net/api/cw/telemetry.
  It contains no name, account, contact or device identifier, only a random install id. It is on
  by default and can be switched off under Settings ("Share anonymous market data"). The update
  check on the desktop and direct-download builds reads the public GitHub releases feed; the App
  Store build does not update itself.
- Saves are written to the app container and can be exported from the Saves screen; deleting the
  app deletes them.
- No demo account is needed because there are no accounts.

Review device guidance: the game is landscape and portrait on iPhone and iPad; the map is the
home screen, Markets holds the exchange, and the CEO Office holds pricing and divisions.
