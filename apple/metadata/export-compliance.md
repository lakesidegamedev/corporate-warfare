# Export compliance determination (code-based draft)

The app's only network connection is an HTTPS POST of anonymous market analytics through
Apple's URLSession, which uses the operating system's TLS. Using only the encryption built into
the operating system is exempt. The app contains no encryption code of its own. It uses BLAKE3
hashing for save integrity and canonical state hashes; hashing is not encryption and does not
require an export determination.

Proposed Info.plist value: `ITSAppUsesNonExemptEncryption = false`.

Owner action: confirm this against the final archive and answer the App Store Connect question.
If any networking, authentication, payment or encrypted-storage dependency is added, revisit this
file before submission.
