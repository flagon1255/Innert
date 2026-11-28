# Installs Innert from the folder this script is in: checks the computer can run it first, then
# copies the game into the user's own programs folder (no administrator needed), adds a Start
# menu shortcut, and a desktop one if wanted, and an entry in Windows' installed apps to remove it
# again. package.ps1 puts it in dist\Innert beside the game, with "Install Innert.cmd" to start it
# by double-clicking. Run by hand:
#   powershell -ExecutionPolicy Bypass -File install.ps1              check, then install
#   powershell -ExecutionPolicy Bypass -File install.ps1 -CheckOnly   only check
#   powershell -ExecutionPolicy Bypass -File install.ps1 -Uninstall   remove it again
# -Desktop adds the desktop shortcut without asking, -Quiet asks nothing at all, -InstallDir picks
# another folder. Saved games and settings in %LOCALAPPDATA%\Innert are never touched.

param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\Innert'),
    [switch]$CheckOnly,
    [switch]$Uninstall,
    [switch]$Desktop,
    [switch]$Quiet,
    # Only copies the game, no shortcuts or installed-apps entry, for trying the script out
    [switch]$Portable
)

$ErrorActionPreference = 'Stop'
$source = $PSScriptRoot
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\InnertScript'
$startMenu = Join-Path ([Environment]::GetFolderPath('Programs')) 'Innert.lnk'
$desktopLink = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Innert.lnk'
$drivers = @{
    'NVIDIA' = 'https://www.nvidia.com/Download/index.aspx'
    'AMD'    = 'https://www.amd.com/en/support/download/drivers.html'
    'Intel'  = 'https://www.intel.com/content/www/us/en/support/detect.html'
}
$runtimeUrl = 'https://aka.ms/vs/17/release/vc_redist.x64.exe'

function Say([string]$text, [string]$color = 'Gray') { Write-Host $text -ForegroundColor $color }
function Pass([string]$text) { Say "  [ ok ] $text" 'Green' }
function Warn([string]$text) { Say "  [warn] $text" 'Yellow' }
function Fail([string]$text) { Say "  [fail] $text" 'Red' }

# Yes or no, `yes` if nothing's typed or -Quiet
function Ask([string]$question, [bool]$yes = $true) {
    if ($Quiet) { return $yes }
    $hint = if ($yes) { '[Y/n]' } else { '[y/N]' }
    $answer = Read-Host "$question $hint"
    if ([string]::IsNullOrWhiteSpace($answer)) { return $yes }
    return $answer.Trim().ToLower().StartsWith('y')
}

