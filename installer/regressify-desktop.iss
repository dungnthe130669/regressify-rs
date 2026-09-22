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
var
  ChromiumPrepared: Boolean;
  ChromiumProgress: TOutputMarqueeProgressWizardPage;

function SetForegroundWindow(Window: HWND): BOOL;
  external 'SetForegroundWindow@user32.dll stdcall';

procedure ChromiumInstallLog(const S: String; const Error, FirstLine: Boolean);
begin
  Log('Chromium: ' + S);
  if not Error and (ChromiumProgress <> nil) and
     ((Pos('Checking ', S) = 1) or (Pos('Waiting ', S) = 1) or
      (Pos('Connecting ', S) = 1) or (Pos('Downloading ', S) = 1) or
      (Pos('Extracting ', S) = 1) or (Pos('Verifying ', S) = 1) or
      (Pos('Reusing cached ', S) = 1) or (Pos('Chromium ready', S) = 1)) then
    ChromiumProgress.SetText(S, '');
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
  if not WizardSilent then
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
    PrepareChromium;
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