#!/usr/bin/env bash
set -euo pipefail

NAME=$(git config user.name 2>/dev/null || echo "")
CUT_OFF=$(date -u -d '30 days ago' +%s)

# Ensure remotes are up to date
git fetch --all --prune

local_candidates=()
remote_candidates=()

# Collect local candidates (not protected and not touched by you since cutoff)
for br in $(git for-each-ref --format '%(refname:short)' refs/heads/); do
  if [[ "$br" == main || "$br" == master || "$br" == develop || "$br" == release/* || "$br" == hotfix/* ]]; then
    continue
  fi
  last_by_you=$(git log -1 --format='%ad' --date=short --author="$NAME" "$br" 2>/dev/null || true)
  if [[ -z "$last_by_you" ]]; then
    local_candidates+=("$br")
  else
    last_sec=$(date -d "$last_by_you" +%s 2>/dev/null || echo 0)
    if (( last_sec < CUT_OFF )); then
      local_candidates+=("$br")
    fi
  fi
done

# Collect remote candidates (origin/* not protected and not touched by you since cutoff)
for br in $(git for-each-ref --format '%(refname:short)' refs/remotes/origin/); do
  short=${br#origin/}
  if [[ "$short" == HEAD ]]; then continue; fi
  if [[ "$short" == main || "$short" == master || "$short" == develop || "$short" == release/* || "$short" == hotfix/* ]]; then
    continue
  fi
  last_by_you=$(git log -1 --format='%ad' --date=short --author="$NAME" "origin/$short" 2>/dev/null || true)
  if [[ -z "$last_by_you" ]]; then
    remote_candidates+=("$short")
  else
    last_sec=$(date -d "$last_by_you" +%s 2>/dev/null || echo 0)
    if (( last_sec < CUT_OFF )); then
      remote_candidates+=("$short")
    fi
  fi
done

# Summary
echo "Archiving ${#local_candidates[@]} local candidate(s): ${local_candidates[*]}" 1>&2
echo "Archiving ${#remote_candidates[@]} remote candidate(s): ${remote_candidates[*]}" 1>&2

# Archive local candidates
for br in "${local_candidates[@]}"; do
  arch="archive/$br"
  if ! git show-ref --verify --quiet "refs/heads/$arch"; then
    git branch "$arch" "$br" 2>/dev/null || true
  fi
  echo "[LOCAL] Pushing $arch to origin..."
  git push origin "$arch" >/dev/null 2>&1 || true
  echo "[LOCAL] Deleting remote branch $br..."
  git push origin --delete "$br" 2>/dev/null || true
  echo "[LOCAL] Deleting local branch $br..."
  git branch -D "$br" 2>/dev/null || true
done

# Archive remote candidates
for br in "${remote_candidates[@]}"; do
  arch="archive/origin/$br"
  if ! git show-ref --verify --quiet "refs/heads/$arch"; then
    git fetch origin >/dev/null 2>&1 || true
    if git rev-parse --verify --quiet "origin/$br"; then
      git switch -c "$arch" "origin/$br" 2>/dev/null || git checkout -b "$arch" "origin/$br"
    fi
  fi
  echo "[REMOTE] Pushing $arch to origin..."
  git push origin "$arch" 2>/dev/null || true
  echo "[REMOTE] Deleting remote branch $br..."
  git push origin --delete "$br" 2>/dev/null || true
done
