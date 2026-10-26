#!/usr/bin/env python3
"""Revoke the development certificates earlier CI runs left behind.

Every hosted macOS runner is a fresh machine, so automatic signing with the App Store Connect
API key creates a new "Apple Development: Created via API" certificate on each release run.
Apple caps development certificates per team, and once the cap is reached the archive fails
("Your account has reached the maximum number of certificates"). Those certificates' private
keys died with their runners, so they can never sign again. This revokes only certificates
whose name says they were created through the API; a person's own development certificate and
every distribution certificate are left alone.

Environment: APPLE_API_ISSUER, APPLE_API_KEY (key id), APPLE_API_KEY_PATH.
"""
import json
import os
import sys
import urllib.parse
import urllib.request

sys.path.insert(0, os.path.dirname(__file__))
from asc_status import API, make_jwt  # noqa: E402

DEVELOPMENT = "DEVELOPMENT,IOS_DEVELOPMENT,MAC_APP_DEVELOPMENT"


def main() -> int:
    token = make_jwt(os.environ["APPLE_API_KEY_PATH"], os.environ["APPLE_API_KEY"], os.environ["APPLE_API_ISSUER"])
    headers = {"Authorization": f"Bearer {token}"}
    query = urllib.parse.urlencode({"filter[certificateType]": DEVELOPMENT, "limit": 200})
    with urllib.request.urlopen(urllib.request.Request(f"{API}/certificates?{query}", headers=headers), timeout=30) as response:
        certificates = json.loads(response.read().decode()).get("data", [])
    stale = [c for c in certificates if "Created via API" in (c["attributes"].get("name") or "")]
    print(f"development certificates: {len(certificates)}, created by CI through the API: {len(stale)}")
    for certificate in stale:
        request = urllib.request.Request(f"{API}/certificates/{certificate['id']}", headers=headers, method="DELETE")
        with urllib.request.urlopen(request, timeout=30) as response:
            print(f"revoked {certificate['attributes'].get('certificateType')} {certificate['id']} ({response.status})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
