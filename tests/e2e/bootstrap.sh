#!/usr/bin/env bash
# Waits for the e2e Nightscout, creates one access token per role, and prints
# `KEY=value` lines for the e2e suite. Usage:
#
#   tests/e2e/bootstrap.sh >> "$GITHUB_ENV"        # CI
#   set -a; eval "$(tests/e2e/bootstrap.sh)"; set +a # local shell
#
# Env overrides: NS_E2E_URL (default http://localhost:1337),
#                NS_E2E_SECRET (default cinnamon-e2e-secret).
set -euo pipefail

url="${NS_E2E_URL:-http://localhost:1337}"
secret="${NS_E2E_SECRET:-cinnamon-e2e-secret}"

if command -v sha1sum >/dev/null 2>&1; then
  hash="$(printf %s "$secret" | sha1sum | cut -d' ' -f1)"
else
  hash="$(printf %s "$secret" | shasum -a 1 | cut -d' ' -f1)"
fi

# 1. Wait until the server answers (any HTTP status means Express is up).
for _ in $(seq 1 120); do
  code="$(curl -s -o /dev/null -w '%{http_code}' -H "api-secret: $hash" "$url/api/v1/status.json" || true)"
  if [ "$code" = "200" ]; then break; fi
  sleep 1
done
if [ "$code" != "200" ]; then
  echo "Nightscout at $url did not become ready (last status: $code)" >&2
  exit 1
fi

# 2. One subject per role we exercise. Re-running is harmless: existing names are reused.
existing="$(curl -sf -H "api-secret: $hash" "$url/api/v2/authorization/subjects")"
for pair in "cinnamon-admin:admin" "cinnamon-reader:readable" "cinnamon-careportal:careportal"; do
  name="${pair%%:*}"
  role="${pair##*:}"
  if ! printf %s "$existing" | grep -q "\"name\":\"$name\""; then
    curl -sf -X POST -H "api-secret: $hash" -H 'Content-Type: application/json' \
      -d "{\"name\":\"$name\",\"roles\":[\"$role\"]}" \
      "$url/api/v2/authorization/subjects" >/dev/null
  fi
done

subjects="$(curl -sf -H "api-secret: $hash" "$url/api/v2/authorization/subjects")"
token_for() {
  printf %s "$subjects" | python3 -c '
import json, sys
name = sys.argv[1]
for s in json.load(sys.stdin):
    if s.get("name") == name:
        print(s["accessToken"])
        break
' "$1"
}

echo "NS_E2E_URL=$url"
echo "NS_E2E_SECRET=$secret"
echo "NS_E2E_ADMIN_TOKEN=$(token_for cinnamon-admin)"
echo "NS_E2E_READ_TOKEN=$(token_for cinnamon-reader)"
echo "NS_E2E_CAREPORTAL_TOKEN=$(token_for cinnamon-careportal)"
