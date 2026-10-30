# Download the newest published codex-fork-installer for Windows and run it.
& {
    Set-StrictMode -Version Latest
    $ErrorActionPreference = "Stop"
    $ProgressPreference = "SilentlyContinue"

    if ($env:OS -ne "Windows_NT") {
        throw "install-fork.ps1 supports Windows only. Use install-fork.sh on macOS, Linux, and Android/Termux."
    }

    $repo = if ($env:CODEX_FORK_REPO) { $env:CODEX_FORK_REPO } else { "mevanlc/codex" }
    $apiUrl = if ($env:CODEX_GITHUB_API_URL) { $env:CODEX_GITHUB_API_URL.TrimEnd("/") } else { "https://api.github.com" }
    $releasesUrl = if ($env:CODEX_GITHUB_RELEASES_URL) { $env:CODEX_GITHUB_RELEASES_URL.TrimEnd("/") } else { "https://github.com/$repo/releases" }

    $target = $null
    if (-not [string]::IsNullOrWhiteSpace($env:CODEX_ARTIFACT_NAME)) {
        $target = $env:CODEX_ARTIFACT_NAME -replace "^codex-", ""
    } else {
        $arch = try {
            [string][System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
        } catch {
            if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
        }
        switch -Regex ($arch) {
            "^(?i:Arm64)$" { $target = "aarch64-pc-windows-msvc" }
            "^(?i:X64|AMD64)$" { $target = "x86_64-pc-windows-msvc" }
            default {
                throw "Unsupported Windows architecture '$arch'; set CODEX_ARTIFACT_NAME"
            }
        }
    }

    $asset = "codex-fork-installer-$target.exe"
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $token = if ($env:GH_TOKEN) { $env:GH_TOKEN } elseif ($env:GITHUB_TOKEN) { $env:GITHUB_TOKEN } else { $null }
    $apiHeaders = @{
        "Accept"     = "application/vnd.github+json"
        "User-Agent" = "codex-fork-installer-bootstrap"
    }
    $downloadHeaders = @{
        "User-Agent" = "codex-fork-installer-bootstrap"
    }
    if ($token) {
        $apiHeaders["Authorization"] = "Bearer $token"
        $downloadHeaders["Authorization"] = "Bearer $token"
    }

    $releases = $null
    $apiSucceeded = $false
    try {
        $releases = Invoke-RestMethod -Uri "$apiUrl/repos/$repo/releases?per_page=100" -Headers $apiHeaders -TimeoutSec 30
        $apiSucceeded = $true
    } catch {
        $apiSucceeded = $false
    }

    $downloadUrl = $null
    $expectedSha256 = $null
    if ($apiSucceeded) {
        $matchingAsset = $null
        $candidates = @($releases) | Where-Object {
            $null -ne $_ -and
            -not $_.draft -and
            -not $_.prerelease -and
            $_.published_at -and
            ([string]$_.tag_name -match "^fork-v\d+\.\d+\.\d+")
        } | Sort-Object { [DateTimeOffset]::Parse([string]$_.published_at).UtcTicks } -Descending

        foreach ($release in $candidates) {
            $found = @($release.assets) | Where-Object { $_.name -eq $asset } | Select-Object -First 1
            if ($null -ne $found -and $found.browser_download_url) {
                $matchingAsset = $found
                break
            }
        }

        if ($null -eq $matchingAsset) {
            throw "No published release of $repo carries $asset"
        }

        $downloadUrl = [string]$matchingAsset.browser_download_url
        $digest = [string]$matchingAsset.digest
        if ($digest -notmatch "^sha256:([0-9a-fA-F]{64})$") {
            throw "GitHub published no SHA-256 digest for $asset"
        }
        $expectedSha256 = $Matches[1].ToLowerInvariant()
    } else {
        $downloadUrl = "$releasesUrl/latest/download/$asset"
    }

    $tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-fork-installer-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
    try {
        $installerPath = Join-Path $tempDir $asset
        Invoke-WebRequest -UseBasicParsing -Uri $downloadUrl -Headers $downloadHeaders -OutFile $installerPath

        if ($expectedSha256) {
            $actualSha256 = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($actualSha256 -ne $expectedSha256) {
                throw "$asset does not match its published digest (expected sha256:$expectedSha256, got sha256:$actualSha256)"
            }
        }

        $installerArgs = @($args | ForEach-Object {
            if ($_ -eq "-?") { "--help" } else { $_ }
        })
        & $installerPath @installerArgs
    } finally {
        Remove-Item -LiteralPath $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
} @args

# Keep stdout on the caller's pipeline and read the native exit status separately.
$exitCode = $LASTEXITCODE
if ($exitCode -ne 0) {
    if ($MyInvocation.MyCommand.Path) {
        exit $exitCode
    }
    throw "codex-fork-installer exited with code $exitCode"
}
