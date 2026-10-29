; Corporate Warfare Windows setup (Inno Setup 6). Built from CI by tools/build-windows-setup.ps1:
;   iscc /DDisplayVersion=1.9.0 /DNumericVersion=1.9.0 /DAppImage=<jpackage app image dir>
;        /DArt=<dir from tools/installer_art.py> /DOutDir=<dir> setup.iss
; One AppId for every version. Installing any version over any other, older or newer, replaces the
; program files in place. The saves live in %APPDATA%\Corporate Warfare, which this script never
; writes to or deletes except when the player explicitly chooses to delete them while uninstalling.
; The 1.8 MSI (upgrade code in WindowsInstall.UPGRADE_CODE) is removed first, keeping saves.

#ifndef DisplayVersion
  #error Pass /DDisplayVersion=<version shown to players>
#endif
#ifndef NumericVersion
  #define NumericVersion "0.0.0"
#endif
#ifndef AppImage
  #error Pass /DAppImage=<folder holding Corporate Warfare.exe>
#endif
#ifndef Art
  #error Pass /DArt=<folder from tools/installer_art.py>
#endif
#ifndef OutDir
  #define OutDir "."
#endif

#define AppName "Corporate Warfare"
#define ExeName "Corporate Warfare.exe"
#define Mutex "CorporateWarfareRunning"
#define MsiUpgradeCode "{5B6F6B2E-3C1D-4B8E-9F1A-0C7D2E4A9B31}"

