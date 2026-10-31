#!/usr/bin/env bash
# Run a command up to N times, but only again when it failed on a download from Maven Central or Google Maven.
# Gradle treats an HTTP 403 or 429 from a repository as fatal and does not try another repository, so a rate-limited
# runner fails a build that would pass a minute later. A compile error or a failing test is not retried.
#
#   compose/scripts/retry.sh 3 ./gradlew --no-daemon :androidApp:assembleRelease
set -uo pipefail
attempts="$1"; shift
log="$(mktemp)"
trap 'rm -f "$log"' EXIT
for ((n = 1; n <= attempts; n++)); do
  "$@" 2>&1 | tee "$log"
  status=${PIPESTATUS[0]}
  ((status == 0)) && exit 0
  if ((n < attempts)) && grep -qE 'status code (403|429|5[0-9][0-9])|Could not (resolve|GET|HEAD)|Read timed out|Connection (reset|timed out)|Premature end of Content-Length' "$log"; then
    echo "retry.sh: attempt $n of $attempts failed on a download (exit $status); waiting $((n * 30)) s" >&2
    sleep $((n * 30))
  else
    exit "$status"
  fi
done
exit "$status"
