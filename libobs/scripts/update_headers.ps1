param(
    [string]$Branch = "", # Allow specifying branch as a parameter
    [string]$Repository = "obsproject/obs-studio"
)

# Function to get the latest release tag from GitHub
function Get-LatestReleaseTag
{
    $releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/releases/latest"
    return $releases.tag_name
}

# Set up paths
$tempDir = Join-Path -Path $env:TEMP -ChildPath "obs-studio-temp"
$targetHeaderDir = Join-Path -Path $PSScriptRoot -ChildPath "../headers/obs"
$targetBindingsPath = Join-Path -Path $PSScriptRoot -ChildPath "../src/bindings.rs"

# Determine which branch/tag to use
if ( [string]::IsNullOrEmpty($Branch))
{
    $Branch = Get-LatestReleaseTag
    Write-Host "No branch specified. Using latest release tag: $Branch"
}
else
{
    Write-Host "Using specified branch/tag: $Branch"
}

# Clean up temp directory if it exists
if (Test-Path -Path $tempDir)
{
    Write-Host "Cleaning up existing temporary directory..."
    Remove-Item -Path $tempDir -Recurse -Force
}

# Clone the repository with depth 1
Write-Host "Cloning obs-studio repository (branch/tag: $Branch)..."
git clone --recursive --depth 1 --branch $Branch https://github.com/$Repository.git $tempDir

if (-not $?)
{
    Write-Error "Failed to clone the repository. Make sure git is installed and the branch/tag name is correct."
    exit 1
}

# Ensure the target directory exists
if (-not (Test-Path -Path $targetHeaderDir))
{
    Write-Host "Creating target directory for headers..."
    New-Item -Path $targetHeaderDir -ItemType Directory -Force | Out-Null
}
else
{
    # Clear existing header files
    Write-Host "Clearing existing header files from target directory..."
    Remove-Item -Path "$targetHeaderDir\*" -Recurse -Force
}

# Copy header files from libobs to the target directory
$sourceHeaderDir = Join-Path -Path $tempDir -ChildPath "libobs"
Write-Host "Copying header files from $sourceHeaderDir to $targetHeaderDir..."

# Get all header files (*.h, *.hpp, etc.)
$headerFiles = Get-ChildItem -Path $sourceHeaderDir -Include @("*.h", "*.hpp") -Recurse
$headerCount = $headerFiles.Count
Write-Host "Found $headerCount header files."

foreach ($file in $headerFiles)
{
    $relativePath = $file.FullName.Substring($sourceHeaderDir.Length + 1)
    $destination = Join-Path -Path $targetHeaderDir -ChildPath $relativePath

    # Ensure the destination directory exists
    $destinationDir = Split-Path -Path $destination -Parent
    if (-not (Test-Path -Path $destinationDir))
    {
        New-Item -Path $destinationDir -ItemType Directory -Force | Out-Null
    }

    Copy-Item -Path $file.FullName -Destination $destination -Force
}

Write-Host "Configuring CMake for libobs..."
Push-Location $tempDir
try
{
    cmake -DENABLE_PLUGINS=OFF -DENABLE_UI=OFF -DENABLE_SCRIPTING=OFF -DENABLE_HEVC=OFF -DENABLE_FRONTEND=OFF --preset windows-x64
    cmake --build --preset windows-x64
}
finally
{
    Pop-Location
}

Copy-Item $tempDir/build_x64/libobs/RelWithDebInfo/obs.lib $PSScriptRoot/../
git clone "https://github.com/sshcrack/dummy-dll-generator" --depth 1 $tempDir/dummy-dll

Push-Location $PSScriptRoot/../../libobs-bootstrapper/assets/
try
{
    . $tempDir/dummy-dll/dummyDLL.exe $tempDir/build_x64/libobs/RelWithDebInfo/obs.dll
    Move-Item out.dll obs-dummy.dll -Force
    Remove-Item out.exp -Force
    Remove-Item out.lib -Force
}
finally
{
    Pop-Location
}



# Write a small version header file with fixed defines
$versionHeaderPath = Join-Path -Path $PSScriptRoot -ChildPath "../headers/obs/obsconfig.h"
$versionHeaderDir = Split-Path -Path $versionHeaderPath -Parent
if (-not (Test-Path -Path $versionHeaderDir))
{
    New-Item -Path $versionHeaderDir -ItemType Directory -Force | Out-Null
}

$versionHeaderContent = @'
#pragma once

#define OBS_VERSION "unknown"
#define OBS_DATA_PATH "../../data"
#define OBS_INSTALL_PREFIX ""
#define OBS_PLUGIN_DESTINATION "obs-plugins"
#define OBS_RELATIVE_PREFIX "../../"
#define OBS_RELEASE_CANDIDATE 0
#define OBS_BETA 0
'@

Write-Host "Writing version header to $versionHeaderPath"
Set-Content -LiteralPath $versionHeaderPath -Value $versionHeaderContent -Encoding UTF8

git -C $PSScriptRoot/../ apply $PSScriptRoot/patches/001_gh_action_fix_compile.patch

# Build bindings and copy to src/bindings.rs
Write-Host "Building bindings..."
cargo build --features generate_bindings --target-dir $tempDir --release

# Get the bindings.rs file
$bindings = Get-ChildItem -Path $tempDir/release/build -Recurse -Filter "bindings.rs" |
        Where-Object { $_.FullName -match "libobs-[^\\]+\\out\\bindings.rs" } |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

if ($bindings)
{
    Write-Host "Found: $( $bindings.FullName )"
    Write-Host "Copying to: $targetBindingsPath"
    Copy-Item -Path $bindings.FullName -Destination $targetBindingsPath -Force
}
else
{
    Write-Warning "No bindings.rs file found for libobs-* in $buildPath"
}

Write-Host "Cleaning up temporary directory..."
Remove-Item -Path $tempDir -Recurse -Force

Write-Host "Updating mock files..."
Invoke-WebRequest "https://api.github.com/repos/libobs-rs/libobs-builds/releases" -OutFile $PSScriptRoot/../../libobs-bootstrapper/mock_responses/libobs_builds_release.json
Invoke-WebRequest "https://api.github.com/repos/obsproject/obs-studio/releases" -OutFile $PSScriptRoot/../../cargo-obs-build/mock_responses/obs_studio_release.json
Invoke-WebRequest "https://api.github.com/repos/obsproject/obs-studio/releases/latest" -OutFile $PSScriptRoot/../../cargo-obs-build/mock_responses/obs_studio_release_latest.json

Write-Host "Header files updated successfully!"
