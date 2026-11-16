#!/usr/bin/env bash
# PW-023: read-only inventory assessment. No docker/network writes.
# Only consumes an explicitly supplied sanitized JSON snapshot.
set -euo pipefail

usage() {
  printf '%s\n' \
    'Usage: pw023-audit.sh --snapshot FILE|- [--allow-db NAME ...]' \
    'Exit 0 = static checks PASS; 1 = FAIL; 2 = NEEDS_REVIEW.' \
    'A PASS is NOT firewall, TLS, deployment, or release approval.' >&2
  exit 64
}
snapshot=''
allow=()
while (($#)); do
  case "$1" in
    --snapshot)
      (($# >= 2)) || usage
      snapshot="$2"; shift 2 ;;
    --allow-db)
      (($# >= 2)) || usage
      [[ -n "$2" ]] || usage
      # Reject line-break smuggling: the JSON encoder splits on newlines.
      [[ "$2" != *$'\n'* && "$2" != *$'\r'* ]] || usage
      allow+=("$2"); shift 2 ;;
    -h|--help) usage ;;
    *) usage ;;
  esac
done
[[ -n "$snapshot" ]] || usage
command -v jq >/dev/null || {
  echo "jq is required; no changes made" >&2
  exit 64
}
if [[ "$snapshot" != '-' && ! -r "$snapshot" ]]; then
  echo "snapshot cannot be read; no changes made" >&2
  exit 64
fi
allow_json="$(printf '%s\n' "${allow[@]}" | jq -Rsc 'split("\n") | map(select(length > 0)) | unique')"
# The input schema excludes Docker Env, mounts, labels, secrets, and
# the output excludes full network/IP inventories by design.
set +e
report="$(jq -ce --argjson allow_db "$allow_json" '
  def same($left; $right): (($left | sort) == ($right | sort));
  def network($key): .networks[$key];
  def role($key): .roles[$key];
  def names($key): .network_names[$key];
  # Missing/null/malformed port metadata is NOT proof of no publication.
  # Only an explicitly inspected empty array can establish that.
  def ports_empty($key):
    ((role($key).ports | type) == "array" and
     (role($key).ports | length) == 0);
  # Docker reports published host ports as decimal strings. Reject
  # missing, non-string, non-decimal, zero and out-of-range values.
  # Merely checking `length > 0` accepts strings like "bad" or arrays.
  def valid_host_port:
    if type != "string" then false
    elif (test("^[0-9]{1,5}$") | not) then false
    else (tonumber >= 1 and tonumber <= 65535) end;
  def memberships($key; $keys):
    (.network_names) as $n |
      same((role($key).networks // []); [$keys[] | $n[.]]);
  def members($key):
    ((network($key).members // []) | unique);
  def has_member($key; $name):
    (members($key) | index($name)) != null;
  def required:
    .schema == 1
    and (.roles.server.name | type) == "string"
    and (.roles.edge.name | type) == "string"
    and (.roles.server.name | length) > 0
    and (.roles.edge.name | length) > 0
    and (.roles.server.networks | type) == "array"
    and (.roles.edge.networks | type) == "array"
    and (.roles.server.ports | type) == "array"
    and (.roles.edge.ports | type) == "array"
    and (.networks.database.members | type) == "array"
    and (.networks.application.members | type) == "array"
    and (.networks.front.members | type) == "array"
    and ([.network_names.database, .network_names.application,
          .network_names.front] | all(type == "string" and length > 0))
    and ([.network_names.database, .network_names.application,
          .network_names.front] | unique | length) == 3
    and (.networks.database.name == .network_names.database)
    and (.networks.application.name == .network_names.application)
    and (.networks.front.name == .network_names.front)
    and (role("server").name != role("edge").name);
  if (required | not) then
    {result:"FAIL", reason:"invalid or incomplete sanitized snapshot",
     static_checks:{schema:false},
     unknowns:["All host/network behavior is unverified"],
     deployment_approved:false}
  else
    (
      ((.roles.fetcher != null) or (.roles.prometheus != null)
        or (.networks.monitoring != null) or (.network_names.monitoring != null))
    ) as $monitoring
    | {
      server_running: (.roles.server.running == true),
      edge_running: (.roles.edge.running == true),
      private_app_bridge: (.networks.application.internal == true),
      server_private_networks: memberships("server"; ["database", "application"]),
      edge_private_networks: memberships("edge"; ["application", "front"]),
      server_host_http_absent: ports_empty("server"),
      edge_host_tls_loopback:
        ((.roles.edge.ports | length) == 1 and
         .roles.edge.ports[0].container_port == "8443/tcp" and
         .roles.edge.ports[0].host_ip == "127.0.0.1" and
         (.roles.edge.ports[0].host_port | valid_host_port)),
      app_members_only_expected:
        same(members("application"); [.roles.server.name, .roles.edge.name]),
      db_includes_reader: has_member("database"; .roles.server.name),
      app_includes_reader: has_member("application"; .roles.server.name),
      app_includes_proxy: has_member("application"; .roles.edge.name),
      front_includes_proxy: has_member("front"; .roles.edge.name),
      db_excludes_proxy: (has_member("database"; .roles.edge.name) | not),
      monitoring_profile_consistent:
        (if $monitoring then
          (.roles.fetcher != null and .roles.prometheus != null and
           (.roles.fetcher.name | type) == "string" and
           (.roles.prometheus.name | type) == "string" and
           (.roles.fetcher.name | length) > 0 and
           (.roles.prometheus.name | length) > 0 and
           ([.roles.server.name, .roles.edge.name, .roles.fetcher.name,
             .roles.prometheus.name] | unique | length) == 4 and
           .roles.fetcher.running == true and
           .roles.prometheus.running == true and
           (.network_names.monitoring | type) == "string" and
           (.network_names.monitoring | length) > 0 and
           (.network_names.monitoring != .network_names.database) and
           (.network_names.monitoring != .network_names.application) and
           (.network_names.monitoring != .network_names.front) and
           (.networks.monitoring.name == .network_names.monitoring) and
           (.networks.monitoring.internal == true) and
           memberships("fetcher"; ["database", "monitoring"]) and
           memberships("prometheus"; ["monitoring"]) and
           ports_empty("fetcher") and ports_empty("prometheus") and
           same(members("monitoring"); [.roles.fetcher.name, .roles.prometheus.name]) and
           has_member("database"; .roles.fetcher.name))
         else true end)
    } as $checks
    | (members("database") - $allow_db) as $unknown_db
    | ($allow_db - members("database")) as $unobserved_allow_db
    | {
        result:
          (if ([$checks[] | select(. == false)] | length > 0) then "FAIL"
           elif ($allow_db | length) == 0 then "NEEDS_REVIEW"
           # Require an exact match: a stale pre-authorized container
           # could later join the DB network without re-review.
           elif ($unknown_db | length) > 0 or
                ($unobserved_allow_db | length) > 0 then "NEEDS_REVIEW"
           else "PASS_STATIC" end),
        static_checks: $checks,
        database_members_to_review:
          (if ($allow_db | length) == 0
           then members("database")
           else $unknown_db end),
        reviewed_database_member_count:
          ((members("database") | length) - ($unknown_db | length)),
        unobserved_allowlisted_member_count: ($unobserved_allow_db | length),
        monitoring_inspected: $monitoring,
        unknowns: [
          "No active firewall/IPv6/router/NAT reachability probes performed",
          "TLS certificate trust, renewals and upstream real-IP policy unverified",
          "Docker administrators and trusted DB-network members can bypass the edge",
          "Mount permissions, token rotation, Prometheus UI access and retention require review",
          "Operator must explicitly approve any real-host changes or deployment"
        ],
        deployment_approved: false
      }
  end
' "$snapshot")"
jq_status=$?
set -e
if ((jq_status != 0)); then
  echo "invalid audit input or jq error; no changes made" >&2
  exit 64
fi
printf '%s\n' "$report"
result="$(printf '%s\n' "$report" | jq -r '.result')"
# The JSON report is the sole authority for the exit status.
case "$result" in
  PASS_STATIC) exit 0 ;;
  NEEDS_REVIEW) exit 2 ;;
  *) exit 1 ;;
esac
