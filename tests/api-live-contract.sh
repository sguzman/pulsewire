#!/usr/bin/env bash
# PW-061: runs against existing disposable two-item RSS fixture on each
# database backend before TUI/browser CI journeys. No production URLs.
set -euo pipefail
: "$PULSEWIRE_API_E2E_URL"
: "$PULSEWIRE_API_E2E_TOKEN"
case "$PULSEWIRE_API_E2E_URL" in
  http://127.0.0.1:18091|http://127.0.0.1:18092) ;;
  *) echo "Refusing non-CI API contract server" >&2; exit 2 ;;
esac
base="$PULSEWIRE_API_E2E_URL"
get() {
  curl -fsS -H "Authorization: Bearer $PULSEWIRE_API_E2E_TOKEN" "$base$1"
}
send() {
  curl -fsS -o /dev/null -H "Authorization: Bearer $PULSEWIRE_API_E2E_TOKEN" -X "$1" "$base$2"
}
unauthed="$(curl -sS -o /dev/null -w '%{http_code}' "$base/v1/entries?limit=1")"
test "$unauthed" = 401
unauthed="$(curl -sS -o /dev/null -w '%{http_code}' "$base/v1/feeds/counts")"
test "$unauthed" = 401
# An ambiguous duplicate Authorization header must not authenticate.
# Check both orderings through the live disposable HTTP server.
for order in first-valid first-invalid; do
  if [[ "$order" == "first-valid" ]]; then
    auth_headers=(-H "Authorization: Bearer $PULSEWIRE_API_E2E_TOKEN" \
      -H "Authorization: Bearer invalid-token")
  else
    auth_headers=(-H "Authorization: Bearer invalid-token" \
      -H "Authorization: Bearer $PULSEWIRE_API_E2E_TOKEN")
  fi
  ambiguous="$(curl -sS -o /dev/null -w '%{http_code}' \
    "${auth_headers[@]}" "$base/v1/entries?limit=1")"
  test "$ambiguous" = 401 || {
    echo "duplicate Authorization fields were accepted ($order): $ambiguous" >&2
    exit 1
  }
done
invalid="$(curl -sS -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $PULSEWIRE_API_E2E_TOKEN" "$base/v1/entries?read=invalid")"
test "$invalid" = 400

page_one="$(get '/v1/entries?limit=1&offset=0')"
jq -e '(.items | length) == 1 and .next_offset == 1 and .next_cursor == .items[0].id and .since == null' <<< "$page_one"
first_id="$(jq -er '.items[0].id' <<< "$page_one")"
page_two="$(get '/v1/entries?limit=1&offset=1')"
jq -e --argjson first "$first_id" '(.items | length) == 1 and .next_offset == null and .next_cursor == .items[0].id and .items[0].id < $first' <<< "$page_two"
second_id="$(jq -er '.items[0].id' <<< "$page_two")"
jq -e '(.items | length) == 0 and .next_offset == null and .next_cursor == null' <<< "$(get '/v1/entries?limit=1&offset=2')"
jq -e '(.items | length) == 1 and .next_offset == 1' <<< "$(get '/v1/entries?limit=0')"
jq -e --argjson first "$first_id" --argjson second "$second_id" '(.items | length) == 1 and .items[0].id == $first and .next_cursor == $first and .since == $second' \
  <<< "$(get "/v1/entries?since=$second_id")"
jq -e '(.items | length) == 1 and .next_offset == null' \
  <<< "$(get '/v1/feeds/pulsewire-ci-real-rss/entries?limit=1&offset=1')"
jq -e '(.items | length) == 2' <<< "$(get '/v1/entries/search?q=Pulsewire%20CI%20Story')"
jq -e '(.items | length) == 1 and .next_offset == 1' <<< "$(get '/v1/entries/search?q=Pulsewire%20CI%20Story&limit=1&offset=0')"
jq -e '(.items | length) == 1 and .next_offset == null' <<< "$(get '/v1/entries/search?q=Pulsewire%20CI%20Story&limit=1&offset=1')"
for q in '%25' '%5F' '%5C'; do
  jq -e '(.items | length) == 0 and .next_offset == null' <<< "$(get "/v1/entries/search?q=$q")"
done
jq -e '(.items | length) == 2' <<< "$(get '/v1/entries?read=unread')"
jq -e '(.items | length) == 0' <<< "$(get '/v1/entries?read=read')"

# Full-catalog feed counts include unsubscribed sources and zero-read states.
jq -e 'any(.[]; .feed_id == "pulsewire-ci-real-rss" and .total_count == 2 and .unread_count == 2 and .read_count == 0)' \
  <<< "$(get '/v1/feeds/counts')"
jq -e '.count == 0' <<< "$(get '/v1/entries/unread/count')"
curl -fsS -o /dev/null -H "Authorization: Bearer $PULSEWIRE_API_E2E_TOKEN" \
  -H 'Content-Type: application/json' -d '{"feed_id":"pulsewire-ci-real-rss"}' "$base/v1/subscriptions"
jq -e '.count == 2' <<< "$(get '/v1/entries/unread/count')"
jq -e 'any(.[]; .feed_id == "pulsewire-ci-real-rss" and .unread_count == 2)' <<< "$(get '/v1/feeds/unread/counts')"
send POST "/v1/entries/$first_id/read"
jq -e 'any(.[]; .feed_id == "pulsewire-ci-real-rss" and .total_count == 2 and .unread_count == 1 and .read_count == 1)' <<< "$(get '/v1/feeds/counts')"
jq -e '.count == 1' <<< "$(get '/v1/entries/unread/count')"
send DELETE "/v1/entries/$first_id/read"
jq -e '.count == 2' <<< "$(get '/v1/entries/unread/count')"
send DELETE '/v1/subscriptions/pulsewire-ci-real-rss'
jq -e '.count == 0' <<< "$(get '/v1/entries/unread/count')"

# Folder pagination is the same contract as global/feed/search pagination.
folder="$(curl -fsS -H "Authorization: Bearer $PULSEWIRE_API_E2E_TOKEN" \
  -H 'Content-Type: application/json' -d '{"name":"PW061 CI acceptance"}' "$base/v1/folders")"
folder_id="$(jq -er '.id' <<< "$folder")"
curl -fsS -o /dev/null -H "Authorization: Bearer $PULSEWIRE_API_E2E_TOKEN" \
  -H 'Content-Type: application/json' -d '{"feed_id":"pulsewire-ci-real-rss"}' \
  "$base/v1/folders/$folder_id/feeds"
jq -e '(.items | length) == 1 and .next_offset == 1' \
  <<< "$(get "/v1/folders/$folder_id/entries?limit=1&offset=0")"
jq -e '(.items | length) == 1 and .next_offset == null' \
  <<< "$(get "/v1/folders/$folder_id/entries?limit=1&offset=1")"
send DELETE "/v1/folders/$folder_id"
echo "PW-061 live API contract passed on $base"
