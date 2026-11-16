#!/usr/bin/env bash
# PW-032 live sampling is allowed ONLY inside the named disposable
# GitHub Actions PostgreSQL/RSS/metrics fixture. No external URLs.
# Read-only Docker stats, SELECT, and private /metrics scraping only.
set -euo pipefail
: "${RUNNER_TEMP:?}"
: "${GITHUB_WORKSPACE:?}"
: "${GITHUB_SHA:?}"
: "${TEST_DB_PASSWORD:?}"
: "${GITHUB_ACTIONS:?}"
if [[ "$GITHUB_ACTIONS" != "true" ||
      "${PULSEWIRE_PW032_DISPOSABLE:-}" != "YES" ]]; then
  echo "PW-032 live capture is limited to explicitly opted-in disposable CI" >&2
  exit 64
fi
for container in pulsewire-ci-postgres pulsewire-ci-rss pulsewire-ci-metrics; do
  case "$(docker inspect --type container -f '{{.HostConfig.NetworkMode}}' "$container")" in
    pulsewire-ci) ;;
    *) echo "PW-032 requires the isolated CI-only network" >&2; exit 64 ;;
  esac
  if [[ "$(docker inspect --type container -f '{{.State.Running}}' "$container")" != true ]]; then
    echo "PW-032 CI fixture is not running" >&2
    exit 64
  fi
done
if [[ -n "$(docker port pulsewire-ci-metrics 9898/tcp)" ]]; then
  echo "PW-032 metrics unexpectedly host-published" >&2
  exit 64
fi
token_path="$RUNNER_TEMP/pw023-metrics-token"
if [[ ! -s "$token_path" ]]; then
  echo "PW-032 disposable test token missing" >&2
  exit 64
fi
scratch="$(mktemp -d "$RUNNER_TEMP/pw032-disposable.XXXXXXXX")"
trap 'rm -rf -- "$scratch"' EXIT

# Count only application-owned PostgreSQL heap/materialized tables,
# including their indexes and TOAST through pg_total_relation_size.
# No shared database-wide size and no SQL writes.
sql="$(cat <<'SQL'
SELECT
  (SELECT COALESCE(SUM(pg_total_relation_size(c.oid)), 0)::bigint
   FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
   WHERE n.nspname='fetcher' AND c.relkind IN ('r', 'm')),
  (SELECT COUNT(*) FROM fetcher.feed_state_current
   WHERE next_action_at <= CURRENT_TIMESTAMP),
  (SELECT COUNT(*) FROM fetcher.feeds WHERE category='news'),
  (SELECT COUNT(*) FROM fetcher.feeds WHERE category='gov'),
  (SELECT COUNT(*) FROM fetcher.feeds WHERE category='journals')
SQL
)"
catalog=''
for sample in 1 2 3; do
  stamp="$(date +%s%3N)"
  docker_json="$(docker stats --no-stream --format '{{json .}}' pulsewire-ci-metrics)"
  sql_result="$(docker exec -e PGPASSWORD="$TEST_DB_PASSWORD" \
    pulsewire-ci-postgres psql -X -v ON_ERROR_STOP=1 -U admin -d data \
    -At -F '|' -c "$sql")"
  if [[ ! "$sql_result" =~ ^[0-9]+\|[0-9]+\|[0-9]+\|[0-9]+\|[0-9]+$ ]]; then
    echo "PW-032 received invalid disposable SQL counts" >&2
    exit 1
  fi
  IFS='|' read -r db_bytes due news gov journals <<< "$sql_result"
  if [[ "$((news + gov + journals))" -lt 1 ]]; then
    echo "PW-032 must observe at least one CI fixture feed" >&2
    exit 1
  fi
  this_catalog="$news/$gov/$journals"
  if [[ -n "$catalog" && "$catalog" != "$this_catalog" ]]; then
    echo "PW-032 sample catalog changed during CI observation" >&2
    exit 1
  fi
  catalog="$this_catalog"
  metrics="$(docker exec pulsewire-ci-rss wget -qO- \
    --header="Authorization: Bearer $(cat "$token_path")" \
    http://pulsewire-ci-metrics:9898/metrics)"
  jq -cn --argjson timestamp_ms "$stamp" \
    --argjson docker_stats "$docker_json" \
    --argjson database_bytes "$db_bytes" \
    --argjson backlog_due "$due" --arg metrics_text "$metrics" \
    '{timestamp_ms:$timestamp_ms,docker_stats:$docker_stats,
      database_bytes:$database_bytes,backlog_due:$backlog_due,
      metrics_text:$metrics_text}' \
    > "$scratch/sample-$sample.json"
  if ((sample < 3)); then sleep 2; fi
done
IFS='/' read -r news gov journals <<< "$catalog"
# The checked-out code, not a made-up deployed SHA.
source_sha="$(git -C "$GITHUB_WORKSPACE" rev-parse HEAD)"
jq -s --arg commit "$source_sha" \
  --argjson news "$news" --argjson government "$gov" \
  --argjson journals "$journals" \
  '{schema:1,source:"synthetic_ci",commit:$commit,
    catalog:{news:$news,government:$government,journals:$journals},
    samples:.}' "$scratch"/sample-*.json > "$scratch/raw.json"
node "$GITHUB_WORKSPACE/scripts/pw032-ci-normalize.mjs" \
  --input "$scratch/raw.json" > "$scratch/sanitized.json"
node "$GITHUB_WORKSPACE/scripts/pw032-envelope.mjs" \
  --input "$scratch/sanitized.json" > "$scratch/report.json"
jq -e '
  .kind == "pw032_resource_envelope" and
  .source_claim == "synthetic_ci" and
  .catalog_feed_count >= 1 and
  .window.sample_count == 3 and
  .memory_bytes.max > 0 and
  .database_bytes.first > 0 and
  .acceptance.target_host_verified == false and
  .acceptance.sustainable_capacity_proven == false and
  .acceptance.pw032_weighted_points == 0 and
  .acceptance.deployment_approved == false
' "$scratch/report.json" >/dev/null
# Never print raw Prometheus labels, container identifiers,
# the SQL query, Docker environment or the scrape bearer.
jq -r '"PW-032 disposable CI observations passed: " +
  (.window.sample_count|tostring) + " real snapshots, " +
  (.catalog_feed_count|tostring) + " private fixture feeds, CPU max " +
  (.cpu_container_percent.max|tostring) + "%, memory max " +
  (.memory_bytes.max|tostring) + " bytes; no host acceptance"' \
  "$scratch/report.json"
