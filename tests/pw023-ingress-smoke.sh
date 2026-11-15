#!/usr/bin/env bash
# PW-023 disposable TLS ingress regression; NEVER uses real host networks.
set -euo pipefail

: "${RUNNER_TEMP:?}"
: "${TEST_DB_PASSWORD:?}"
workspace="$(cd "$(dirname "$0")/.." && pwd)"
tls_dir="$RUNNER_TEMP/pw023-ingress-fixture"
mkdir -p "$tls_dir"

cleanup() {
  result=$?
  if [ "$result" -ne 0 ]; then
    docker logs --tail 60 pw023-ci-proxy 2>&1 || true
    docker logs --tail 40 pw023-ci-ingress-server 2>&1 || true
  fi
  docker rm -f pw023-ci-client-a pw023-ci-client-b \
    pw023-ci-proxy pw023-ci-ingress-server >/dev/null 2>&1 || true
  docker network rm pw023-ci-edge pw023-ci-private >/dev/null 2>&1 || true
  exit "$result"
}
trap cleanup EXIT

# Fully disposable certificate: NOT for production, not a trusted CA.
openssl req -x509 -nodes -newkey rsa:2048 -sha256 -days 1 \
  -subj "/CN=pulsewire-ci-edge-proxy" \
  -addext "subjectAltName=DNS:pulsewire-ci-edge-proxy,IP:127.0.0.1" \
  -keyout "$tls_dir/key.pem" -out "$tls_dir/cert.pem" \
  >/dev/null 2>&1
chmod 0600 "$tls_dir/key.pem"
chmod 0644 "$tls_dir/cert.pem"

# Verify the actual optional Compose merge before building a live topology.
PULSEWIRE_POSTGRES_HOST=ci-postgres \
PULSEWIRE_POSTGRES_PASSWORD_FILE=/run/secrets/pulsewire_postgres_password \
PULSEWIRE_DOCKER_NETWORK=pulsewire-ci \
PULSEWIRE_TLS_CERT_SOURCE="$tls_dir/cert.pem" \
PULSEWIRE_TLS_KEY_SOURCE="$tls_dir/key.pem" \
  docker compose -f "$workspace/crates/server/docker-compose.yml" \
    -f "$workspace/crates/server/docker-compose.ingress.yml" \
    config --format json > "$tls_dir/compose.json"
