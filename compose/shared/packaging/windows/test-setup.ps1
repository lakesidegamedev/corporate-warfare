# CI check of the setup program on a Windows runner: install, upgrade/downgrade over another
# version, uninstall, and the saves survive all of it. Fails with a message on the first problem.
param(
    [Parameter(Mandatory)] [string] $Setup,
    [Parameter(Mandatory)] [string] $AppImage,
    [Parameter(Mandatory)] [string] $Work
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$appId = '{7C1E9A52-4D3B-4F6A-8E21-3A9D5B0C6F47}_is1'
$key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$appId"
$target = Join-Path $Work 'game'
$saves = Join-Path $env:APPDATA 'Corporate Warfare'
$marker = Join-Path $saves 'careers\installer-test.sav'

function Check($ok, $message) { if (-not $ok) { throw "FAILED: $message" } else { Write-Host "ok: $message" } }
function Run($file, $arguments) {
    $p = Start-Process -FilePath $file -ArgumentList $arguments -Wait -PassThru
    Check ($p.ExitCode -eq 0) "$(Split-Path -Leaf $file) $arguments exited 0 (was $($p.ExitCode))"
}
function Shown { (Get-ItemProperty -LiteralPath $key).DisplayVersion }

Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Work | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $marker) | Out-Null
Set-Content -Path $marker -Value 'saved career'

# An older build of the same game, made from the same files.
$oldDir = Join-Path $Work 'old'
& (Join-Path $here 'build-setup.ps1') -DisplayVersion '1.0.0-test' -NumericVersion '1.0.0' -AppImage $AppImage -OutDir $oldDir
$old = (Get-ChildItem $oldDir -Filter '*-setup.exe').FullName
Check ($old -ne $null) 'older test setup built'

$quiet = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER'

# 1. Fresh install, with the desktop shortcut.
Run $Setup "$quiet /DIR=`"$target`" /TASKS=desktopicon,startmenu"
Check (Test-Path (Join-Path $target 'Corporate Warfare.exe')) 'game exe installed'
Check (Test-Path (Join-Path ([Environment]::GetFolderPath('Programs')) 'Corporate Warfare.lnk')) 'start menu shortcut exists'
Check (Test-Path (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Corporate Warfare.lnk')) 'desktop shortcut exists'
$newVersion = Shown
Check (Test-Path $marker) 'saves intact after install'

# 2. An older version over the newer one, into the same folder, with a leftover file from "newer".
Set-Content -Path (Join-Path $target 'app\leftover-from-newer.txt') -Value 'x'
Run $old $quiet
Check ((Shown) -eq '1.0.0-test') "downgrade changed the installed version ($newVersion to $(Shown))"
Check (Test-Path (Join-Path $target 'Corporate Warfare.exe')) 'game exe present after downgrade'
Check (-not (Test-Path (Join-Path $target 'app\leftover-from-newer.txt'))) 'stale program file removed by the downgrade'
Check (Test-Path $marker) 'saves intact after downgrade'

# 3. And forward again.
Run $Setup $quiet
Check ((Shown) -eq $newVersion) 'upgrade back to the new version'
Check (Test-Path $marker) 'saves intact after upgrade'

# 4. Silent uninstall always keeps the saves.
Run (Join-Path $target 'unins000.exe') $quiet
Check (-not (Test-Path (Join-Path $target 'Corporate Warfare.exe'))) 'game exe removed by uninstall'
Check (-not (Test-Path (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Corporate Warfare.lnk'))) 'desktop shortcut removed'
Check (Test-Path $marker) 'saves survived the silent uninstall'
Remove-Item -Recurse -Force $saves -ErrorAction SilentlyContinue
Write-Host 'Setup program checks passed.'
