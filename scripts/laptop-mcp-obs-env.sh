#!/usr/bin/env bash
# Source this file from the repository root or any subdirectory.

_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
_obs_install="$_repo_root/obs-install"

if [[ ! -d "$_obs_install" ]]; then
  echo "OBS validation install not found at $_obs_install" >&2
  echo "Run ./scripts/bootstrap-laptop-mcp-obs.sh first." >&2
  return 1 2>/dev/null || exit 1
fi

export PATH="$_obs_install/bin:$PATH"
export PKG_CONFIG_PATH="$_obs_install/lib/pkgconfig:$_obs_install/lib/$(gcc -dumpmachine)/pkgconfig:${PKG_CONFIG_PATH:-}"
export LD_LIBRARY_PATH="$_obs_install/lib:$_obs_install/lib/$(gcc -dumpmachine):${LD_LIBRARY_PATH:-}"
export RUSTFLAGS="-L $_obs_install/lib -L $_obs_install/lib/$(gcc -dumpmachine) ${RUSTFLAGS:-}"
export TOP_SECRET_NO_DUMMY_DLL=true

unset _repo_root _obs_install
