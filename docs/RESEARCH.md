# Corporate Warfare source research

Research date: 2026-10-25. Scope: official geography, Apple distribution, and Rust/Swift packaging facts needed to audit PLAN.md. These are findings for planning, not evidence that the project already builds or ships. Recheck mutable Apple requirements before selecting the release runner and before upload.

## 1. BEA geography requires a correction

**Verified:** BEA's November 17, 2004 definition contains **179 economic areas**. It uses February 2004 OMB statistical areas and Census 2000 population. Its published roster includes these relevant areas:

| EA code | Official name |
| --- | --- |
| 029 | Charleston, WV |
| 033 | Cincinnati-Middletown-Wilmington, OH-KY-IN |
| 034 | Clarksburg, WV+Morgantown, WV |
| 035 | Cleveland-Akron-Elyria, OH |
| 040 | Columbus-Marion-Chillicothe, OH |
| 044 | Dayton-Springfield-Greenville, OH |
| 047 | Detroit-Warren-Flint, MI |
| 060 | Fort Wayne-Huntington-Auburn, IN |
| 078 | Indianapolis-Anderson-Columbus, IN |
| 129 | Pittsburgh-New Castle, PA |
| 166 | Toledo-Fremont, OH |

Cleveland and Akron are therefore one EA. Canton (Ohio), Youngstown, Lima, Findlay and Wheeling do not appear as standalone EA names in this roster. This does **not** establish their county assignments. [BEA 2004 release and roster](https://www.bea.gov/news/2004/new-bea-economic-areas-2004).

**Verified:** BEA built these areas by assigning counties to 344 component economic areas and aggregating those into 179 EAs. The hierarchy distinguishes counties, CBSAs, CSAs, component EAs and EAs. The method uses commuting, geographic size and other links; it aims to limit cross-boundary commuting. BEA's article says comprehensive county/hierarchy code files were available on its regional website. [BEA methodology, November 2004, pp. 68-71](https://apps.bea.gov/scb/pdf/2004/11November/1104Econ-areas.pdf).

**Unresolved:** A usable official county-to-EA file was not recovered in this research pass. The release points to the historical `www.bea.gov/regional/docs/econlist.cfm`; that www URL returned 404 and the apps counterpart was inaccessible through the browsing tool. Do not invent mappings from metropolitan names, infer membership from a visual map, or silently substitute current CBSAs. No claim is made here that a newer official EA delineation does or does not exist.

**Planning inference:** The proposed list of cities is not yet a verified list of 13-14 BEA regions. Choose either the actual EA geography or explicitly named custom game regions. Freeze a county FIPS crosswalk, vintage and checksum before balancing the corridor. If using 2004 boundaries with contemporary data, specify how county-equivalent changes and splits are reconciled. Preserve display names separately from immutable region IDs. Build-time county aggregation does not require county entities in runtime simulation.

**Required evidence before calling the dataset ready:** official source or provenance-traceable archived crosswalk; complete county coverage for the declared geography; one destination per source county or a documented split rule; stable EA IDs; no duplicate area membership; exact slice IDs; reconciliation report for source-year boundary changes; and totals reconciled to source data. Until then the table above is a candidate roster, not the final corridor.

## 2. Apple build and TestFlight gates

**Verified as checked on 2026-10-25:** App Store Connect uploads have required Xcode 26 or later and the iOS 26 SDK since April 28, 2026. Apple's current requirements also state an iOS/iPadOS deployment floor of 13 since September 9, 2026. SDK version and minimum supported runtime are separate choices. [Apple upcoming requirements](https://developer.apple.com/news/upcoming-requirements/).

Apple's upload help explicitly applies the Xcode 26 build requirement to iOS TestFlight delivery. The bundle ID and version associate a build with its App Store Connect record; the build string uniquely identifies a build. Upload processing is a separate step before the build appears. [Apple upload builds](https://developer.apple.com/help/app-store-connect/manage-builds/upload-builds/).

**Verified:** Apple's Xcode compatibility table pairs each Xcode release with supported macOS versions, SDKs and deployment targets. For example, the page currently lists Xcode 26.6 with macOS Tahoe 26.2 through 26.x. This is an example from a mutable table, not a pinned project toolchain. [Apple Xcode system requirements](https://developer.apple.com/xcode/system-requirements/).

**Planning inference:** iOS 17+ remains compatible with the cited upload floor, but must be checked against the selected supported Xcode release. A macOS executor with matching Xcode/SDK is required for the supported archive and simulator workflow. Linux Rust tests cannot establish an iOS archive, SwiftUI correctness, signing, device energy use, or A15 performance. Secure the Mac build path early, rather than leaving it as final console setup.

**Verified:** Apple Developer Program membership includes TestFlight distribution. A free developer account supports direct device testing through Xcode, but is not the same distribution entitlement. [Apple program benefits](https://developer.apple.com/programs/).

**Verified:** Create the App Store Connect app record before uploading; the Account Holder must accept the latest agreement. [Apple app record setup](https://developer.apple.com/help/app-store-connect/create-an-app-record/add-a-new-app). App Store Connect provisioning uses an explicit App ID matching the bundle ID and a distribution certificate; automatic signing can manage distribution profiles. [Apple provisioning](https://developer.apple.com/help/account/provisioning-profiles/create-an-app-store-provisioning-profile). Treat team selection, final bundle ID and signing authority as owner inputs, and keep credentials out of the repository.

**Verified:** TestFlight builds last up to 90 days. Internal testing allows up to 100 App Store Connect users; external testing allows up to 10,000 people. External testing requires the first build to pass Beta App Review, and later builds may require review. Test information, feedback email, export-compliance determination and valid provisioning identifiers are part of the workflow. [Apple TestFlight overview](https://developer.apple.com/help/app-store-connect/test-a-beta-version/testflight-overview/).

**Planning inference:** Define two release outcomes separately: (1) reproducibly archived, signed, exportable build plus complete submission material, pending named owner credentials/actions; (2) processed build installed and launched on a real iPhone via TestFlight. External beta acceptance is an additional Apple-controlled gate. Do not mark the second outcome complete with an unsigned simulator build or an upload script that has never run.

## 3. Distribution material and privacy

**Verified:** Apple requires a privacy policy URL for apps, and App Store privacy answers must include third-party practices. Apps without collection can select the corresponding no-collection answer. [Apple manage app privacy](https://developer.apple.com/help/app-store-connect/manage-app-information/manage-app-privacy/). Apple defines collection in terms of transmitting data off the device for access beyond servicing the immediate request; on-device-only processing is not collection under that definition. [Apple privacy details](https://developer.apple.com/app-store/app-privacy-details/).

**Verified:** Privacy manifests use `PrivacyInfo.xcprivacy`; apps and SDKs must describe relevant collection and required-reason API usage. Merely using an XCFramework does not establish which declarations are appropriate. [Apple privacy manifests](https://developer.apple.com/documentation/bundleresources/privacy-manifest-files). Uploads using required-reason APIs without declarations have been rejected since May 1, 2024. [Apple required-reason API guidance](https://developer.apple.com/documentation/bundleresources/describing-use-of-required-reason-api).

**Planning inference:** Offline gameplay simplifies the privacy posture but does not settle optional iCloud sync, crash reporting, analytics or dependencies. Prepare About/privacy screens, policy URL, beta description, What to Test, review contact, age-rating answers, export-compliance answers, icons and release notes. Audit the actual archive and dependencies before final declarations. Do not predeclare encryption exemptions or no collection without reviewing shipped behavior.

## 4. UniFFI is bindings, not Apple packaging

**Verified:** UniFFI supports Swift bindings and explicitly distinguishes bindings generation from shipping a library to platforms. [Mozilla UniFFI introduction](https://mozilla.github.io/uniffi-rs/). Swift output includes generated Swift code plus C headers/module map; Swift 6 concurrency support introduces Sendable obligations, including foreign trait implementations. [Mozilla Swift bindings](https://mozilla.github.io/uniffi-rs/latest/swift/overview.html).

**Verified:** `uniffi-bindgen-swift` supports separate Swift/header/modulemap generation and XCFramework-compatible modulemaps. [Mozilla Swift generator](https://mozilla.github.io/uniffi-rs/latest/swift/uniffi-bindgen-swift.html). UniFFI's lifting/lowering model includes serialization for complex types. [Mozilla FFI internals](https://mozilla.github.io/uniffi-rs/latest/internals/lifting_and_lowering.html).

**Planning inference:** The plan's mention of SoA slices must not be interpreted as guaranteed zero-copy Swift snapshots. Begin with small owned snapshot DTOs, measure bytes and allocations at the FFI boundary, and keep tick timings separate from encoding, FFI conversion and main-thread view updates. A serial host queue must actually own command/tick ordering; Sendable is not a substitute for this contract.

**Verified:** Rust's iOS targets distinguish device ARM64, simulator ARM64 and simulator x86_64. They require the corresponding Xcode-provided SDK. `aarch64-apple-darwin` is not an iOS simulator target. Rust respects `IPHONEOS_DEPLOYMENT_TARGET`; tests require a device or simulator environment. [Rust iOS target support](https://doc.rust-lang.org/rustc/platform-support/apple-ios.html).

**Verified:** Apple XCFrameworks package distinct platform/library variants, including simulators, and may contain static libraries and headers. [Apple XCFramework guide](https://developer.apple.com/documentation/xcode/creating-a-multi-platform-binary-framework-bundle). Mozilla's own application-services workflow builds an XCFramework plus generated Swift sources, and its remote Swift package artifact uses a checksum. [Mozilla iOS packaging workflow](https://mozilla.github.io/application-services/book/howtos/locally-published-components-in-firefox-ios.html).

**Planning inference:** An early packaging milestone should prove ARM64 device and simulator artifacts, generated Swift wrapper compilation, matching deployment targets, Swift 6 concurrency checks, simulator load/tick/save, and a real-device archive. Add macOS or Intel simulator variants only when required. Pin Rust, UniFFI generator/runtime, Xcode and package dependencies together, regenerate bindings in CI, and test stale/mismatched artifacts as release failures.

## 5. Open verification register

- Geography: recover and validate the official crosswalk, establish exact region list, select data years, and inspect suppression/missing-cell behavior for each source.
- Data rights: verify each downloaded product's terms, notices and embedded third-party content. The plan's blanket public-domain assertion has not been audited here.
- Apple account: confirm active membership, legal agreements, owning team, app name/bundle ID and roles. No account was accessed in this research.
- Apple executor: select actual Mac/CI access, pin supported toolchain, establish signing storage and credential scope. None was provisioned here.
- Release: establish internal versus external beta target, encryption determination, actual privacy manifest and review metadata.
- Packaging: choose a pinned UniFFI release and validate generated output on that exact version. `latest` documentation is a research reference, not a dependency pin.
- Device proof: select available iPhone model, OS and measurement protocol; no hardware or TestFlight evidence exists from this research.
