; ops MCP Server installer
; Built with Inno Setup 6.x  (https://jrsoftware.org/isinfo.php).
;
; Parameterised so the same .iss compiles into either an x64 or an arm64
; installer. The release workflow defines OpsBinary, Arch, AppVersion, and
; OutputDir on the ISCC command line, e.g.:
;
;   iscc installer\ops.iss /DOpsBinary=...\ops-x64.exe ^
;                          /DArch=x64 /DAppVersion=0.3.0 /DOutputDir=.\dist
;
; The single binary `ops.exe` understands `register --binary <path>` and
; `unregister` subcommands, so the wizard does not ship a second helper.
; The `register` subcommand also copies the install path to the clipboard,
; so the user has it ready to paste into any MCP client - even one that
; isn't Claude Desktop.

#ifndef OpsBinary
  #define OpsBinary "..\target\release\ops.exe"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef AppVersion
  #define AppVersion "0.3.0"
#endif
#ifndef OutputDir
  #define OutputDir ".\out"
#endif

[Setup]
; Stable AppId so future installs upgrade in place rather than stacking
; duplicate entries in Add/Remove Programs.
AppId={{B8A6F3C9-5D2E-4A7B-9F3D-1E5C8B7A2D4F}
AppName=ops MCP Server
AppVersion={#AppVersion}
AppVerName=ops MCP Server {#AppVersion}
AppPublisher=Joseph Wander
AppPublisherURL=https://github.com/AIWander/ops
AppSupportURL=https://github.com/AIWander/ops/issues
AppUpdatesURL=https://github.com/AIWander/ops/releases

; Per-user install. No admin prompt; lives in %LOCALAPPDATA%\Ops.
DefaultDirName={localappdata}\Ops
DefaultGroupName=ops MCP Server
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

OutputDir={#OutputDir}
OutputBaseFilename=install-ops-{#Arch}
Compression=lzma2/ultra
SolidCompression=yes
WizardStyle=modern
LicenseFile=..\LICENSE

; Architecture gating. arm64 installer only runs on arm64; x64 installer runs
; on x64 native and on arm64 via emulation.
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif

UninstallDisplayName=ops MCP Server
UninstallDisplayIcon={app}\ops.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#OpsBinary}"; DestDir: "{app}"; DestName: "ops.exe"; Flags: ignoreversion

[Run]
; Post-install: register the ops server. This also copies the path to
; clipboard. runhidden so the user only sees the wizard, not a console flash.
Filename: "{app}\ops.exe"; \
  Parameters: "register --binary ""{app}\ops.exe"""; \
  StatusMsg: "Registering ops with Claude Desktop..."; \
  Flags: runhidden waituntilterminated

[UninstallRun]
; Pre-uninstall: remove the ops entry from claude_desktop_config.json
; before the binary is deleted.
Filename: "{app}\ops.exe"; \
  Parameters: "unregister"; \
  Flags: runhidden waituntilterminated

[Messages]
FinishedHeadingLabel=Setup has installed ops MCP Server on this computer.
FinishedLabel=The path to ops.exe has been copied to your clipboard.%n%nIf Claude Desktop was installed, the `ops` server has also been registered. Restart Claude Desktop to pick it up.%n%nFor any other MCP client (Claude Code, Codex CLI, Gemini CLI, ...), paste the clipboard path into that client's config.
