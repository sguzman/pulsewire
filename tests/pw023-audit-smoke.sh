#!/usr/bin/env bash
# PW-023: deterministic, offline, harmless network preflight scenarios.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
fixture="$root/tests/fixtures/pw023-network-audit.json"
temp="$(mktemp -d)"
trap 'rm -rf "$temp"' EXIT
audit="$root/scripts/pw023-audit.sh"
allowed=(--allow-db ci-server --allow-db ci-fetcher --allow-db ci-db-server)

verify() {
  local expected_status="$1" expected_result="$2" input="$3"
  shift 3
  local actual_status=0 report=''
  report="$(bash "$audit" --snapshot "$input" "$@")" || actual_status=$?
  if [[ "$actual_status" != "$expected_status" ]]; then
    echo "expected status $expected_status, got $actual_status" >&2
    return 1
  fi
  if [[ "$expected_status" == "64" ]]; then
    return 0
  fi
  printf '%s\n' "$report" | jq -e \
    --arg state "$expected_result" \
    '.result == $state and .deployment_approved == false
     and (.unknowns | length) >= 1' >/dev/null
  echo "PW-023 offline audit: $expected_result status $expected_status"
}
mutate() {
  local label="$1" expr="$2"
  jq "$expr" "$fixture" > "$temp/$label.json"
}

verify 0 PASS_STATIC "$fixture" "${allowed[@]}"
verify 2 NEEDS_REVIEW "$fixture"
verify 2 NEEDS_REVIEW "$fixture" \
  --allow-db ci-server --allow-db ci-fetcher
# An overbroad or stale allowlist is not the current network's
# reviewed inventory, even when every observed member is allowed.
verify 2 NEEDS_REVIEW "$fixture" "${allowed[@]}" --allow-db ci-stale
verify 2 NEEDS_REVIEW "$fixture" \
  --allow-db ci-server --allow-db ci-fetcher --allow-db ci-stale
# The observed-member count must not include unobserved allowlist names.
stale_status=0
stale_report="$(bash "$audit" --snapshot "$fixture" \
  "${allowed[@]}" --allow-db ci-stale)" || stale_status=$?
test "$stale_status" -eq 2
jq -e '.reviewed_database_member_count == 3 and
       .unobserved_allowlisted_member_count == 1 and
       .database_members_to_review == [] and
       .deployment_approved == false' <<< "$stale_report" >/dev/null
