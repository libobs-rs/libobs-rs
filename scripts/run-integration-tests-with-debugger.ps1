#!/usr/bin/env pwsh

<#
.SYNOPSIS
    Build and run all integration tests with debugger/error catching

.DESCRIPTION
    This script:
    1. Builds all integration tests in the workspace
    2. Runs each test binary with error/exception catching
    3. Continues running all tests even if some fail
    4. Returns a failure code if any test failed

.PARAMETER Features
    Cargo features to enable (default: "__test_environment")

.PARAMETER NoDefaultFeatures
    Disable default features

.PARAMETER AllFeatures
    Enable all features

.PARAMETER Package
    Specific package to test (default: all workspace packages)

.EXAMPLE
    .\run-integration-tests-with-debugger.ps1

.EXAMPLE
    .\run-integration-tests-with-debugger.ps1 -AllFeatures
#>

param(
    [string]$Features = "__test_environment",
    [switch]$NoDefaultFeatures,
    [switch]$AllFeatures,
    [string]$Package = ""
)

$ErrorActionPreference = "Continue"
Set-StrictMode -Version Latest

# Colors for output
function Write-ColorOutput($ForegroundColor) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    if ($args) {
        Write-Output $args
    }
    $host.UI.RawUI.ForegroundColor = $fc
}

function Write-Success { Write-ColorOutput Green @args }
function Write-Info { Write-ColorOutput Cyan @args }
function Write-Warning { Write-ColorOutput Yellow @args }
function Write-Failure { Write-ColorOutput Red @args }

# Track results
$script:TotalTests = 0
$script:PassedTests = 0
$script:FailedTests = 0
$script:FailedTestsList = @()

Write-Info "=================================="
Write-Info "Integration Test Runner with Debugger"
Write-Info "=================================="
Write-Info ""

# Build cargo arguments
$cargoArgs = @("test", "--no-run", "--tests")

if ($NoDefaultFeatures) {
    $cargoArgs += "--no-default-features"
}

if ($AllFeatures) {
    $cargoArgs += "--all-features"
} elseif ($Features) {
    $cargoArgs += "--features"
    $cargoArgs += $Features
}

if ($Package) {
    $cargoArgs += "-p"
    $cargoArgs += $Package
}

# Add message format for parsing
$cargoArgs += "--message-format=json"

Write-Info "Step 1: Building all integration tests..."
Write-Info "Command: cargo $($cargoArgs -join ' ')"
Write-Info ""

# Build tests and capture output
$buildOutput = & cargo @cargoArgs 2>&1 | Out-String
$buildExitCode = $LASTEXITCODE

if ($buildExitCode -ne 0) {
    Write-Failure "Build failed with exit code $buildExitCode"
    Write-Output $buildOutput
    exit $buildExitCode
}

Write-Success "✓ Build completed successfully"
Write-Info ""

# Parse JSON output to find test executables
Write-Info "Step 2: Locating test executables..."
$testExecutables = @()

$buildOutput -split "`n" | ForEach-Object {
    if ($_ -match '^\s*{') {
        try {
            $json = $_ | ConvertFrom-Json
            if ($json.executable -and $json.profile.test -eq $true) {
                $exe = $json.executable
                if ($exe -and (Test-Path $exe)) {
                    $testExecutables += $exe
                }
            }
        } catch {
            # Ignore JSON parsing errors
        }
    }
}

# Remove duplicates
$testExecutables = $testExecutables | Select-Object -Unique

if ($testExecutables.Count -eq 0) {
    Write-Warning "No test executables found. This might be normal if there are no integration tests."
    exit 0
}

Write-Info "Found $($testExecutables.Count) test executable(s)"
Write-Info ""

# Find debugger (cdb.exe from Windows SDK)
function Find-Debugger {
    # Common locations for Windows SDK debuggers
    $possiblePaths = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\Debuggers\x64\cdb.exe",
        "${env:ProgramFiles}\Windows Kits\10\Debuggers\x64\cdb.exe",
        "${env:ProgramFiles(x86)}\Windows Kits\10\Debuggers\x86\cdb.exe",
        "${env:ProgramFiles}\Windows Kits\10\Debuggers\x86\cdb.exe"
    )
    
    foreach ($path in $possiblePaths) {
        if (Test-Path $path) {
            return $path
        }
    }
    
    # Try to find it in PATH
    $cdbInPath = Get-Command cdb.exe -ErrorAction SilentlyContinue
    if ($cdbInPath) {
        return $cdbInPath.Source
    }
    
    return $null
}

$debuggerPath = Find-Debugger

if (-not $debuggerPath) {
    Write-Warning "Windows Debugger (cdb.exe) not found. Running tests without debugger."
    Write-Warning "Install Windows SDK to enable debugger support: https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/"
    Write-Info ""
    $useDebugger = $false
} else {
    Write-Info "Found debugger: $debuggerPath"
    Write-Info ""
    $useDebugger = $true
}

# Step 3: Run each test executable with error catching
Write-Info "Step 3: Running tests with debugger/error catching..."
Write-Info ""

