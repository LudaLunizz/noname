$ErrorActionPreference = "Stop"

$repo = "Silvertree2010/noname"
$dir = if ($env:NONAME_INSTALL_DIR) { $env:NONAME_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "noname\bin" }
$arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64" } else { "x86_64" }
$target = "$arch-pc-windows-msvc"
$url = "https://github.com/$repo/releases/latest/download/noname-$target.zip"
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("noname-" + [guid]::NewGuid())

New-Item -ItemType Directory -Force $tmp | Out-Null
Write-Host "downloading noname for $target"
Invoke-WebRequest -UseBasicParsing $url -OutFile (Join-Path $tmp "noname.zip")
Expand-Archive (Join-Path $tmp "noname.zip") -DestinationPath $tmp -Force
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item (Join-Path $tmp "noname.exe") (Join-Path $dir "noname.exe") -Force
Remove-Item -Recurse -Force $tmp
Write-Host "installed to $dir\noname.exe"

$path = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not ($path -split ";" | Where-Object { $_ -eq $dir })) {
    [Environment]::SetEnvironmentVariable("Path", "$path;$dir", "User")
    Write-Host "added $dir to your PATH, open a new terminal to use noname"
}

if (-not (Get-Command claude -ErrorAction SilentlyContinue)) {
    Write-Host "noname needs Claude Code: https://claude.com/claude-code"
}
