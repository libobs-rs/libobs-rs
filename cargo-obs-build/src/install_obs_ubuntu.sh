#!/bin/bash

set -e

echo "Performing System-wide Installation of OBS Studio on Ubuntu/Linux..."
echo "Make sure there is no existing OBS Studio installation via apt or other package managers."

if [[ -n "${CARGO_OBS_BUILD_YES:-}" ]]; then
    echo "CARGO_OBS_BUILD_YES is set; continuing without prompt."
else
    echo "Do you want to continue? (y/n)"
    read -r answer
    if [[ "$answer" != "y" ]]; then
        echo "Aborting installation."
        exit 1
    fi
fi

echo "Installing OBS Studio system dependencies for Linux..."

sudo apt update
sudo apt install cmake extra-cmake-modules ninja-build pkg-config clang clang-format build-essential curl ccache git zsh libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev libavutil-dev libswresample-dev libswscale-dev libx264-dev libcurl4-openssl-dev libmbedtls-dev libgl1-mesa-dev libjansson-dev libluajit-5.1-dev python3-dev libx11-dev libxcb-randr0-dev libxcb-shm0-dev libxcb-xinerama0-dev libxcb-composite0-dev libxcomposite-dev libxinerama-dev libxcb1-dev libx11-xcb-dev libxcb-xfixes0-dev swig libcmocka-dev libxss-dev libglvnd-dev libgles2-mesa-dev libwayland-dev librist-dev libsrt-openssl-dev libpci-dev libpipewire-0.3-dev libqrcodegencpp-dev uthash-dev libsimde-dev qt6-base-dev qt6-base-private-dev qt6-svg-dev qt6-wayland qt6-image-formats-plugins libasound2-dev libfdk-aac-dev libfontconfig-dev libfreetype6-dev libjack-jackd2-dev libpulse-dev libsndio-dev libspeexdsp-dev libudev-dev libv4l-dev libva-dev libvlc-dev libvpl-dev libdrm-dev nlohmann-json3-dev libwebsocketpp-dev libasio-dev libffmpeg-nvenc-dev xvfb ffmpeg libblas-dev libblas3 liblapack3

TEMP_DIR=$(mktemp -d)

OBS_REPO="${OBS_GIT_REPO:-https://github.com/obsproject/obs-studio.git}"
echo "Cloning OBS Studio from: $OBS_REPO"

# Clone OBS Studio repository
git clone --recursive "$OBS_REPO" $TEMP_DIR
cd $TEMP_DIR

git fetch --tags
LATEST_TAG=$(git describe --tags --abbrev=0)

OBS_BUILD_TAG="${OBS_BUILD_TAG:-$LATEST_TAG}"

# Get the latest stable tag
echo "Building OBS Studio version: $OBS_BUILD_TAG"
git checkout $OBS_BUILD_TAG
git submodule update --init --recursive

# Configure build with CMAKE_INSTALL_PREFIX=/usr
cmake --preset ubuntu \
-DCMAKE_INSTALL_PREFIX=/usr -DOBS_COMPILE_DEPRECATION_AS_WARNING=ON

# Build OBS Studio
cmake --build build_ubuntu --parallel $(nproc)

# Install OBS Studio to /usr
sudo cmake --install build_ubuntu

rm -rf $TEMP_DIR

echo "OBS Studio has been successfully installed system-wide on your Ubuntu/Linux system."