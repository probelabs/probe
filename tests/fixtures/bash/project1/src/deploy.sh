#!/usr/bin/env bash
# Deploy script for the fixture project.

set -euo pipefail

APP_NAME="probe-fixture"
DEPLOY_ENV="${DEPLOY_ENV:-staging}"

log_info() {
  echo "[INFO] $*"
}

deploy_app() {
  local version="$1"
  log_info "Deploying $APP_NAME $version to $DEPLOY_ENV"
  echo "deployed"
}

rollback() {
  local version="$1"
  log_info "Rolling back $APP_NAME from $version"
  echo "rolled back"
}
