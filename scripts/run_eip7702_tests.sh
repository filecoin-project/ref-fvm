#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BA_DIR="${ROOT_DIR}/../builtin-actors"

echo "[eip7702] Building builtin-actors bundle in Docker..."
if ! command -v docker >/dev/null 2>&1; then
  echo "ERROR: Docker is required to run this script. Please install/start Docker." >&2
  exit 1
fi

REF_FVM_ABS="$(cd "${ROOT_DIR}" && pwd)"
BA_ABS="$(cd "${BA_DIR}" && pwd)"
# Build the bundle with ref-fvm mounted to satisfy local patch paths.
docker run --rm ${DOCKER_PLATFORM:-} \
  -v "${BA_ABS}/output:/output" \
  -v "${REF_FVM_ABS}:/usr/src/ref-fvm" \
  builtin-actors-builder "testing"

echo "[eip7702] Running ref-fvm tests (host toolchain)..."
pushd "${ROOT_DIR}" >/dev/null
if cargo test -p fvm --tests -- --nocapture; then
  echo "[eip7702] Host tests succeeded."
else
  echo "[eip7702] Host tests failed; falling back to Docker runner..." >&2
  popd >/dev/null
  # Run tests inside the builtin-actors builder image, mounting both repos under a common /work root
  REF_FVM_ABS="$(cd "${ROOT_DIR}" && pwd)"
  BA_ABS="$(cd "${BA_DIR}" && pwd)"
  echo "[eip7702] Docker test run with volumes:"
  echo "  - ref-fvm: ${REF_FVM_ABS} -> /work/ref-fvm"
  echo "  - builtin-actors: ${BA_ABS} -> /work/builtin-actors"
  docker run --rm ${DOCKER_PLATFORM:-} \
    -v "${REF_FVM_ABS}:/work/ref-fvm" \
    -v "${BA_ABS}:/work/builtin-actors" \
    -w /work/ref-fvm \
    builtin-actors-builder bash -lc 'rustup show && cargo test -p fvm --tests -- --nocapture' || {
      echo "[eip7702] ref-fvm tests failed inside Docker as well." >&2
      exit 1
    }
  echo "[eip7702] Docker-based tests succeeded."
  exit 0
fi
popd >/dev/null

echo "[eip7702] Test matrix summary:"
echo " - EXTCODE* projection + windowing"
echo " - Depth limit (delegation not re-followed)"
echo " - Value transfer short-circuit"
echo " - Delegated revert payload propagation"

echo "[eip7702] Done."
