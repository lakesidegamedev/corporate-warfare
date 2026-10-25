# Export compliance determination (code-based draft)

The shipped app makes no network connections and contains no encryption or cryptographic
communication code. It uses BLAKE3 hashing for save integrity and canonical state hashes; hashing
is not encryption and does not require an export determination.

Proposed Info.plist value: `ITSAppUsesNonExemptEncryption = false`.

Owner action: confirm this against the final archive and answer the App Store Connect question.
If any networking, authentication, payment or encrypted-storage dependency is added, revisit this
file before submission.
