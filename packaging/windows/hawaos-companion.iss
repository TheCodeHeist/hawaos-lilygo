; HawaOS Companion installer for Windows (Inno Setup 6). Built by
; .github/workflows/windows.yml:
;
;   iscc /DAppVersion=0.9.0 /DExeDir=..\..\companion\target\release packaging\windows\hawaos-companion.iss
;
; Installs for the current user only (no administrator rights), under
; %LOCALAPPDATA%\Programs. Settings live in %APPDATA%\hawaos-companion and are
; kept on uninstall, like on Linux.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef ExeDir
  #define ExeDir "..\..\companion\target\release"
#endif

[Setup]
; Never change the AppId: Windows uses it to recognise updates.
AppId={{56F6AE17-FD41-4877-9265-22F68EC5CBEA}
AppName=HawaOS Companion
AppVersion={#AppVersion}
AppPublisher=TheCodeHeist
AppPublisherURL=https://github.com/TheCodeHeist/hawaos-lilygo
AppSupportURL=https://github.com/TheCodeHeist/hawaos-lilygo/issues
DefaultDirName={localappdata}\Programs\HawaOS Companion
DefaultGroupName=HawaOS Companion
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir=..\..\dist
OutputBaseFilename=hawaos-companion-{#AppVersion}-windows-x64-setup
SetupIconFile=hawaos.ico
UninstallDisplayIcon={app}\hawaos-companion.exe
UninstallDisplayName=HawaOS Companion
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
; Closes a running companion before replacing it.
CloseApplications=yes

[Tasks]
Name: "autostart"; Description: "Start HawaOS Companion when I sign in"; GroupDescription: "Startup:"
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#ExeDir}\hawaos-companion.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\HawaOS Companion"; Filename: "{app}\hawaos-companion.exe"
Name: "{autodesktop}\HawaOS Companion"; Filename: "{app}\hawaos-companion.exe"; Tasks: desktopicon

[Registry]
; The same value the companion's "Start with system" switch writes
; (companion/src/platform/windows/mod.rs), so either can turn it off.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "HawaOS Companion"; ValueData: """{app}\hawaos-companion.exe"" --minimized"; Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\hawaos-companion.exe"; Description: "Open HawaOS Companion"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/IM hawaos-companion.exe /F"; Flags: runhidden; RunOnceId: "StopCompanion"

[Code]
// The autostart value, also when the companion's own switch wrote it.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'HawaOS Companion');
end;
