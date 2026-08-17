#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
obs_version="$(tr -d '[:space:]' < "$repo_root/libobs/OBS_VERSION")"
source_dir="$repo_root/obs-studio"
install_dir="$repo_root/obs-install"
stamp="$install_dir/.libobs-rs-obs-version"
jobs="${JOBS:-$(nproc)}"

if [[ -f "$stamp" ]] && [[ "$(cat "$stamp")" == "$obs_version" ]] \
  && find "$install_dir" -path '*/pkgconfig/libobs.pc' -print -quit | grep -q .; then
  echo "OBS $obs_version is already installed at $install_dir"
  exit 0
fi

rm -rf "$source_dir" "$install_dir"

# Match the Ubuntu CI environment for BLAS/LAPACK discovery when Debian installs the
# alternatives behind subdirectories.
if [[ -e /usr/lib/x86_64-linux-gnu/blas/libblas.so.3 ]]; then
  sudo ln -sf /usr/lib/x86_64-linux-gnu/blas/libblas.so.3 /usr/lib/x86_64-linux-gnu/libblas.so.3
fi
if [[ -e /usr/lib/x86_64-linux-gnu/lapack/liblapack.so.3 ]]; then
  sudo ln -sf /usr/lib/x86_64-linux-gnu/lapack/liblapack.so.3 /usr/lib/x86_64-linux-gnu/liblapack.so.3
fi

git clone --recursive --depth 1 --branch "$obs_version" \
  https://github.com/obsproject/obs-studio.git "$source_dir"

cmake --preset ubuntu -S "$source_dir" \
  -DCMAKE_INSTALL_PREFIX="$install_dir" \
  -DENABLE_BROWSER=OFF
cmake --build "$source_dir/build_ubuntu" --parallel "$jobs"
cmake --install "$source_dir/build_ubuntu"

# OBS installs libraries into a Debian multiarch subdirectory while some generated pkg-config
# metadata still refers to ${prefix}/lib. Keep both layouts usable for Rust tests/doctests.
multiarch_dir="$install_dir/lib/$(gcc -dumpmachine)"
if [[ -d "$multiarch_dir" ]]; then
  for library in "$multiarch_dir"/libobs.so*; do
    [[ -e "$library" ]] || continue
    ln -sfn "$(basename "$multiarch_dir")/$(basename "$library")" \
      "$install_dir/lib/$(basename "$library")"
  done
fi

printf '%s\n' "$obs_version" > "$stamp"
echo "OBS $obs_version installed at $install_dir"
echo "Run: source scripts/laptop-mcp-obs-env.sh"
