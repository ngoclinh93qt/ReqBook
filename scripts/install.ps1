param(
    [string]$Version = "latest",
    [string]$Repo = "ngoclinh93qt/ReqBook",
    [string]$InstallDir = "$HOME\.local\bin"
)

$ErrorActionPreference = "Stop"
if (-not $PSBoundParameters.ContainsKey("Repo")) {
    if ($env:RQB_REPO) { $Repo = $env:RQB_REPO }
    elseif ($env:MAD_REPO) { $Repo = $env:MAD_REPO }
}
if (-not $PSBoundParameters.ContainsKey("InstallDir")) {
    if ($env:RQB_INSTALL_DIR) { $InstallDir = $env:RQB_INSTALL_DIR }
    elseif ($env:MAD_INSTALL_DIR) { $InstallDir = $env:MAD_INSTALL_DIR }
}
$arch = if ([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq "Arm64") { "aarch64" } else { "x86_64" }
$target = "$arch-pc-windows-msvc"
$archive = "rqb-$target.zip"

if ($Version -eq "latest") {
    $baseUrl = "https://github.com/$Repo/releases/latest/download"
} else {
    $baseUrl = "https://github.com/$Repo/releases/download/$Version"
}

function Invoke-RequiredDownload {
    param(
        [string]$Url,
        [string]$OutFile,
        [string]$AssetName
    )

    try {
        Invoke-WebRequest -Uri $Url -OutFile $OutFile
    } catch {
        $hint = "Check https://github.com/$Repo/releases and rerun the release workflow if the asset is missing."
        if ($Version -eq "latest") {
            $hint = "The latest GitHub release may not include binaries for this platform yet. $hint"
        }
        throw "failed to download Reqbook release asset`nasset: $AssetName`nurl: $Url`n$hint"
    }
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) "rqb-install-$PID"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
    $archivePath = Join-Path $tmp $archive
    Invoke-RequiredDownload -Url "$baseUrl/$archive" -OutFile $archivePath -AssetName $archive

    $checksumPath = Join-Path $tmp "$archive.sha256"
    try {
        Invoke-WebRequest -Uri "$baseUrl/$archive.sha256" -OutFile $checksumPath
        $expected = (Get-Content $checksumPath).Split(" ")[0].Trim()
        $actual = (Get-FileHash -Algorithm SHA256 $archivePath).Hash.ToLowerInvariant()
        if ($expected.ToLowerInvariant() -ne $actual) {
            throw "checksum mismatch"
        }
    } catch {
        Write-Warning "checksum verification skipped or failed: $_"
    }

    Expand-Archive -Force -Path $archivePath -DestinationPath $tmp
    $bin = Get-ChildItem -Recurse -Path $tmp -Filter "rqb.exe" | Select-Object -First 1
    if (-not $bin) {
        throw "archive did not contain rqb.exe"
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Force $bin.FullName (Join-Path $InstallDir "rqb.exe")
    & (Join-Path $InstallDir "rqb.exe") version
    Write-Host "Installed Reqbook to $InstallDir"
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