[Setup]
AppId={{7C1E9A52-4D3B-4F6A-8E21-3A9D5B0C6F47}
AppName={#AppName}
AppVersion={#DisplayVersion}
AppVerName={#AppName} {#DisplayVersion}
AppPublisher=Lakeside Games
AppMutex={#Mutex}
VersionInfoVersion={#NumericVersion}.0
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} setup
DefaultDirName={autopf}\{#AppName}
UsePreviousAppDir=yes
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableReadyPage=no
; Per user by default (no administrator prompt); the player can choose all users on the first screen.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=license.txt
SetupIconFile=..\..\icons\corporate-warfare.ico
UninstallDisplayIcon={app}\{#ExeName}
WizardStyle=modern
WizardImageFile={#Art}\wizard-100.bmp,{#Art}\wizard-125.bmp,{#Art}\wizard-150.bmp,{#Art}\wizard-200.bmp
WizardSmallImageFile={#Art}\small-100.bmp,{#Art}\small-125.bmp,{#Art}\small-150.bmp,{#Art}\small-200.bmp
CloseApplications=yes
CloseApplicationsFilter=*.exe,*.dll
RestartApplications=no
Compression=lzma2/max
SolidCompression=yes
OutputDir={#OutDir}
OutputBaseFilename=CorporateWarfare-{#DisplayVersion}-setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Messages]
WelcomeLabel1=Welcome to Corporate Warfare
SetupAppRunningError=Corporate Warfare is open.%n%nClose the game, then click OK to continue. Your saves are safe and are not touched by setup.
UninstallAppRunningError=Corporate Warfare is open.%n%nClose the game, then click OK to continue the uninstall.
FinishedHeadingLabel=Corporate Warfare is ready
ClickFinish=Click Finish to close setup.

[Tasks]
Name: "startmenu"; Description: "Add Corporate Warfare to the &Start menu"
Name: "desktopicon"; Description: "Add a shortcut to the &desktop"

[InstallDelete]
; Program files only. A different version can ship different jar and runtime files; clear them so
; an older or newer one never mixes with leftovers. The saves are in %APPDATA%, never in {app}.
Type: filesandordirs; Name: "{app}\app"
Type: filesandordirs; Name: "{app}\runtime"

[Files]
Source: "{#AppImage}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#ExeName}"; Tasks: startmenu
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#ExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#ExeName}"; Description: "Play Corporate Warfare now"; Flags: nowait postinstall skipifsilent
; The in-game updater passes /RELAUNCH so the game opens again after a silent version change.
Filename: "{app}\{#ExeName}"; Flags: nowait skipifnotsilent; Check: RelaunchRequested

[UninstallDelete]
Type: filesandordirs; Name: "{app}\app"
Type: filesandordirs; Name: "{app}\runtime"

[Code]
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{7C1E9A52-4D3B-4F6A-8E21-3A9D5B0C6F47}_is1';

var
  ReplacedVersion: String;

function RelaunchRequested: Boolean;
begin
  Result := CmdLineParamExists('/RELAUNCH');
end;

function SavesFolder: String;
begin
  Result := ExpandConstant('{userappdata}\{#AppName}');
end;

function InstalledVersion: String;
begin
  Result := '';
  if not RegQueryStringValue(HKCU, UninstallKey, 'DisplayVersion', Result) then
    if not RegQueryStringValue(HKLM, UninstallKey, 'DisplayVersion', Result) then
      Result := '';
end;

procedure InitializeWizard;
begin
  ReplacedVersion := InstalledVersion;
  if ReplacedVersion = '' then
    WizardForm.WelcomeLabel2.Caption :=
      'This installs Corporate Warfare on your computer.' + #13#10#13#10 +
      'Your saved careers and settings are kept in your own application data folder, not in the ' +
      'game folder, so installing or removing the game never touches them.' + #13#10#13#10 +
      'Click Next to continue.'
  else if ReplacedVersion = '{#DisplayVersion}' then
    WizardForm.WelcomeLabel2.Caption :=
      'Corporate Warfare ' + ReplacedVersion + ' is already installed. This puts the game files back ' +
      'as they should be. Your saves are not touched.' + #13#10#13#10 + 'Click Next to continue.'
  else
    WizardForm.WelcomeLabel2.Caption :=
      'Corporate Warfare ' + ReplacedVersion + ' is installed. This changes it to {#DisplayVersion}, ' +
      'whether that is newer or older.' + #13#10#13#10 +
      'Your saved careers and settings are kept. Newer versions may save in a way older versions ' +
      'cannot read, so the game also keeps its own backup when you go back.' + #13#10#13#10 +
      'Click Next to continue.';
end;

{ The 1.8 MSI installs by the upgrade code below. Remove it before installing, keeping saves (an MSI
  never touches the saves folder). Per-machine MSIs need administrator rights, so each removal runs
  elevated and Windows asks once. A missing or already removed product is not an error. }
procedure RemoveMsiInstall;
var
  Script: String;
  Code: Integer;
begin
  Script :=
    '$i = New-Object -ComObject WindowsInstaller.Installer; ' +
    'foreach ($p in $i.RelatedProducts(''{#MsiUpgradeCode}'')) { ' +
    'Start-Process msiexec.exe -Verb RunAs -ArgumentList ''/x'', $p, ''/qn'', ''/norestart'' -Wait }';
  WizardForm.StatusLabel.Caption := 'Removing the earlier installer version of Corporate Warfare...';
  Exec('powershell.exe', '-NoProfile -ExecutionPolicy Bypass -Command "' + Script + '"', '',
    SW_HIDE, ewWaitUntilTerminated, Code);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
    RemoveMsiInstall;
end;

{ ---- Uninstall: ask whether to keep the saves; /VERYSILENT and /SILENT always keep them. ---- }

var
  DeleteSaves: Boolean;

function AskKeepSaves(StopLabel: String): Integer;
begin
  Result := TaskDialogMsgBox(
    'Keep your saved careers and settings?',
    'Your saves are in:' + #13#10 + SavesFolder + #13#10#13#10 +
    'Keeping them lets you pick up where you left off if you install Corporate Warfare again.',
    mbConfirmation, MB_YESNOCANCEL,
    ['Keep my saves and settings (recommended)', 'Delete my saves and settings too', StopLabel], 0);
end;

function InitializeUninstall: Boolean;
var
  Answer, Code: Integer;
begin
  Result := True;
  DeleteSaves := False;
  if UninstallSilent then Exit;
  Answer := AskKeepSaves('Open my saves folder first');
  if Answer = IDCANCEL then
  begin
    if DirExists(SavesFolder) then
      ShellExec('open', SavesFolder, '', '', SW_SHOWNORMAL, ewNoWait, Code);
    Answer := AskKeepSaves('Cancel the uninstall');
  end;
  if Answer = IDCANCEL then
    Result := False
  else if Answer = IDNO then
  begin
    if MsgBox('This permanently deletes every saved career and your settings. They cannot be brought back.' +
        #13#10#13#10 + 'Delete them?', mbError, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      DeleteSaves := True;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and DeleteSaves and (not UninstallSilent) then
    DelTree(SavesFolder, True, True, True);
end;
