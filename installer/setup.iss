; CapIMESwitch 安装脚本
; 编译:ISCC.exe installer\setup.iss

#define MyAppName "CapIMESwitch"
#define MyAppVersion "0.4.3"
#define MyAppPublisher "CapIMESwitch"
#define MyAppExeName "cap-ime-switch.exe"

[Setup]
AppId={{B6F3C2A1-9D4E-4F7B-8A2C-5E1D0F3A9C41}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={localappdata}\Programs\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
; 默认 auto 在检测到已安装(同 AppId)时隐藏目录选择页,改为始终显示
DisableDirPage=no
PrivilegesRequired=lowest
OutputDir=dist
OutputBaseFilename=CapIMESwitch-Setup-{#MyAppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile=app.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName}
AppMutex=CapIMESwitch_SingleInstance

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Default.isl,ChineseSimplified.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

; 选项面板的配置文件(exe 同目录),程序运行期创建,卸载时一并删除
[UninstallDelete]
Type: files; Name: "{app}\config.toml"

[Code]
// 卸载时清除程序在 HKCU Run 键中设置的开机自启动项
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'CapIMESwitch');
end;