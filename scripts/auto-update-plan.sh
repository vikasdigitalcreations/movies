#!/usr/bin/env bash
# Decides whether the auto-update workflow has anything to do, and what to build on.
#
# Run from a checkout of the default branch. Writes needed / ref / current / upstream_tag /
# next_version to $GITHUB_OUTPUT.
#
# Which commit to build on: the default branch, unless it is older than the last release.
# That is the normal state right after an automatic release, whose change sits on its own
# auto/vendor-<tag> branch until someone merges it. Building on the default branch then
# would ship older code under a newer number, so the build starts from the commit the last
# release was tagged on instead. Updates keep chaining without anyone merging anything.
#
# Needs: git, gh (GH_TOKEN), python. Environment: UPSTREAM, GITHUB_REPOSITORY, FORCE.
# RELEASED_TAG overrides the lookup of the last release, for testing.
set -euo pipefail

PY=${PY:-python3}
newest() { printf '%s\n' "$@" | sort -V | tail -1; }
version_at() { git show "$1:src-tauri/tauri.conf.json" | "$PY" -c "import json,sys;print(json.load(sys.stdin)['version'])"; }
vendored_at() { git show "$1:vendor/moviebox-tui/Cargo.toml" | sed -n 's/^version *= *"\([^"]*\)".*/\1/p' | head -1; }

upstream_tag=$(gh api "repos/$UPSTREAM/releases/latest" --jq .tag_name)
upstream=${upstream_tag#v}
latest_tag=${RELEASED_TAG:-$(gh release view --repo "$GITHUB_REPOSITORY" --json tagName --jq .tagName 2>/dev/null || echo v0.0.0)}
released=${latest_tag#v}

ref=$(git rev-parse HEAD)
app=$(version_at "$ref")
if [ "$latest_tag" != "v0.0.0" ] && [ "$(newest "$app" "$released")" != "$app" ]; then
  echo "The default branch is MovieBox $app but $released is released; building on $latest_tag instead."
  git rev-parse -q --verify "refs/tags/$latest_tag" >/dev/null \
    || git fetch --quiet --depth 1 origin "refs/tags/$latest_tag:refs/tags/$latest_tag"
  ref=$(git rev-parse "$latest_tag^{commit}")
  app=$(version_at "$ref")
  if [ "$app" != "$released" ]; then
    echo "::error::Release $latest_tag is tagged on a commit whose version is $app, so it cannot be built on. Merge the code that shipped as $released into the default branch."
    exit 1
  fi
fi

current=$(vendored_at "$ref")
echo "building on $ref (MovieBox $app, vendoring MovieBox-Tui $current) | upstream latest: $upstream | last release: $released"

needed=false
if [ "$upstream" != "$current" ] && [ "$(newest "$upstream" "$current")" = "$upstream" ]; then
  needed=true
fi
if [ "${FORCE:-false}" = "true" ]; then needed=true; fi

# An earlier run already published this upstream version (its branch is the proof).
if [ "$needed" = true ] && [ "${FORCE:-false}" != "true" ] \
   && git ls-remote --exit-code --heads "https://github.com/$GITHUB_REPOSITORY" "auto/vendor-$upstream_tag" >/dev/null 2>&1; then
  echo "auto/vendor-$upstream_tag already exists, so $upstream_tag was published before. Nothing to do."
  needed=false
fi

next=$(echo "$app" | awk -F. '{printf "%d.%d.%d", $1, $2, $3 + 1}')

{
  echo "needed=$needed"
  echo "ref=$ref"
  echo "current=$current"
  echo "upstream_tag=$upstream_tag"
  echo "next_version=$next"
} >> "${GITHUB_OUTPUT:-/dev/stdout}"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  if [ "$needed" = true ]; then
    echo "### Update needed: MovieBox-Tui $current → $upstream, shipping as MovieBox $next" >> "$GITHUB_STEP_SUMMARY"
  else
    echo "### Up to date: vendoring MovieBox-Tui $current, upstream is $upstream" >> "$GITHUB_STEP_SUMMARY"
  fi
fi
