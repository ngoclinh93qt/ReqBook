param(
    [string]$Version = "latest",
    [string]$Repo = "mark-api-down/mad",
    [string]$InstallDir = "$HOME\.local\bin"
)

$ErrorActionPreference = "Stop"
$arch = if ([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq "Arm64") { "aarch64" } else { "x86_64" }
$target = "$arch-pc-windows-msvc"
$archive = "mad-$target.zip"

if ($Version -eq "latest") {
    $baseUrl = "https://github.com/$Repo/releases/latest/download"
} else {
    $baseUrl = "https://github.com/$Repo/releases/download/$Version"
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) "mad-install-$PID"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
    $archivePath = Join-Path $tmp $archive
    Invoke-WebRequest -Uri "$baseUrl/$archive" -OutFile $archivePath

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
    $bin = Get-ChildItem -Recurse -Path $tmp -Filter "mad.exe" | Select-Object -First 1
    if (-not $bin) {
        throw "archive did not contain mad.exe"
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Force $bin.FullName (Join-Path $InstallDir "mad.exe")
    & (Join-Path $InstallDir "mad.exe") version
    Write-Host "Installed MarkApiDown to $InstallDir"
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
