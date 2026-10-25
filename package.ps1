# Builds Innert and packs it into dist\Innert.zip, ready to send to someone: the game, the Visual
# C++ runtime it needs so they don't have to install it, and a readme. Run it with
#   powershell -ExecutionPolicy Bypass -File package.ps1

Set-Location $PSScriptRoot

# Cargo reports its progress on stderr, which Windows PowerShell would otherwise take for errors
$ErrorActionPreference = 'Continue'
cargo build --release 2>&1 | ForEach-Object { "$_" }
if ($LASTEXITCODE -ne 0) { throw 'The build failed, see above. Is the game still running? Close it first.' }
$ErrorActionPreference = 'Stop'

$out = Join-Path $PSScriptRoot 'dist\Innert'
if (Test-Path $out) { Remove-Item $out -Recurse -Force }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item 'target\release\innert.exe' $out

# Microsoft allows these to be shipped next to a program, so it runs without installing them
$studios = @("${env:ProgramFiles(x86)}\Microsoft Visual Studio", "$env:ProgramFiles\Microsoft Visual Studio") | Where-Object { Test-Path $_ }
$runtime = Get-ChildItem $studios -Recurse -Directory -Filter 'Microsoft.VC14*.CRT' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\x64\\' -and $_.FullName -notmatch 'onecore|debug' } |
    Sort-Object FullName -Descending | Select-Object -First 1
if ($runtime) {
    foreach ($dll in 'vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll') {
        Copy-Item (Join-Path $runtime.FullName $dll) $out
    }
} else {
    Write-Warning 'The Visual C++ runtime was not found, so whoever runs it may need to install it: https://aka.ms/vs/17/release/vc_redist.x64.exe'
}

@'
INNERT
======

To play: unzip this folder anywhere and double-click innert.exe.

You need
  - Windows 10 or 11, 64-bit
  - A graphics card that supports Vulkan 1.3. Most NVIDIA, AMD and Intel ones from the last
    several years do. If the game says it can't use your graphics card, install the latest
    driver from NVIDIA, AMD or Intel.
  - Windows may warn that the program is from an unknown publisher: click "More info", then
    "Run anyway".

Controls
  Mouse                  look around
  W A S D                walk           Shift   run          Space   jump
  Left click             attack, shoot a drawn bow, cast a spell
  Right click            heavy attack, draw a bow, eat, pour water
  1-9 or the mouse wheel pick what's in hand
  E                      pick up, open chests, pet
  I or Tab               inventory
  F                      walk or fly
  F1                     settings: lower "Render Scale" there if it runs slowly
  F12                    screenshot
  Esc                    menu
'@ | Set-Content -Encoding utf8 (Join-Path $out 'README.txt')

$zip = Join-Path $PSScriptRoot 'dist\Innert.zip'
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $out -DestinationPath $zip
$size = (Get-Item $zip).Length / 1MB
Write-Host ("Made {0} ({1:N1} MB)" -f $zip, $size)
