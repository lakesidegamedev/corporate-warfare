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
    if any(re.search(r"URLSession|https?://[a-zA-Z]", body) for body in text.values()):
        problems.append("network calls found; the offline declaration no longer holds")
    if any(re.search(r"Analytics|Firebase|Sentry|Amplitude|Mixpanel|Crashlytics", body)
           for body in text.values()):
        problems.append("analytics or crash SDK reference found; owner approval required")
    if manifest.get("NSPrivacyTracking") is not False:
        problems.append("NSPrivacyTracking must be false for this release")
    if manifest.get("NSPrivacyCollectedDataTypes"):
        problems.append("collected data types declared; owner approval required")
    if problems:
        for problem in problems:
            print(f"privacy: {problem}", file=sys.stderr)
        return 1
    print("privacy audit ok: no collection, no tracking, declared required-reason APIs only")
    return 0


if __name__ == "__main__":
    sys.exit(main())
