; Makes dist\InnertSetup.exe from what package.ps1 puts in dist\Innert, with Inno Setup 6:
;   powershell -ExecutionPolicy Bypass -File package.ps1
; It installs for whoever runs it, with no administrator needed, adds a Start menu shortcut and if
; wanted one on the desktop, and an uninstaller. Vulkan comes with the graphics card's driver: if
; it isn't there, it says so and offers to open the page for the right one.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

[Setup]
; The same for every version, so a newer one installs over an older one
AppId={{ED1B1DA3-0006-4365-8C74-DF07BB51ACC6}
AppName=Innert
AppVersion={#AppVersion}
AppVerName=Innert {#AppVersion}
DefaultDirName={autopf}\Innert
DefaultGroupName=Innert
DisableProgramGroupPage=yes
DisableWelcomePage=no
; Into the user's own programs folder, so no administrator is asked for, unless chosen for everyone
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
UsedUserAreasWarning=no
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=dist
OutputBaseFilename=InnertSetup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\innert.exe
UninstallDisplayName=Innert
; The controls, before the last page
InfoAfterFile=dist\Innert\README.txt

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "dist\Innert\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Innert"; Filename: "{app}\innert.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Innert"; Filename: "{app}\innert.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\innert.exe"; WorkingDir: "{app}"; Description: "{cm:LaunchProgram,Innert}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; The shaders made ready on the first start, made again if it's ever installed again. The quality
; chosen, quality.txt beside them, is kept.
Type: filesandordirs; Name: "{localappdata}\Innert\shaders"

[Code]
const
  NvidiaPage = 'https://www.nvidia.com/Download/index.aspx';
  AmdPage = 'https://www.amd.com/en/support/download/drivers.html';
  IntelPage = 'https://www.intel.com/content/www/us/en/support/detect.html';

// The graphics cards Windows knows of, one to a line, or nothing if it can't be asked
function GraphicsCards(): String;
var
  Locator, Service, Cards, Card: Variant;
  I: Integer;
begin
  Result := '';
  try
    Locator := CreateOleObject('WbemScripting.SWbemLocator');
    Service := Locator.ConnectServer('.', 'root\CIMV2');
    Cards := Service.ExecQuery('SELECT Name FROM Win32_VideoController');
    for I := 0 to Cards.Count - 1 do
    begin
      Card := Cards.ItemIndex(I);
      Result := Result + Card.Name + #13#10;
    end;
  except
    Result := '';
  end;
end;

// Where to get the driver for the card the game would draw with: an NVIDIA or AMD one rather than
// the graphics built into an Intel processor beside it, as on many laptops
function DriverPage(Cards: String): String;
var
  Upper: String;
begin
  Upper := Uppercase(Cards);
  if Pos('NVIDIA', Upper) > 0 then
    Result := NvidiaPage
  else if (Pos('AMD', Upper) > 0) or (Pos('RADEON', Upper) > 0) then
    Result := AmdPage
  else if Pos('INTEL', Upper) > 0 then
    Result := IntelPage
  else
    Result := '';
end;

// Vulkan's there once a graphics driver that has it is, which puts vulkan-1.dll in Windows' folders
function VulkanFound(): Boolean;
begin
  Result := FileExists(ExpandConstant('{sys}\vulkan-1.dll')) or FileExists(ExpandConstant('{syswow64}\vulkan-1.dll'));
end;

function InitializeSetup(): Boolean;
var
  Cards, Page, Text: String;
  Code: Integer;
begin
  Result := True;
  if VulkanFound() then
    Exit;
  Cards := Trim(GraphicsCards());
  Page := DriverPage(Cards);
  Text := 'Innert draws with Vulkan, which comes with the driver for your graphics card, and the driver on this computer doesn''t have it yet.' + #13#10#13#10;
  if Cards <> '' then
    Text := Text + 'Your graphics: ' + Cards + #13#10#13#10;
  if Page <> '' then
  begin
    Text := Text + 'Open the page to download the latest driver now? Innert can be installed either way, and starts once the new driver''s in.';
    if SuppressibleMsgBox(Text, mbInformation, MB_YESNO, IDNO) = IDYES then
      ShellExec('open', Page, '', '', SW_SHOWNORMAL, ewNoWait, Code);
  end
  else
    SuppressibleMsgBox(Text + 'Installing the latest driver from NVIDIA, AMD or Intel adds it. Innert can be installed either way.', mbInformation, MB_OK, IDOK);
end;
