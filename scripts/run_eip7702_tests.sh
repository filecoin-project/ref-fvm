#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BA_DIR="${ROOT_DIR}/../builtin-actors"

echo "[eip7702] Building builtin-actors bundle in Docker..."
if ! command -v docker >/dev/null 2>&1; then
  echo "ERROR: Docker is required to run this script. Please install/start Docker." >&2
  exit 1
fi

pushd "${BA_DIR}" >/dev/null
make bundle-testing-repro
popd >/dev/null

echo "[eip7702] Running ref-fvm tests (this may rebuild with the Docker-built toolchain)..."
pushd "${ROOT_DIR}" >/dev/null
cargo test -p fvm --tests -- --nocapture || {
  echo "[eip7702] ref-fvm tests failed. If you are on macOS, prefer running tests inside Docker." >&2
  exit 1
}
popd >/dev/null

echo "[eip7702] Test matrix summary:"
echo " - EXTCODE* projection + windowing"
echo " - Depth limit (delegation not re-followed)"
echo " - Value transfer short-circuit"
echo " - Delegated revert payload propagation"

echo "[eip7702] Done."

