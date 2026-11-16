#!/usr/bin/env bash
# PW-023 sanitized Docker metadata collector: READ ONLY, no socket probes.
# docker inspect --format accesses ONLY network names, effective runtime
# published ports, running flag and network-member names; never secrets.
# NetworkSettings.Ports (not HostConfig.PortBindings) includes -P mappings.
set -euo pipefail

usage() {
  printf '%s\n' \
    'Usage: pw023-capture.sh --server NAME --edge NAME --db-network NAME' \
    '  --app-network NAME --front-network NAME' \
    '  [--fetcher NAME --prometheus NAME --monitor-network NAME]' \
    'Prints sanitized JSON to stdout; never changes Docker or the host.' >&2
  exit 64
}
server='' edge='' db='' app='' front='' fetcher='' prom='' mon=''
while (($#)); do
  (($# >= 2)) || usage
  case "$1" in
    --server) server="$2" ;;
    --edge) edge="$2" ;;
    --db-network) db="$2" ;;
    --app-network) app="$2" ;;
    --front-network) front="$2" ;;
    --fetcher) fetcher="$2" ;;
    --prometheus) prom="$2" ;;
    --monitor-network) mon="$2" ;;
    *) usage ;;
  esac
  shift 2
done
for value in "$server" "$edge" "$db" "$app" "$front"; do
  [[ -n "$value" ]] || usage
done
if [[ -n "$fetcher$prom$mon" ]] &&
   [[ -z "$fetcher" || -z "$prom" || -z "$mon" ]]; then
  usage
fi
for program in docker jq; do
  command -v "$program" >/dev/null || {
    echo "$program is required; no changes made" >&2
    exit 64
  }
done

container_record() {
  local id="$1" networks binds running
  networks="$(docker inspect --type container \
    --format '{{json .NetworkSettings.Networks}}' "$id")"
  # HostConfig.PortBindings is configuration intent and can omit
  # automatically allocated -P/--publish-all ports. The runtime
  # NetworkSettings.Ports includes effective host publications;
  # exposed-but-unpublished ports have null entries and are ignored.
  binds="$(docker inspect --type container \
    --format '{{json .NetworkSettings.Ports}}' "$id")"
  running="$(docker inspect --type container \
    --format '{{json .State.Running}}' "$id")"
  jq -nc --arg name "$id" --argjson networks "$networks" \
    --argjson binds "$binds" --argjson running "$running" '
      {
        name: $name,
        running: $running,
        networks: (($networks // {}) | keys),
        ports: [
          (($binds // {}) | to_entries[]) as $port
          | ($port.value // [])[]
          | {container_port:$port.key,
             host_ip:(.HostIp // ""),
             host_port:(.HostPort // "")}
        ]
      }'
}

network_record() {
  local name="$1" members internal
  members="$(docker network inspect --format '{{json .Containers}}' "$name")"
  internal="$(docker network inspect --format '{{json .Internal}}' "$name")"
  jq -nc --arg name "$name" --argjson members "$members" \
    --argjson internal "$internal" '
      {name:$name,internal:$internal,
       members:[(($members // {}) | .[]) | .Name]} | .members |= sort'
}

server_json="$(container_record "$server")"
edge_json="$(container_record "$edge")"
db_json="$(network_record "$db")"
app_json="$(network_record "$app")"
front_json="$(network_record "$front")"

if [[ -n "$mon" ]]; then
  fetcher_json="$(container_record "$fetcher")"
  prom_json="$(container_record "$prom")"
  mon_json="$(network_record "$mon")"
else
  fetcher_json='null'
  prom_json='null'
  mon_json='null'
fi

jq -nc --arg db "$db" --arg app "$app" --arg front "$front" \
  --arg mon "$mon" \
  --argjson server "$server_json" --argjson edge "$edge_json" \
  --argjson fetcher "$fetcher_json" --argjson prom "$prom_json" \
  --argjson database "$db_json" --argjson application "$app_json" \
  --argjson frontdata "$front_json" --argjson monitoring "$mon_json" '
    {schema:1,
     network_names:{database:$db,application:$app,front:$front}
       + (if $mon=="" then {} else {monitoring:$mon} end),
     roles:{server:$server,edge:$edge}
       + (if $mon=="" then {} else {fetcher:$fetcher,prometheus:$prom} end),
     networks:{database:$database,application:$application,front:$frontdata}
       + (if $mon=="" then {} else {monitoring:$monitoring} end)
    }'