# The game's own processes running from `folder`, never any other program's
function Running-From([string]$folder) {
    $full = [IO.Path]::GetFullPath($folder).TrimEnd('\') + '\'
    @(Get-Process -Name 'innert' -ErrorAction SilentlyContinue | Where-Object {
        $_.Path -and $_.Path.StartsWith($full, [StringComparison]::OrdinalIgnoreCase)
    })
}

# Whether the computer can run Innert: each check says what it found, and $false if one fails that
# the game can't do without
function Test-Prerequisites {
    Say 'Checking this computer can run Innert...' 'Cyan'
    $ok = $true

    # Windows 10 or 11, 64-bit
    $os = [Environment]::OSVersion.Version
    if (-not [Environment]::Is64BitOperatingSystem) {
        Fail 'Windows is 32-bit: Innert needs 64-bit Windows 10 or 11.'
        $ok = $false
    } elseif ($os.Major -lt 10) {
        Fail "Windows $($os.Major).$($os.Minor) is too old: Innert needs Windows 10 or 11."
        $ok = $false
    } else {
        $name = if ($os.Build -ge 22000) { 'Windows 11' } else { 'Windows 10' }
        Pass "$name, 64-bit (build $($os.Build))"
    }

    # A graphics card, and which maker's, for its driver page
    $cards = @(Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | Where-Object { $_.Name })
    $real = @($cards | Where-Object { $_.Name -notmatch 'Basic Display|Basic Render|Remote|Virtual|Parsec|Meta' })
    $maker = $null
    foreach ($card in $real) {
        foreach ($m in $drivers.Keys) { if ($card.Name -match $m -or ($m -eq 'AMD' -and $card.Name -match 'Radeon')) { $maker = $m } }
    }
    if ($real.Count -eq 0) {
        Fail 'No graphics card driver found, only the basic display driver: install the driver for your graphics card.'
        $ok = $false
    } else {
        foreach ($card in $real) {
            $vram = if ($card.AdapterRAM -gt 0) { ' ({0:N1} GB reported)' -f ($card.AdapterRAM / 1GB) } else { '' }
            Pass "Graphics: $($card.Name)$vram"
        }
    }

    # Vulkan, which comes with the graphics card's driver
    $vulkan = Join-Path $env:SystemRoot 'System32\vulkan-1.dll'
    if (Test-Path $vulkan) {
        $version = (Get-Item $vulkan).VersionInfo.ProductVersion
        Pass "Vulkan $version"
    } else {
        Fail 'Vulkan was not found. It comes with your graphics card driver: install the latest one.'
        $ok = $false
        $page = if ($maker) { $drivers[$maker] } else { $null }
        if ($page -and (Ask "Open the $maker driver page now?")) { Start-Process $page }
        elseif (-not $page) { $drivers.GetEnumerator() | ForEach-Object { Say "         $($_.Key): $($_.Value)" } }
    }

    # The Visual C++ runtime: it comes beside the game, or else it has to be installed
    $bundled = @('vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll') | Where-Object { -not (Test-Path (Join-Path $source $_)) }
    $installed = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64' -ErrorAction SilentlyContinue).Installed -eq 1
    if ($bundled.Count -eq 0) {
        Pass 'Visual C++ runtime (comes with the game)'
    } elseif ($installed) {
        Pass 'Visual C++ runtime (installed)'
    } else {
        Warn 'The Visual C++ runtime is missing, and the game will not start without it.'
        if (Ask 'Download and run Microsoft''s installer for it now?') {
            $file = Join-Path $env:TEMP 'vc_redist.x64.exe'
            Invoke-WebRequest -UseBasicParsing $runtimeUrl -OutFile $file
            Start-Process $file -ArgumentList '/install', '/passive', '/norestart' -Wait
            Pass 'Visual C++ runtime installed'
        } else {
            Say "         Get it from $runtimeUrl"
            $ok = $false
        }
    }

    # Room for it, with some to spare for saves and the shaders made on the first start
    $drive = [IO.Path]::GetPathRoot([IO.Path]::GetFullPath($InstallDir))
    $free = (Get-PSDrive -Name $drive.Substring(0, 1) -ErrorAction SilentlyContinue).Free
    if ($free -and $free -lt 300MB) {
        Fail ('Only {0:N0} MB free on {1}: Innert needs about 300 MB.' -f ($free / 1MB), $drive)
        $ok = $false
    } elseif ($free) {
        Pass ('{0:N1} GB free on {1}' -f ($free / 1GB), $drive)
    }

    # Memory: it runs on 8 GB, less is tight
    $memory = (Get-CimInstance Win32_ComputerSystem -ErrorAction SilentlyContinue).TotalPhysicalMemory
    if ($memory -and $memory -lt 6GB) {
        Warn ('{0:N1} GB of memory: Innert is made for 8 GB; pick the Low quality in F1 settings.' -f ($memory / 1GB))
    } elseif ($memory) {
        Pass ('{0:N1} GB of memory' -f ($memory / 1GB))
    }
    return $ok
}

function Remove-Innert {
    Say 'Removing Innert...' 'Cyan'
    $dir = $InstallDir
    $entry = Get-ItemProperty $uninstallKey -ErrorAction SilentlyContinue
    if ($entry -and $entry.InstallLocation) { $dir = $entry.InstallLocation }
    if ((Running-From $dir).Count -gt 0) { throw 'Innert is still running: close it first.' }
    foreach ($link in $startMenu, $desktopLink) { if (Test-Path $link) { Remove-Item $link -Force } }
    if (Test-Path $uninstallKey) { Remove-Item $uninstallKey -Force }
    # Only ever a folder that holds the game, so a wrong path can't empty anything else
    if ((Test-Path (Join-Path $dir 'innert.exe')) -and -not $PSScriptRoot.StartsWith($dir, [StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item $dir -Recurse -Force
    } elseif (Test-Path (Join-Path $dir 'innert.exe')) {
        # Run from inside the folder itself: removed once this script has finished
        Start-Process cmd.exe -ArgumentList '/c', "timeout /t 2 >nul & rmdir /s /q `"$dir`"" -WindowStyle Hidden
    }
    # The shaders made ready on the first start, made again if it's installed again. Saves and the
    # chosen quality are kept.
    $shaders = Join-Path $env:LOCALAPPDATA 'Innert\shaders'
    if (Test-Path $shaders) { Remove-Item $shaders -Recurse -Force }
    Pass "Removed. Your saved games are kept in $(Join-Path $env:LOCALAPPDATA 'Innert')."
}

function Install-Innert {
    if (-not (Test-Path (Join-Path $source 'innert.exe'))) { throw "innert.exe isn't beside this script in $source." }
    $from = [IO.Path]::GetFullPath($source).TrimEnd('\')
    $to = [IO.Path]::GetFullPath($InstallDir).TrimEnd('\')
    if ($from -eq $to) { throw 'This is already the installed copy.' }
    if ((Running-From $to).Count -gt 0) { throw 'Innert is running from where it would be installed: close it first.' }

    Say "Installing Innert into $to..." 'Cyan'
    New-Item -ItemType Directory -Force $to | Out-Null
    Get-ChildItem $from -File | Where-Object { $_.Name -ne 'Install Innert.cmd' } | Copy-Item -Destination $to -Force
    Pass 'Files copied'
    if ($Portable) { return }

    $shell = New-Object -ComObject WScript.Shell
    function Link([string]$path) {
        $link = $shell.CreateShortcut($path)
        $link.TargetPath = Join-Path $to 'innert.exe'
        $link.WorkingDirectory = $to
        $link.IconLocation = (Join-Path $to 'innert.exe') + ',0'
        $link.Description = 'Innert'
        $link.Save()
    }
    Link $startMenu
    Pass 'Start menu shortcut'
    if ($Desktop -or (-not $Quiet -and (Ask 'Add a desktop shortcut too?' $false))) {
        Link $desktopLink
        Pass 'Desktop shortcut'
    }

    # Listed in Windows' installed apps, removed from there by this same script
    $version = (Get-Item (Join-Path $to 'innert.exe')).VersionInfo.ProductVersion
    $size = [int]((Get-ChildItem $to -File | Measure-Object Length -Sum).Sum / 1KB)
    New-Item -Force $uninstallKey | Out-Null
    $script = Join-Path $to 'install.ps1'
    $values = @{
        DisplayName = 'Innert'; DisplayIcon = (Join-Path $to 'innert.exe'); InstallLocation = $to
        DisplayVersion = "$version"; Publisher = 'Innert'; EstimatedSize = $size; NoModify = 1; NoRepair = 1
        UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$script`" -Uninstall -Quiet -InstallDir `"$to`""
    }
    foreach ($k in $values.Keys) { New-ItemProperty $uninstallKey -Name $k -Value $values[$k] -Force | Out-Null }
    Pass 'Listed in installed apps, to remove it from there'
}

try {
    if ($Uninstall) { Remove-Innert; exit 0 }
    $ready = Test-Prerequisites
    if ($CheckOnly) {
        if ($ready) { Say 'This computer can run Innert.' 'Green' } else { Say 'Fix what failed above, then run this again.' 'Red' }
        exit ([int](-not $ready))
    }
    if (-not $ready -and -not (Ask 'Something above failed. Install anyway?' $false)) { exit 1 }
    Install-Innert
    Say 'Innert is installed. The very first start takes 15-20 seconds while the shaders are prepared.' 'Green'
    if (-not $Portable -and (Ask 'Start Innert now?')) { Start-Process (Join-Path $InstallDir 'innert.exe') -WorkingDirectory $InstallDir }
    exit 0
} catch {
    Fail $_.Exception.Message
    exit 1
}
