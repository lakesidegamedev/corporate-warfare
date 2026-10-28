# Store declarations (draft for owner review)

One description of what Corporate Warfare collects, mapped onto each store's form so the answers
match the published privacy policy word for word. Source text: `apple/metadata/privacy-policy.md`.
Nothing here is submitted or published without the owner. Issues: #131 (App Store compliance),
#134 (privacy page), #140 (Play app content).

## What the app actually sends

| Flow | When | Contents | Identifier | Off switch |
|---|---|---|---|---|
| Market snapshot | every five game weeks | the simulated economy (goods made, sold, imported, exported, held, prices) and the player's company results | random install id, created on first launch, not tied to any account or device id | Settings (Portfolio, Settings on iPhone and iPad; Settings, Analytics elsewhere) |
| Update check (Windows, Linux, direct-download Android only) | at launch | nothing: a read of the public GitHub release list | none | Settings, Updates |

Endpoint for the snapshot: `https://ops.lakesidegames.net/api/cw/telemetry` over HTTPS. No
advertising, third-party analytics or crash-reporting SDK. No account, login, payment or contact
data.

## Privacy page (#134)

Publish the text of `apple/metadata/privacy-policy.md` as its own page, for example
`https://lakesidegames.net/corporate-warfare/privacy/`, with the review contact added. The store
answers below quote that page; change one, change all.

## App Store Connect, App Privacy (#131)

- Data collection: Yes.
- Data type: Gameplay Content.
- Linked to the user: No. Used for tracking: No.
- Purpose: Analytics.
- Matches `apple/Sources/PrivacyInfo.xcprivacy` (Gameplay Content, not linked, not tracking,
  Analytics; UserDefaults reason CA92.1).
- Age rating, content rights and export compliance: see `apple/metadata/age-rating-notes.md` and
  `apple/metadata/export-compliance.md`. Content rights: maps from US Census TIGER and Natural
  Earth (public domain); cloud imagery from Solar System Scope (CC BY 4.0, from NASA Blue Marble,
  credited in Settings, About); Silkscreen font under the SIL Open Font License; A House Divided
  sector data reused by the owner's grant (DECISIONS, 2026-10-26).

## Google Play, Data safety (#140)

- Does the app collect or share user data: Collects; does not share.
- Data types:
  - App activity, Other actions: the in-game economy and company results. Collected, not shared,
    optional (the player can turn it off), purpose Analytics.
  - Device or other IDs: the random install id. Collected, not shared, optional, purpose
    Analytics.
- Processed ephemerally: No. Encrypted in transit: Yes (HTTPS).
- Data deletion: there is no account and the id is random and not linked to the player, so there
  is nothing to request deletion against; turning the snapshot off stops collection. State this in
  the policy page.

## Google Play, other app content (#140)

- Ads: the app contains no ads.
- App access: all functionality is available without login or special access.
- Target audience: 13 and over (a business simulation; keeps it outside the Families policy).
- Content rating (IARC): no violence, sexual content, drugs, gambling or real-money purchases, no
  user-generated content or chat; expect Everyone or Teen.
- Financial features: not applicable. The game simulates trading and lending with in-game money
  only and moves no real money.
- News, health, government, COVID-19 declarations: No.
