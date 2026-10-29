# Builds CorporateWarfare-<version>-setup.exe from the jpackage app image with Inno Setup 6.
# Run by the :shared:packageWindowsSetup Gradle task, which passes the version and folders.
param(
    [Parameter(Mandatory)] [string] $DisplayVersion,
    [Parameter(Mandatory)] [string] $NumericVersion,
    [Parameter(Mandatory)] [string] $AppImage,
    [Parameter(Mandatory)] [string] $OutDir
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = (Resolve-Path (Join-Path $here '..\..\..\..')).Path

function Find-Iscc {
    $found = Get-Command iscc.exe -ErrorAction SilentlyContinue
    if ($found) { return $found.Source }
    foreach ($dir in @($env:ProgramFiles, ${env:ProgramFiles(x86)}, "$env:LOCALAPPDATA\Programs")) {
        if (-not $dir) { continue }
        $path = Join-Path $dir 'Inno Setup 6\ISCC.exe'
        if (Test-Path $path) { return $path }
    }
    return $null
}

$iscc = Find-Iscc
if (-not $iscc) {
    Write-Host 'Inno Setup 6 not found; installing it with Chocolatey.'
    choco install innosetup --no-progress -y | Out-Host
    $iscc = Find-Iscc
}
if (-not $iscc) { throw 'Inno Setup 6 (ISCC.exe) is not installed and could not be installed.' }
if (-not (Test-Path (Join-Path $AppImage 'Corporate Warfare.exe'))) { throw "No app image at $AppImage" }

$art = Join-Path $OutDir 'art'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
python -c "import PIL" 2>$null
if ($LASTEXITCODE -ne 0) {
    python -m pip install --quiet --disable-pip-version-check pillow
    if ($LASTEXITCODE -ne 0) { throw 'Could not install Pillow for installer_art.py' }
}
python (Join-Path $repo 'tools\installer_art.py') $art
if ($LASTEXITCODE -ne 0) { throw 'installer_art.py failed' }

& $iscc "/DDisplayVersion=$DisplayVersion" "/DNumericVersion=$NumericVersion" "/DAppImage=$AppImage" "/DArt=$art" "/DOutDir=$OutDir" (Join-Path $here 'setup.iss')
if ($LASTEXITCODE -ne 0) { throw "ISCC failed with exit code $LASTEXITCODE" }
Get-ChildItem $OutDir -Filter '*-setup.exe' | ForEach-Object { Write-Host "Built $($_.FullName)" }
