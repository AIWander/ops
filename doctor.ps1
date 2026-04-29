# ops MCP Server - Health Check (doctor.ps1)
# Verifies binary location, state directory writability, Claude Desktop config, and binary version.

$ErrorActionPreference = "Continue"
$passed = 0
$failed = 0
$warnings = 0

function Write-Check {
    param([string]$Label, [string]$Status, [string]$Detail)
    switch ($Status) {
        "PASS" {
            Write-Host "  [PASS] " -ForegroundColor Green -NoNewline
            $script:passed++
        }
        "FAIL" {
            Write-Host "  [FAIL] " -ForegroundColor Red -NoNewline
            $script:failed++
        }
        "WARN" {
            Write-Host "  [WARN] " -ForegroundColor Yellow -NoNewline
            $script:warnings++
        }
    }
    Write-Host "$Label" -NoNewline
    if ($Detail) { Write-Host " - $Detail" -ForegroundColor DarkGray } else { Write-Host "" }
}

Write-Host ""
Write-Host "ops MCP Server - Doctor" -ForegroundColor Cyan
Write-Host "=======================" -ForegroundColor Cyan
Write-Host ""

# --- Check 1: Binary exists ---
$binaryPaths = @(
    "C:\Program Files\Ops\ops.exe",
    (Join-Path $env:LOCALAPPDATA "Ops\ops.exe")
)
$binaryFound = $null
foreach ($bp in $binaryPaths) {
    if (Test-Path $bp) {
        $size = [math]::Round((Get-Item $bp).Length / 1KB)
        Write-Check "Binary found" "PASS" "$bp ($size KB)"
        $binaryFound = $bp
        break
    }
}
if (-not $binaryFound) {
    Write-Check "Binary not found" "FAIL" "Expected at '$($binaryPaths[0])' or '$($binaryPaths[1])'"
    Write-Host "  Run install-ops-x64.exe or place ops.exe manually." -ForegroundColor DarkGray
}

# --- Check 2: State directory writable ---
$stateDir = Join-Path $env:LOCALAPPDATA "Ops"
if (-not (Test-Path $stateDir)) {
    try {
        New-Item -ItemType Directory -Path $stateDir -Force | Out-Null
        Write-Check "State directory created" "PASS" $stateDir
    } catch {
        Write-Check "Cannot create state directory" "FAIL" $stateDir
    }
} else {
    Write-Check "State directory exists" "PASS" $stateDir
}

$testFile = Join-Path $stateDir "doctor_test.tmp"
try {
    Set-Content -Path $testFile -Value "doctor" -ErrorAction Stop
    Remove-Item $testFile -ErrorAction SilentlyContinue
    Write-Check "State directory writable" "PASS" ""
} catch {
    Write-Check "State directory not writable" "FAIL" $stateDir
}

# --- Check 3: Claude Desktop config has ops entry ---
$configPath = Join-Path $env:APPDATA "Claude\claude_desktop_config.json"
if (Test-Path $configPath) {
    try {
        $config = Get-Content $configPath -Raw | ConvertFrom-Json
        $opsEntry = $config.mcpServers.ops
        if ($opsEntry) {
            $cmd = $opsEntry.command
            Write-Check "ops in Claude Desktop config" "PASS" "command: $cmd"
            # Cross-check that command path matches binary
            if ($binaryFound -and $cmd -ne $binaryFound) {
                Write-Check "Config path matches binary" "WARN" "Config points to '$cmd' but binary found at '$binaryFound'"
            } elseif ($binaryFound) {
                Write-Check "Config path matches binary" "PASS" ""
            }
        } else {
            Write-Check "ops entry missing from config" "FAIL" "Run install-ops-x64.exe to add it"
        }
    } catch {
        Write-Check "Could not parse Claude Desktop config" "WARN" $configPath
    }
} else {
    Write-Check "Claude Desktop config not found" "WARN" "Expected at $configPath"
}

# --- Check 4: Binary --version ---
if ($binaryFound) {
    try {
        $version = & $binaryFound --version 2>&1
        if ($version -match "ops") {
            Write-Check "Binary version" "PASS" "$version"
        } else {
            Write-Check "Binary version check" "WARN" "Unexpected output: $version"
        }
    } catch {
        Write-Check "Binary failed to run" "FAIL" "$binaryFound"
    }
}

# --- Check 5: Git available (optional, used by some session tools) ---
$gitVersion = $null
try { $gitVersion = & git --version 2>&1 } catch {}
if ($gitVersion -and $gitVersion -match "git version") {
    Write-Check "Git available" "PASS" "$gitVersion"
} else {
    Write-Check "Git not found" "WARN" "Some session tools use git - install from https://git-scm.com"
}

# --- Summary ---
Write-Host ""
Write-Host "Results: " -NoNewline
Write-Host "$passed passed" -ForegroundColor Green -NoNewline
if ($failed -gt 0) {
    Write-Host ", $failed failed" -ForegroundColor Red -NoNewline
}
if ($warnings -gt 0) {
    Write-Host ", $warnings warnings" -ForegroundColor Yellow -NoNewline
}
Write-Host ""

if ($failed -gt 0) {
    Write-Host "Fix the failures above before using ops." -ForegroundColor Red
    exit 1
} else {
    Write-Host "ops is ready." -ForegroundColor Green
    exit 0
}
