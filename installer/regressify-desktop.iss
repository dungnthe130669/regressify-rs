#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef SourceDir
  #error SourceDir is required
#endif
#ifndef InstallerOutput
  #define InstallerOutput "Output"
#endif

[Setup]
AppId={{ED6F0908-7E11-4608-A532-E43482AA117B}
AppName=Regressify Desktop
AppVersion={#AppVersion}
AppPublisher=Tuyen Pham
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Regressify Desktop
DefaultGroupName=Regressify Desktop
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableWelcomePage=yes
DisableReadyPage=yes
DisableFinishedPage=yes
AppMutex=Local\Regressify.Desktop.Running.ED6F0908-7E11-4608-A532-E43482AA117B
SetupMutex=Local\Regressify.Desktop.Setup.ED6F0908-7E11-4608-A532-E43482AA117B
CloseApplications=no
RestartApplications=no
ChangesEnvironment=yes
OutputDir={#InstallerOutput}
OutputBaseFilename=Regressify-Desktop-Setup-{#AppVersion}-x64
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
SetupLogging=yes
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\regressify-desktop.exe

[Files]
Source: "{#SourceDir}\regressify-desktop.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\regressify.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSING.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Regressify Desktop"; Filename: "{app}\regressify-desktop.exe"; IconFilename: "{app}\regressify-desktop.exe"; IconIndex: 0; AppUserModelID: "Regressify.Desktop"
Name: "{autodesktop}\Regressify Desktop"; Filename: "{app}\regressify-desktop.exe"; IconFilename: "{app}\regressify-desktop.exe"; IconIndex: 0; AppUserModelID: "Regressify.Desktop"

[Registry]
Root: HKCU; Subkey: "Software\Classes\Directory\shell\RegressifyDesktop"; ValueType: string; ValueName: ""; ValueData: "Open with Regressify Desktop"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Directory\shell\RegressifyDesktop"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\regressify-desktop.exe"",0"
Root: HKCU; Subkey: "Software\Classes\Directory\shell\RegressifyDesktop\command"; ValueType: string; ValueName: ""; ValueData: """{app}\regressify-desktop.exe"" ""%1\."""
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\RegressifyDesktop"; ValueType: string; ValueName: ""; ValueData: "Open with Regressify Desktop"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\RegressifyDesktop"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\regressify-desktop.exe"",0"
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\RegressifyDesktop\command"; ValueType: string; ValueName: ""; ValueData: """{app}\regressify-desktop.exe"" ""%V\."""

[Run]
Filename: "{app}\regressify-desktop.exe"; Parameters: "{code:LaunchParameters}"; Flags: nowait runasoriginaluser; Check: ShouldLaunch; BeforeInstall: PrepareChromium

[Code]
const
  UserEnvironmentKey = 'Environment';
  CliPathOwnerKey = 'Software\Regressify\DesktopInstaller';
  BroadcastWindow = $FFFF;
  EnvironmentChangeMessage = $001A;

var
  ChromiumPrepared: Boolean;
  ChromiumProgress: TOutputMarqueeProgressWizardPage;

function SetForegroundWindow(Window: HWND): BOOL;
  external 'SetForegroundWindow@user32.dll stdcall';

{ Setup is a 32-bit process, including when installing the x64 payloads. }
function QueryRegistryValue(RootKey: LongWord; const SubKey, ValueName: String;
  Flags: Cardinal; var ValueType: Cardinal; Data: LongWord; var DataSize: Cardinal): Longint;
  external 'RegGetValueW@advapi32.dll stdcall';

function ExpandPathEnvironment(const Source, Destination: String; Size: Cardinal): Cardinal;
  external 'ExpandEnvironmentStringsW@kernel32.dll stdcall';

function NotifyEnvironmentChange(Window: HWND; Message: Cardinal; WParam: LongWord;
  const Section: String; Flags, Timeout: Cardinal; var MessageResult: LongWord): Longint;
  external 'SendMessageTimeoutW@user32.dll stdcall';

function ReadUserPath(var PathValue: String; var ValueType: Cardinal): Boolean;
var
  Status: Longint;
  DataSize: Cardinal;
begin
  DataSize := 0;
  { Query the type without expanding tokens or accepting non-string registry data. }
  Status := QueryRegistryValue(HKCU, UserEnvironmentKey, 'Path', $10000006,
    ValueType, 0, DataSize);
  if Status = 2 then
  begin
    PathValue := '';
    ValueType := 0;
    Result := True;
  end
  else
    Result := (Status = 0) and RegQueryStringValue(HKCU, UserEnvironmentKey, 'Path', PathValue);
end;

function WriteUserPath(const PathValue: String; ValueType: Cardinal): Boolean;
begin
  if ValueType = 1 then
    Result := RegWriteStringValue(HKCU, UserEnvironmentKey, 'Path', PathValue)
  else
    Result := RegWriteExpandStringValue(HKCU, UserEnvironmentKey, 'Path', PathValue);
end;

function NormalizePathEntry(Value: String; ExpandVariables: Boolean): String;
var
  Expanded: String;
  BufferSize, RequiredSize: Cardinal;
begin
  Value := RemoveQuotes(Trim(Value));
  if ExpandVariables then
  begin
    BufferSize := ExpandPathEnvironment(Value, '', 0);
    if (BufferSize > 0) and (BufferSize <= 32767) then
    begin
      SetLength(Expanded, BufferSize);
      RequiredSize := ExpandPathEnvironment(Value, Expanded, BufferSize);
      if (RequiredSize > 0) and (RequiredSize <= BufferSize) then
      begin
        SetLength(Expanded, RequiredSize - 1);
        Value := Expanded;
      end;
    end;
  end;
  StringChangeEx(Value, '/', '\', True);
  Result := RemoveBackslashUnlessRoot(Value);
end;

function FindPathEntry(const PathValue, Directory: String; ExpandVariables: Boolean;
  var EntryStart, EntryLength: Integer): Boolean;
var
  Cursor: Integer;
  Quoted: Boolean;
begin
  Result := False;
  EntryStart := 1;
  Quoted := False;
  for Cursor := 1 to Length(PathValue) + 1 do
  begin
    if Cursor <= Length(PathValue) then
      if PathValue[Cursor] = '"' then
        Quoted := not Quoted;
    if (Cursor > Length(PathValue)) or ((PathValue[Cursor] = ';') and not Quoted) then
    begin
      EntryLength := Cursor - EntryStart;
      if CompareText(NormalizePathEntry(Copy(PathValue, EntryStart, EntryLength), ExpandVariables),
        NormalizePathEntry(Directory, False)) = 0 then
      begin
        Result := True;
        Exit;
      end;
      EntryStart := Cursor + 1;
    end;
  end;
end;

function RemovePathEntry(var PathValue: String; const Directory: String; ExpandVariables: Boolean): Boolean;
var
  EntryStart, EntryLength: Integer;
begin
  Result := FindPathEntry(PathValue, Directory, ExpandVariables, EntryStart, EntryLength);
  if not Result then
    Exit;
  { Remove one owned entry and its separator, leaving every other byte intact. }
  if EntryStart > 1 then
    Delete(PathValue, EntryStart - 1, EntryLength + 1)
  else if EntryLength < Length(PathValue) then
    Delete(PathValue, EntryStart, EntryLength + 1)
  else
    PathValue := '';
end;

procedure RemoveCliPath;
var
  Directory, PathValue: String;
  ValueType, PathCreated: Cardinal;
  Written: Boolean;
begin
  if not RegQueryStringValue(HKCU, CliPathOwnerKey, 'CliPath', Directory) or (Directory = '') then
    Exit;
  if not ReadUserPath(PathValue, ValueType) then
    RaiseException('Cannot read the user PATH; it has not been changed.');
  PathCreated := 0;
  RegQueryDWordValue(HKCU, CliPathOwnerKey, 'PathCreated', PathCreated);
  if RemovePathEntry(PathValue, Directory, ValueType = 2) then
  begin
    if (PathValue = '') and (PathCreated = 1) then
      Written := RegDeleteValue(HKCU, UserEnvironmentKey, 'Path')
    else
      Written := WriteUserPath(PathValue, ValueType);
    if not Written then
      RaiseException('Cannot remove the Regressify CLI entry from the user PATH.');
  end;
  RegDeleteValue(HKCU, CliPathOwnerKey, 'CliPath');
  RegDeleteValue(HKCU, CliPathOwnerKey, 'PathCreated');
  RegDeleteKeyIfEmpty(HKCU, CliPathOwnerKey);
end;

procedure RegisterCliPath(const Directory: String);
var
  OwnedDirectory, PathValue, Entry: String;
  ValueType, PathCreated: Cardinal;
  EntryStart, EntryLength: Integer;
begin
  if RegQueryStringValue(HKCU, CliPathOwnerKey, 'CliPath', OwnedDirectory) and
     (CompareText(OwnedDirectory, Directory) <> 0) then
    RemoveCliPath;
  if not ReadUserPath(PathValue, ValueType) then
    RaiseException('Cannot read the user PATH; it has not been changed.');
  if FindPathEntry(PathValue, Directory, ValueType = 2, EntryStart, EntryLength) then
    Exit;
  Entry := Directory;
  if Pos(';', Entry) > 0 then
    Entry := '"' + Entry + '"';
  if PathValue <> '' then
    PathValue := PathValue + ';';
  PathValue := PathValue + Entry;
  if Length(PathValue) > 32766 then
    RaiseException('The user PATH is too long to add the Regressify CLI.');
  PathCreated := 0;
  if ValueType = 0 then
    PathCreated := 1;
  { Keep ownership across upgrades; never claim an entry that was already present. }
  if not RegWriteDWordValue(HKCU, CliPathOwnerKey, 'PathCreated', PathCreated) then
    RaiseException('Cannot record ownership of the Regressify CLI PATH entry.');
  if not RegWriteStringValue(HKCU, CliPathOwnerKey, 'CliPath', Directory) then
  begin
    RegDeleteValue(HKCU, CliPathOwnerKey, 'PathCreated');
    RaiseException('Cannot record ownership of the Regressify CLI PATH entry.');
  end;
  if not WriteUserPath(PathValue, ValueType) then
  begin
    RegDeleteValue(HKCU, CliPathOwnerKey, 'CliPath');
    RegDeleteValue(HKCU, CliPathOwnerKey, 'PathCreated');
    RegDeleteKeyIfEmpty(HKCU, CliPathOwnerKey);
    RaiseException('Cannot add the Regressify CLI to the user PATH.');
  end;
end;

procedure ChromiumInstallLog(const S: String; const Error, FirstLine: Boolean);
begin
  Log('Chromium: ' + S);
  if not Error and (ChromiumProgress <> nil) and
     ((Pos('Checking ', S) = 1) or (Pos('Waiting ', S) = 1) or
      (Pos('Connecting ', S) = 1) or (Pos('Downloading ', S) = 1) or
      (Pos('Extracting ', S) = 1) or (Pos('Verifying ', S) = 1) or
      (Pos('Reusing cached ', S) = 1) or (Pos('Chromium ready', S) = 1)) then
  begin
    ChromiumProgress.SetText(S, '');
    Log('Chromium progress: ' + S);
  end;
end;

procedure PrepareChromium;
var
  Started: Boolean;
  ResultCode: Integer;
begin
  if ChromiumPrepared then
    Exit;
  ChromiumPrepared := True;
  if ExpandConstant('{param:SKIPCHROMIUM|0}') = '1' then
  begin
    Log('Chromium preparation skipped; Reference/Test will install it when needed.');
    Exit;
  end;

  Log('Preparing shared Chromium cache before launching Regressify Desktop.');
  if WizardForm.Visible then
  begin
    ChromiumProgress := CreateOutputMarqueeProgressPage('Preparing Chromium', '');
    ChromiumProgress.SetText('Checking Chromium cache', '');
    ChromiumProgress.Show;
    ChromiumProgress.Animate;
  end;
  Started := False;
  ResultCode := -1;
  try
    try
      Started := ExecAndLogOutput(ExpandConstant('{app}\regressify-desktop.exe'),
        '--install-chromium', ExpandConstant('{app}'), SW_HIDE,
        ewWaitUntilTerminated, ResultCode, @ChromiumInstallLog);
      if not Started then
        Log('Cannot start Chromium preparation: ' + SysErrorMessage(ResultCode));
    except
      Log('Chromium preparation error: ' + GetExceptionMessage);
    end;
  finally
    if ChromiumProgress <> nil then
      ChromiumProgress.Hide;
  end;

  if Started and (ResultCode = 0) then
    Log('Chromium preparation completed successfully.')
  else
  begin
    Log('Chromium preparation failed (exit ' + IntToStr(ResultCode) + '); app installation remains successful and Reference/Test can retry.');
    if not WizardSilent then
      SuppressibleMsgBox('Regressify Desktop was installed, but Chromium could not be prepared.' + #13#10#13#10 +
        'Reference/Test will retry when a browser is needed. Check your connection, or configure Proxy in Run settings and choose Install Chromium.' + #13#10#13#10 +
        'Setup log: ' + ExpandConstant('{log}'), mbInformation, MB_OK, IDOK);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssInstall) and WizardForm.Visible then
  begin
    WizardForm.BringToFront;
    if SetForegroundWindow(WizardForm.Handle) then
      Log('Installer brought to foreground after running-app checks.')
    else
      Log('Windows denied installer foreground activation.');
  end;
  if CurStep = ssPostInstall then
  begin
    RegisterCliPath(ExpandConstant('{app}'));
    PrepareChromium;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  MessageResult: LongWord;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveCliPath;
    NotifyEnvironmentChange(BroadcastWindow, EnvironmentChangeMessage, 0, 'Environment', 2, 5000, MessageResult);
  end;
end;

function ShouldLaunch(): Boolean;
begin
  Result := ExpandConstant('{param:NORELAUNCH|0}') <> '1';
end;

function LaunchParameters(Param: String): String;
var
  Workspace: String;
begin
  Result := '--after-install';
  Workspace := ExpandConstant('{param:WORKSPACE|}');
  if Workspace <> '' then
  begin
    StringChangeEx(Workspace, '"', '', True);
    if Workspace[Length(Workspace)] = '\' then
      Workspace := Workspace + '\';
    Result := Result + ' "' + Workspace + '"';
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  InstalledMS, InstalledLS, SetupMS, SetupLS: Cardinal;
begin
  Result := '';
  if CheckForMutexes('Local\Regressify.Desktop.Running.ED6F0908-7E11-4608-A532-E43482AA117B') then
  begin
    Result := 'Close all Regressify Desktop windows and wait for active runs to finish, then retry setup.';
    Exit;
  end;
  if GetVersionNumbers(ExpandConstant('{app}\regressify-desktop.exe'), InstalledMS, InstalledLS) and
     GetVersionNumbers(ExpandConstant('{srcexe}'), SetupMS, SetupLS) then
  begin
    if (InstalledMS > SetupMS) or ((InstalledMS = SetupMS) and (InstalledLS > SetupLS)) then
      Result := 'A newer version of Regressify Desktop is already installed. Downgrades are not supported.';
  end;
end;