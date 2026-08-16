#!/usr/bin/env bash
set -e

# ==============================================================================
# RustSale CRM - All-in-One Build & Cloudflare R2 Release Script
# ==============================================================================
#
# Cách sử dụng:
#   ./scripts/release.sh
#   ./scripts/release.sh --version 0.2.0 --notes "Cập nhật tính năng Zalo & Scraper"
#   ./scripts/release.sh --skip-build   (chỉ upload file đã build sẵn)
#   ./scripts/release.sh --dry-run      (chỉ build mà không upload lên R2)
# ==============================================================================

cd "$(dirname "$0")/.."

bun run scripts/publish-release.ts "$@"