# One CLI argument containing line breaks must not silently create
# multiple independent trusted Docker member names.
verify 64 NONE "$fixture" --allow-db $'ci-server\nci-fetcher\nci-db-server'
verify 64 NONE "$fixture" --allow-db $'ci-server\rci-fetcher'
mutate reader-port '.roles.server.ports=[{container_port:"8091/tcp",host_ip:"127.0.0.1",host_port:"18091"}]'
verify 1 FAIL "$temp/reader-port.json" "${allowed[@]}"
mutate exposed-tls '.roles.edge.ports[0].host_ip="0.0.0.0"'
verify 1 FAIL "$temp/exposed-tls.json" "${allowed[@]}"
mutate edge-port-low '.roles.edge.ports[0].host_port="1"'
verify 0 PASS_STATIC "$temp/edge-port-low.json" "${allowed[@]}"
mutate edge-port-high '.roles.edge.ports[0].host_port="65535"'
verify 0 PASS_STATIC "$temp/edge-port-high.json" "${allowed[@]}"
# The collector emits decimal strings. A nonempty string, array or
# object is NOT proof of a valid host TCP port; reject out-of-range
# values and incorrect JSON types without accepting the snapshot.
mutate edge-port-absent 'del(.roles.edge.ports[0].host_port)'
verify 1 FAIL "$temp/edge-port-absent.json" "${allowed[@]}"
mutate edge-port-null '.roles.edge.ports[0].host_port=null'
verify 1 FAIL "$temp/edge-port-null.json" "${allowed[@]}"
mutate edge-port-zero '.roles.edge.ports[0].host_port="0"'
verify 1 FAIL "$temp/edge-port-zero.json" "${allowed[@]}"
mutate edge-port-too-high '.roles.edge.ports[0].host_port="65536"'
verify 1 FAIL "$temp/edge-port-too-high.json" "${allowed[@]}"
mutate edge-port-not-decimal '.roles.edge.ports[0].host_port="abc"'
verify 1 FAIL "$temp/edge-port-not-decimal.json" "${allowed[@]}"
mutate edge-port-suffix '.roles.edge.ports[0].host_port="18443/tcp"'
verify 1 FAIL "$temp/edge-port-suffix.json" "${allowed[@]}"
mutate edge-port-number '.roles.edge.ports[0].host_port=18443'
verify 1 FAIL "$temp/edge-port-number.json" "${allowed[@]}"
mutate edge-port-array '.roles.edge.ports[0].host_port=[]'
verify 1 FAIL "$temp/edge-port-array.json" "${allowed[@]}"
mutate edge-port-object '.roles.edge.ports[0].host_port={}'
verify 1 FAIL "$temp/edge-port-object.json" "${allowed[@]}"
mutate db-proxy '.roles.edge.networks+=["ci-db"] | .networks.database.members+=["ci-proxy"]'
verify 1 FAIL "$temp/db-proxy.json" "${allowed[@]}"
mutate app-intruder '.networks.application.members+=["ci-unknown"]'
verify 1 FAIL "$temp/app-intruder.json" "${allowed[@]}"
mutate monitor-public '.roles.prometheus.ports=[{container_port:"9090/tcp",host_ip:"0.0.0.0",host_port:"9090"}]'
verify 1 FAIL "$temp/monitor-public.json" "${allowed[@]}"
mutate fetcher-ports-missing 'del(.roles.fetcher.ports)'
verify 1 FAIL "$temp/fetcher-ports-missing.json" "${allowed[@]}"
mutate fetcher-ports-null '.roles.fetcher.ports=null'
verify 1 FAIL "$temp/fetcher-ports-null.json" "${allowed[@]}"
mutate prometheus-ports-missing 'del(.roles.prometheus.ports)'
verify 1 FAIL "$temp/prometheus-ports-missing.json" "${allowed[@]}"
mutate prometheus-ports-object '.roles.prometheus.ports={}'
verify 1 FAIL "$temp/prometheus-ports-object.json" "${allowed[@]}"
mutate prometheus-ports-string '.roles.prometheus.ports=""'
verify 1 FAIL "$temp/prometheus-ports-string.json" "${allowed[@]}"
mutate monitor-missing '.networks.monitoring.internal=false'
verify 1 FAIL "$temp/monitor-missing.json" "${allowed[@]}"
mutate monitor-untrusted '.networks.monitoring.members+=["ci-intruder"]'
verify 1 FAIL "$temp/monitor-untrusted.json" "${allowed[@]}"
mutate stopped-server '.roles.server.running=false'
verify 1 FAIL "$temp/stopped-server.json" "${allowed[@]}"
mutate stopped-fetcher '.roles.fetcher.running=false'
verify 1 FAIL "$temp/stopped-fetcher.json" "${allowed[@]}"
mutate stopped-prometheus '.roles.prometheus.running=false'
verify 1 FAIL "$temp/stopped-prometheus.json" "${allowed[@]}"
mutate duplicate-monitor-role '.roles.prometheus.name=.roles.server.name'
verify 1 FAIL "$temp/duplicate-monitor-role.json" "${allowed[@]}"
mutate empty-monitor-role '.roles.prometheus.name=""'
verify 1 FAIL "$temp/empty-monitor-role.json" "${allowed[@]}"
mutate monitor-network-name '.networks.monitoring.name="other-monitor-network"'
verify 1 FAIL "$temp/monitor-network-name.json" "${allowed[@]}"
mutate app-network-name '.networks.application.name="other-app-network"'
verify 1 FAIL "$temp/app-network-name.json" "${allowed[@]}"
mutate empty-server-name '.roles.server.name=""'
verify 1 FAIL "$temp/empty-server-name.json" "${allowed[@]}"
mutate incomplete 'del(.roles.server.name)'
verify 1 FAIL "$temp/incomplete.json" "${allowed[@]}"
printf 'not json\n' > "$temp/invalid.json"
verify 64 NONE "$temp/invalid.json" "${allowed[@]}"
echo 'PW-023 read-only inventory: all synthetic allow/deny/review cases passed'