jq -e '
  (.services.server.ports // [] | length) == 0
  and (.services.edge_proxy.ports | length) == 1
  and .services.edge_proxy.ports[0].host_ip == "127.0.0.1"
  and .services.edge_proxy.ports[0].target == 8443
  and (.services.edge_proxy.networks | has("pulsewire_services") | not)
  and (.services.server.networks | has("pulsewire_edge") | not)
  and .networks.pulsewire_ingress_private.internal == true
  and .networks.pulsewire_services.external == true
' "$tls_dir/compose.json" >/dev/null

docker pull nginx:1.27.5-alpine >/dev/null
docker pull curlimages/curl:8.11.1 >/dev/null

# Existing pulsewire-ci PostgreSQL is a disposable workflow fixture.
# The new reader has NO direct host port; its private application bridge
# is accessible only to the proxy. The proxy is NOT on the DB network.
docker network create --internal pw023-ci-private >/dev/null
docker network create pw023-ci-edge >/dev/null
docker run -d --name pw023-ci-ingress-server --network pulsewire-ci \
  -e PULSEWIRE_POSTGRES_HOST=ci-postgres \
  -e PULSEWIRE_POSTGRES_PASSWORD_FILE=/run/secrets/pulsewire_postgres_password \
  -v "$RUNNER_TEMP/pw023-runtime-secret:/run/secrets/pulsewire_postgres_password:ro" \
  pulsewire-server:ci >/dev/null
docker network connect --alias server pw023-ci-private pw023-ci-ingress-server

# Create before startup so both networks are attached before NGINX
# resolves the private server hostname.
docker create --name pw023-ci-proxy --network pw023-ci-private \
  -p 127.0.0.1:18443:8443 \
  --read-only --tmpfs /var/cache/nginx --tmpfs /var/run \
  --security-opt no-new-privileges \
  -v "$workspace/deploy/pw023/nginx.conf:/etc/nginx/conf.d/default.conf:ro" \
  -v "$tls_dir:/etc/nginx/tls:ro" \
  nginx:1.27.5-alpine >/dev/null
docker network connect --alias pulsewire-ci-edge-proxy pw023-ci-edge pw023-ci-proxy
docker start pw023-ci-proxy >/dev/null
docker exec pw023-ci-proxy nginx -t >/dev/null

# Inspect topology, not merely the declarative Compose example.
docker inspect pw023-ci-proxy --format '{{json .NetworkSettings.Networks}}' | \
  jq -e 'has("pw023-ci-private") and has("pw023-ci-edge") and (has("pulsewire-ci") | not)' >/dev/null
docker inspect pw023-ci-ingress-server --format '{{json .NetworkSettings.Networks}}' | \
  jq -e 'has("pw023-ci-private") and has("pulsewire-ci") and (has("pw023-ci-edge") | not)' >/dev/null
test -z "$(docker port pw023-ci-ingress-server)"
test "$(docker port pw023-ci-proxy 8443/tcp)" = '127.0.0.1:18443'

# Distinct, persistent edge containers give independent REAL TCP source IPs.
for who in a b; do
  docker run -d --name "pw023-ci-client-$who" --network pw023-ci-edge \
    -v "$tls_dir/cert.pem:/tmp/fixture-ca.pem:ro" \
    --entrypoint sh curlimages/curl:8.11.1 -c "sleep 300" >/dev/null
done
ip_a="$(docker inspect pw023-ci-client-a -f '{{(index .NetworkSettings.Networks "pw023-ci-edge").IPAddress}}')"
ip_b="$(docker inspect pw023-ci-client-b -f '{{(index .NetworkSettings.Networks "pw023-ci-edge").IPAddress}}')"
test -n "$ip_a" && test -n "$ip_b" && test "$ip_a" != "$ip_b"

url=https://pulsewire-ci-edge-proxy:8443
up=0
for try in {1..30}; do
  if docker exec pw023-ci-client-a curl --max-time 3 --cacert /tmp/fixture-ca.pem \
    --fail --silent "$url/reader" > "$tls_dir/reader.html" 2>/dev/null; then
    up=1; break
  fi
  sleep 1
done
test "$up" = 1
grep -qi 'pulsewire' "$tls_dir/reader.html"
docker exec pw023-ci-client-a curl --max-time 5 --cacert /tmp/fixture-ca.pem \
  --fail --silent --dump-header - --output /dev/null "$url/reader" > "$tls_dir/headers"
grep -qi '^strict-transport-security:' "$tls_dir/headers"

for path in /health /ready /metrics /docs /openapi.json /not-a-route; do
  status="$(docker exec pw023-ci-client-a curl --max-time 5 \
    --cacert /tmp/fixture-ca.pem --silent --output /dev/null \
    --write-out '%{http_code}' "$url$path")"
  test "$status" = 404 || { echo "unexpected ingress route: $path -> $status" >&2; exit 1; }
done
status="$(docker exec pw023-ci-client-a curl --max-time 5 \
  --cacert /tmp/fixture-ca.pem --silent --output /dev/null \
  --write-out '%{http_code}' "$url/v1/entries")"
test "$status" = 401

# Non-TLS plaintext must NOT work on the only published proxy port.
if curl --fail --max-time 3 --silent \
  "http://127.0.0.1:18443/reader" >/dev/null 2>&1; then
  echo "plain HTTP incorrectly accepted on the TLS ingress" >&2
  exit 1
fi
# This validates both network separation and the absence of an external
# direct server listener. DNS alias is private; even its IP is unreachable.
private_ip="$(docker inspect pw023-ci-ingress-server \
  -f '{{(index .NetworkSettings.Networks "pw023-ci-private").IPAddress}}')"
test -n "$private_ip"
if docker exec pw023-ci-client-a curl --max-time 2 --fail --silent \
  "http://$private_ip:8091/health" >/dev/null 2>&1; then
  echo "edge client bypassed HTTPS to private reader IP" >&2
  exit 1
fi
if docker exec pw023-ci-client-a curl --max-time 2 --fail --silent \
  http://server:8091/health >/dev/null 2>&1; then
  echo "edge client resolved a private server network alias" >&2
  exit 1
fi

# Spoofing forwarded headers must not create new login rate-limit
# buckets. The second true peer must still be allowed a normal
# invalid-credential attempt (401, not edge 429).
limited=0
for i in {1..16}; do
  status="$(docker exec pw023-ci-client-a curl --max-time 5 \
    --cacert /tmp/fixture-ca.pem --silent --output /dev/null \
    --write-out '%{http_code}' \
    -H 'Content-Type: application/json' \
    -H "X-Forwarded-For: 203.0.113.$i" \
    -H "X-Real-IP: 198.51.100.$i" \
    -H "Forwarded: for=192.0.2.$i" \
    --data "{\"username\":\"pw023-unknown-$i\",\"password\":\"invalid\"}" \
    "$url/v1/auth/login")"
  case "$status" in
    401) ;;
    429) limited=$((limited+1)) ;;
    *) echo "unexpected auth response from first peer: $status" >&2; exit 1 ;;
  esac
done
test "$limited" -ge 1 || { echo "spoofed headers bypassed login throttling" >&2; exit 1; }

second="$(docker exec pw023-ci-client-b curl --max-time 5 \
  --cacert /tmp/fixture-ca.pem --silent --output /dev/null \
  --write-out '%{http_code}' -H 'Content-Type: application/json' \
  --data '{"username":"pw023-second-peer","password":"invalid"}' \
  "$url/v1/auth/login")"
test "$second" = 401 || {
  echo "second real peer received unexpected login status: $second" >&2
  exit 1
}
# Exercise the READ-ONLY inventory collector against this known fixture.
# No explicit DB-member approval has been given; it MUST report review,
# never claim that the disposable database network is globally trusted.
snapshot="$(bash "$workspace/scripts/pw023-capture.sh" \
  --server pw023-ci-ingress-server --edge pw023-ci-proxy \
  --db-network pulsewire-ci --app-network pw023-ci-private \
  --front-network pw023-ci-edge)"
printf '%s\n' "$snapshot" | jq -e '
  .schema == 1
  and .roles.server.ports == []
  and .roles.edge.ports[0].host_ip == "127.0.0.1"
  and (.roles.server | has("environment") | not)
  and (.roles.edge | has("mounts") | not)
  and .networks.application.internal == true' >/dev/null
audit_status=0
audit_report="$(printf '%s\n' "$snapshot" | \
  bash "$workspace/scripts/pw023-audit.sh" --snapshot -)" || audit_status=$?
test "$audit_status" -eq 2
printf '%s\n' "$audit_report" | jq -e '
  .result == "NEEDS_REVIEW"
  and (.static_checks | all(.[]; . == true))
  and .deployment_approved == false' >/dev/null
echo "PW-023 live read-only inventory passed: Docker metadata sanitized, static port/network checks pass, DB trust requires review"
echo "PW-023 TLS ingress passed: verified HTTPS, minimal routes, distinct-peer login limiting, forged-header resistance, and edge-to-backend network isolation"
