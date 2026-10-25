#!/usr/bin/env python3
"""Bounded App Store Connect processing check.

Uses the same App Store Connect API key as the release workflow. It builds an ES256 JWT with
openssl, finds the app by bundle ID, polls the uploaded build, and prints a single JSON status
line. Exit codes:
  0  build is VALID (processed)
  2  terminal failure (FAILED, INVALID, EXPIRED)
  3  deadline reached while still PROCESSING (incomplete, never reported as success)

Environment: APPLE_API_ISSUER, APPLE_API_KEY (key id), APPLE_API_KEY_PATH, BUNDLE_ID,
BUILD_NUMBER, optional POLL_SECONDS (default 900) and POLL_INTERVAL (default 20).
"""
import base64
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

API = "https://api.appstoreconnect.apple.com/v1"


def b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode("ascii")


def make_jwt(key_path: str, key_id: str, issuer: str) -> str:
    header = {"alg": "ES256", "kid": key_id, "typ": "JWT"}
    now = int(time.time())
    payload = {"iss": issuer, "iat": now, "exp": now + 1200, "aud": "appstoreconnect-v1"}
    signing_input = f"{b64url(json.dumps(header).encode())}.{b64url(json.dumps(payload).encode())}"
    der = subprocess.run(
        ["openssl", "dgst", "-sha256", "-sign", key_path],
        input=signing_input.encode(),
        capture_output=True,
        check=True,
    ).stdout
    # Convert DER (SEQUENCE of two INTEGERs) to the 64 byte JOSE signature.
    index = 2 if der[1] < 0x80 else 3 + (der[1] & 0x7F) - 1
    if der[index] != 0x02:
        raise SystemExit("unexpected ECDSA signature encoding")
    r_len = der[index + 1]
    r = der[index + 2 : index + 2 + r_len]
    s_start = index + 2 + r_len
    if der[s_start] != 0x02:
        raise SystemExit("unexpected ECDSA signature encoding")
    s_len = der[s_start + 1]
    s = der[s_start + 2 : s_start + 2 + s_len]
    raw = r.lstrip(b"\x00").rjust(32, b"\x00") + s.lstrip(b"\x00").rjust(32, b"\x00")
    return f"{signing_input}.{b64url(raw)}"


def get(token: str, path: str) -> dict:
    request = urllib.request.Request(f"{API}{path}", headers={"Authorization": f"Bearer {token}"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.loads(response.read().decode())


def main() -> int:
    key_id = os.environ["APPLE_API_KEY"]
    issuer = os.environ["APPLE_API_ISSUER"]
    key_path = os.environ["APPLE_API_KEY_PATH"]
    bundle = os.environ.get("BUNDLE_ID", "net.lakesidegames.corporatewarfare")
    build_number = os.environ["BUILD_NUMBER"]
    deadline = time.time() + int(os.environ.get("POLL_SECONDS", "900"))
    interval = int(os.environ.get("POLL_INTERVAL", "20"))
    token = make_jwt(key_path, key_id, issuer)

    query = urllib.parse.urlencode({"filter[bundleId]": bundle, "limit": 1})
    apps = get(token, f"/apps?{query}").get("data", [])
    if not apps:
        print(json.dumps({"state": "NO_APP_RECORD", "bundle": bundle}))
        return 2
    app_id = apps[0]["id"]

    last_state = "NOT_FOUND"
    while time.time() < deadline:
        query = urllib.parse.urlencode(
            {"filter[app]": app_id, "filter[version]": build_number, "limit": 1}
        )
        builds = get(token, f"/builds?{query}").get("data", [])
        if builds:
            build = builds[0]
            last_state = build.get("attributes", {}).get("processingState", "UNKNOWN")
            if last_state == "VALID":
                detail = get(token, f"/builds/{build['id']}/buildBetaDetail")
                attributes = detail.get("data", {}).get("attributes", {})
                print(
                    json.dumps(
                        {
                            "state": "VALID",
                            "build": build_number,
                            "app_id": app_id,
                            "internal_build_state": attributes.get("internalBuildState"),
                            "external_build_state": attributes.get("externalBuildState"),
                        }
                    )
                )
                return 0
            if last_state in {"FAILED", "INVALID", "EXPIRED"}:
                print(json.dumps({"state": last_state, "build": build_number, "app_id": app_id}))
                return 2
        time.sleep(interval)

    print(json.dumps({"state": "PROCESSING_AT_DEADLINE", "build": build_number, "last": last_state}))
    return 3


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (KeyError, urllib.error.URLError, subprocess.CalledProcessError) as error:
        print(json.dumps({"state": "CHECK_FAILED", "detail": str(error)}))
        sys.exit(2)
