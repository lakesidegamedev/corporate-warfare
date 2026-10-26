#!/usr/bin/env python3
"""Audit the shipped Swift sources against the privacy manifest. Fails closed.

Portable: uses only the standard library so it runs on the Linux host and the macOS runner.
"""
import pathlib
import plistlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "apple/Sources/PrivacyInfo.xcprivacy"
SOURCES = ROOT / "apple/Sources"

REQUIRED_REASON_APIS = {
    "NSPrivacyAccessedAPICategoryUserDefaults": r"UserDefaults|@AppStorage",
    "NSPrivacyAccessedAPICategoryFileTimestamp": r"creationDate|contentModificationDate|fileModificationDate",
    "NSPrivacyAccessedAPICategorySystemBootTime": r"systemUptime|systemBootTime",
    "NSPrivacyAccessedAPICategoryDiskSpace": r"volumeAvailableCapacity|systemFreeSize",
    "NSPrivacyAccessedAPICategoryActiveKeyboards": r"activeInputModes",
}


# Owner-approved anonymous telemetry (market-balance snapshots, default on, toggle in Settings).
# Exactly one file may use the network, and only to reach this one endpoint.
TELEMETRY_FILE = "Telemetry.swift"
TELEMETRY_ENDPOINT = "https://ops.lakesidegames.net/api/cw/telemetry"
# The only collected data type allowed: gameplay numbers for analytics, never linked, never tracking.
APPROVED_COLLECTION = {
    "NSPrivacyCollectedDataType": "NSPrivacyCollectedDataTypeGameplayContent",
    "NSPrivacyCollectedDataTypeLinked": False,
    "NSPrivacyCollectedDataTypeTracking": False,
    "NSPrivacyCollectedDataTypePurposes": ["NSPrivacyCollectedDataTypePurposeAnalytics"],
}
SDK_PATTERN = r"\b(Firebase\w*|Sentry\w*|Amplitude\w*|Mixpanel\w*|Crashlytics|FBSDK\w*|AppsFlyer\w*|Adjust\w*|Segment\w*|GoogleAnalytics\w*)\b"
SDK_IMPORT = r"^\s*import\s+(Firebase|Sentry|Amplitude|Mixpanel|Crashlytics|FBSDK|AppsFlyer|Adjust|Segment|GoogleAnalytics|AppTrackingTransparency|AdSupport)"


def swift_sources():
    return [path for path in SOURCES.rglob("*.swift")]


def main() -> int:
    if not MANIFEST.is_file():
        print("privacy: missing privacy manifest", file=sys.stderr)
        return 1
    with MANIFEST.open("rb") as handle:
        manifest = plistlib.load(handle)
    declared = {
        item.get("NSPrivacyAccessedAPIType")
        for item in manifest.get("NSPrivacyAccessedAPITypes", [])
    }
    problems = []
    text = {path: path.read_text(errors="ignore") for path in swift_sources()}
    for api, pattern in REQUIRED_REASON_APIS.items():
        users = [str(path) for path, body in text.items() if re.search(pattern, body)]
        if users and api not in declared:
            problems.append(f"{api} used in {', '.join(users)} but not declared")
        if api in declared:
            entry = next(item for item in manifest["NSPrivacyAccessedAPITypes"]
                         if item.get("NSPrivacyAccessedAPIType") == api)
            if not entry.get("NSPrivacyAccessedAPITypeReasons"):
                problems.append(f"{api} declared without a reason code")
    for path, body in text.items():
        urls = set(re.findall(r"https?://[^\s\"')]+", body))
        uses_network = bool(re.search(r"URLSession|NWConnection|CFNetwork|WebSocket", body))
        if path.name == TELEMETRY_FILE:
            if urls - {TELEMETRY_ENDPOINT}:
                problems.append(f"telemetry reaches an unapproved address: {sorted(urls - {TELEMETRY_ENDPOINT})}")
        elif urls or uses_network:
            problems.append(f"network use in {path.relative_to(ROOT)}; only {TELEMETRY_FILE} may call out")
    for path, body in text.items():
        if re.search(SDK_IMPORT, body, re.MULTILINE) or re.search(SDK_PATTERN, body):
            problems.append(f"analytics, ad or crash SDK reference in {path.relative_to(ROOT)}; owner approval required")
        if re.search(r"identifierForVendor|advertisingIdentifier|ASIdentifierManager", body):
            problems.append(f"device identifier read in {path.relative_to(ROOT)}; snapshots must stay anonymous")
    if manifest.get("NSPrivacyTracking") is not False:
        problems.append("NSPrivacyTracking must be false for this release")
    if manifest.get("NSPrivacyTrackingDomains"):
        problems.append("tracking domains declared; none are approved")
    for item in manifest.get("NSPrivacyCollectedDataTypes", []):
        if item != APPROVED_COLLECTION:
            problems.append(f"collected data type not approved: {item}")
    if problems:
        for problem in problems:
            print(f"privacy: {problem}", file=sys.stderr)
        return 1
    print("privacy audit ok: anonymous gameplay telemetry only, no tracking, declared required-reason APIs only")
    return 0


if __name__ == "__main__":
    sys.exit(main())