foreach ($exe in $testExecutables) {
    $exeName = Split-Path $exe -Leaf
    $exeDir = Split-Path $exe -Parent
    $script:TotalTests++
    
    Write-Info "----------------------------------------"
    Write-Info "Running: $exeName"
    Write-Info "Path: $exe"
    Write-Info "Working Dir: $exeDir"
    Write-Info "----------------------------------------"
    
    # Set environment variable to enable crash dumps/better error messages
    $env:RUST_BACKTRACE = "full"
    
    $exitCode = 0
    $hadException = $false
    $exceptionDetails = ""
    
    if ($useDebugger) {
        # Create a temporary script for the debugger
        $debuggerScript = @"
.logopen "$exeDir\debugger_log.txt"
sxe av
sxe sov
sxe eh
sxe *
g
q
"@
        $scriptPath = Join-Path $env:TEMP "cdb_script_$([guid]::NewGuid().ToString()).txt"
        $debuggerScript | Out-File -FilePath $scriptPath -Encoding ASCII
        
        # Run with debugger
        # -g: go on initial breakpoint
        # -G: go on final breakpoint  
        # -o: debug child processes
        # -c: execute command on start
        # -cf: execute commands from file
        $debuggerArgs = @(
            "-g",
            "-G",
            "-o",
            "-cf", $scriptPath,
            $exe,
            "--nocapture",
            "--test-threads=1"
        )
        
        try {
            $process = Start-Process -FilePath $debuggerPath `
                -ArgumentList $debuggerArgs `
                -WorkingDirectory $exeDir `
                -Wait `
                -NoNewWindow `
                -PassThru `
                -RedirectStandardOutput "$exeDir\test_output.txt" `
                -RedirectStandardError "$exeDir\test_error.txt"
            
            $exitCode = $process.ExitCode
            
            # Display output
            if (Test-Path "$exeDir\test_output.txt") {
                Get-Content "$exeDir\test_output.txt" | Write-Output
                Remove-Item "$exeDir\test_output.txt" -ErrorAction SilentlyContinue
            }
            if (Test-Path "$exeDir\test_error.txt") {
                Get-Content "$exeDir\test_error.txt" | Write-Output
                Remove-Item "$exeDir\test_error.txt" -ErrorAction SilentlyContinue
            }
            
            # Check debugger log for exceptions and critical errors
            if (Test-Path "$exeDir\debugger_log.txt") {
                $debugLog = Get-Content "$exeDir\debugger_log.txt" -Raw
                
                # Check for various critical errors and exceptions
                $criticalPatterns = @(
                    "Access violation",
                    "Stack overflow",
                    "Invalid handle",
                    "STATUS_\w+",
                    "Critical error detected",
                    "c0000374",  # Heap corruption
                    "RtlReportCriticalFailure",
                    "RtlReportFatalFailure",
                    "Fatal",
                    "heap corruption",
                    "Unknown exception"
                )
                
                $pattern = "($($criticalPatterns -join '|'))"
                if ($debugLog -match $pattern) {
                    $hadException = $true
                    $matchedError = $matches[0]
                    $exceptionDetails = "Debugger detected: $matchedError"
                    Write-Warning "Critical error found in debugger log: $matchedError"
                }
                Remove-Item "$exeDir\debugger_log.txt" -ErrorAction SilentlyContinue
            }
        } catch {
            Write-Failure "✗ Exception running debugger: $($_.Exception.Message)"
            $hadException = $true
            $exceptionDetails = $_.Exception.Message
        } finally {
            Remove-Item $scriptPath -ErrorAction SilentlyContinue
        }
    } else {
        # Run without debugger
        try {
            $process = Start-Process -FilePath $exe `
                -ArgumentList @("--nocapture", "--test-threads=1") `
                -WorkingDirectory $exeDir `
                -Wait `
                -NoNewWindow `
                -PassThru `
                -RedirectStandardOutput "$exeDir\test_output.txt" `
                -RedirectStandardError "$exeDir\test_error.txt"
            
            $exitCode = $process.ExitCode
            
            # Display output
            if (Test-Path "$exeDir\test_output.txt") {
                Get-Content "$exeDir\test_output.txt" | Write-Output
                Remove-Item "$exeDir\test_output.txt" -ErrorAction SilentlyContinue
            }
            if (Test-Path "$exeDir\test_error.txt") {
                Get-Content "$exeDir\test_error.txt" | Write-Output
                Remove-Item "$exeDir\test_error.txt" -ErrorAction SilentlyContinue
            }
        } catch {
            Write-Failure "✗ Exception occurred: $($_.Exception.Message)"
            $hadException = $true
            $exceptionDetails = $_.Exception.Message
        }
    }
    
    # Evaluate results
    if ($hadException) {
        Write-Failure "✗ Test crashed with exception: $exceptionDetails"
        $script:FailedTests++
        $script:FailedTestsList += "$exeName (exception: $exceptionDetails)"
    } elseif ($exitCode -eq 0) {
        Write-Success "✓ Test passed (exit code: 0)"
        $script:PassedTests++
    } else {
        Write-Failure "✗ Test failed (exit code: $exitCode)"
        $script:FailedTests++
        $script:FailedTestsList += "$exeName (exit code: $exitCode)"
        
        # Check for specific error codes
        switch ($exitCode) {
            -1073741819 { Write-Failure "  Access Violation (0xC0000005)" }
            -1073740791 { Write-Failure "  Stack Overflow (0xC00000FD)" }
            -1073741571 { Write-Failure "  Stack Buffer Overrun (0xC0000409)" }
            -1073741676 { Write-Failure "  Divide by Zero (0xC0000094)" }
            101 { Write-Warning "  Some tests failed" }
        }
    }
    
    Write-Info ""
}

# Summary
Write-Info "=========================================="
Write-Info "Test Summary"
Write-Info "=========================================="
Write-Info "Total test executables: $script:TotalTests"
Write-Success "Passed: $script:PassedTests"

if ($script:FailedTests -gt 0) {
    Write-Failure "Failed: $script:FailedTests"
    Write-Info ""
    Write-Failure "Failed tests:"
    foreach ($failed in $script:FailedTestsList) {
        Write-Failure "  - $failed"
    }
}

Write-Info "=========================================="
Write-Info ""

# Exit with appropriate code
if ($script:FailedTests -gt 0) {
    Write-Failure "Integration tests FAILED"
    exit 1
} else {
    Write-Success "All integration tests PASSED"
    exit 0
}
