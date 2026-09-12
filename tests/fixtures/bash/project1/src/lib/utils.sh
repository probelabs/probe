#!/usr/bin/env bash
# Shared string helpers for the fixture project.

trim() {
  local value="$1"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "$value"
}

to_upper() {
  printf '%s' "$1" | tr '[:lower:]' '[:upper:]'
}

join_by() {
  local delimiter="$1"
  shift
  local first="$1"
  shift
  printf '%s' "$first" "${@/#/$delimiter}"
}
