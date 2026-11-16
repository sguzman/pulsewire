#!/usr/bin/env bash
# PW-023 offline Docker metadata regression. No daemon/socket/network calls.
# Fake docker intentionally reports no configured bindings even though -P
# created an effective runtime host mapping in NetworkSettings.Ports.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
temp="$(mktemp -d)"
trap 'rm -rf "$temp"' EXIT
mkdir -p "$temp/bin"
cat > "$temp/bin/docker" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" == inspect && "$2" == --type && "$3" == container &&
      "$4" == --format ]]; then
  format="$5"
  name="$6"
  case "$name" in
    ci-server)
      networks='{"ci-db":{},"ci-app":{}}'
      # An image-exposed port is not a host publication until bound.
      ports='{"8091/tcp":null}'
      if [[ "${PW023_MOCK_AUTO_PUBLISH:-0}" == 1 ]]; then
        ports='{"8091/tcp":[{"HostIp":"0.0.0.0","HostPort":"32768"}],"1234/tcp":null}'
      fi
      # Bad inspect data is NOT equivalent to inspected, unpublished
      # ports. The collector must refuse every malformed response.
      case "${PW023_MOCK_PORTS_MODE:-valid}" in
        valid) ;;
        null) ports='null' ;;
        array) ports='[]' ;;
        missing-binding-shape) ports='{"8091/tcp":{}}' ;;
        string-binding-shape) ports='{"8091/tcp":"not-an-array"}' ;;
        missing-host-port)
          ports='{"8091/tcp":[{"HostIp":"127.0.0.1"}]}' ;;
        null-host-port)
          ports='{"8091/tcp":[{"HostIp":"127.0.0.1","HostPort":null}]}' ;;
        *) echo "unexpected mock port variant" >&2; exit 1 ;;
      esac ;;
    ci-proxy)
      networks='{"ci-app":{},"ci-edge":{}}'
      ports='{"8443/tcp":[{"HostIp":"127.0.0.1","HostPort":"18443"}]}' ;;
    *) echo "unexpected mock container" >&2; exit 1 ;;
  esac
  case "$format" in
    '{{json .NetworkSettings.Networks}}') printf '%s\n' "$networks" ;;
    '{{json .NetworkSettings.Ports}}') printf '%s\n' "$ports" ;;
    '{{json .HostConfig.PortBindings}}')
      # Simulated empty PortBindings for an automatic -P publication.
      printf '%s\n' '{}' ;;
    '{{json .State.Running}}') printf '%s\n' true ;;
    *) echo "unexpected inspect field: $format" >&2; exit 1 ;;
  esac
elif [[ "$1" == network && "$2" == inspect && "$3" == --format ]]; then
  format="$4"
  name="$5"
  case "$name" in
    ci-db)
      internal=false
      members='{"one":{"Name":"ci-db-server"},"two":{"Name":"ci-server"},"three":{"Name":"ci-fetcher"}}' ;;
    ci-app)
      internal=true
      members='{"one":{"Name":"ci-server"},"two":{"Name":"ci-proxy"}}' ;;
    ci-edge)
      internal=false
      members='{"one":{"Name":"ci-proxy"},"two":{"Name":"ci-edge-client"}}' ;;
    *) echo "unexpected mock network" >&2; exit 1 ;;
  esac
  case "$format" in
    '{{json .Containers}}') printf '%s\n' "$members" ;;
    '{{json .Internal}}') printf '%s\n' "$internal" ;;
    *) echo "unexpected network field: $format" >&2; exit 1 ;;
  esac
else
  echo "unexpected mock docker invocation" >&2
  exit 1
fi
MOCK
chmod +x "$temp/bin/docker"
capture() {
  PATH="$temp/bin:$PATH" PW023_MOCK_AUTO_PUBLISH="$1" \
    PW023_MOCK_PORTS_MODE="${2:-valid}" \
    bash "$root/scripts/pw023-capture.sh" \
      --server ci-server --edge ci-proxy \
      --db-network ci-db --app-network ci-app \
      --front-network ci-edge
}
allowed=(--allow-db ci-server --allow-db ci-fetcher --allow-db ci-db-server)
# No host port: the exposed-but-unpublished null entry must not be reported.
safe="$(capture 0)"
jq -e '.roles.server.ports == [] and
       .roles.edge.ports == [{container_port:"8443/tcp",host_ip:"127.0.0.1",host_port:"18443"}]' <<< "$safe" >/dev/null
safe_status=0
safe_report="$(bash "$root/scripts/pw023-audit.sh" \
  --snapshot - "${allowed[@]}" <<< "$safe")" || safe_status=$?
test "$safe_status" -eq 0
jq -e '.result == "PASS_STATIC" and .deployment_approved == false' \
  <<< "$safe_report" >/dev/null
# Automatic -P exposes an ephemeral host port even though the mock
# HostConfig.PortBindings is {}. The audit must reject this topology.
published="$(capture 1)"
jq -e '.roles.server.ports ==
  [{container_port:"8091/tcp",host_ip:"0.0.0.0",host_port:"32768"}]' \
  <<< "$published" >/dev/null
bad_status=0
bad_report="$(bash "$root/scripts/pw023-audit.sh" \
  --snapshot - "${allowed[@]}" <<< "$published")" || bad_status=$?
test "$bad_status" -eq 1
jq -e '.result == "FAIL" and
       .static_checks.server_host_http_absent == false and
       .deployment_approved == false' <<< "$bad_report" >/dev/null
# A null, array or malformed Docker runtime port map must never be
# converted into a safe empty list, even if no port is visibly bound.
for mode in null array missing-binding-shape string-binding-shape \
            missing-host-port null-host-port; do
  if capture 0 "$mode" > "$temp/invalid-capture.json" \
      2> "$temp/invalid-capture.err"; then
    echo "malformed Docker ports were accepted: $mode" >&2
    exit 1
  fi
  test ! -s "$temp/invalid-capture.json" || {
    echo "malformed Docker ports produced a snapshot: $mode" >&2
    exit 1
  }
done
echo "PW-023 effective runtime ports passed: unpublished EXPOSE ignored, synthetic -P publication denied, malformed port metadata rejected"
