# Cortex shortcut installer (Windows). Creates two .lnk files on the Desktop
# and in the Start Menu, both with the cmd-wrapper target pattern so they
# can be pinned to the taskbar (raw .bat targets aren't pinnable; cmd.exe
# is, and `cmd /c <script>` survives the pin).
#
#   Cortex.lnk       — daily-driver prod, custom Cortex.ico
#   Cortex Dev.lnk   — live-rebuild dev loop, Cortex-Dev.ico (with "DEV"
#                       overlay text in pink #F000D0 with dark purple
#                       #200040 outline, sampled from the master icon)
#
# Idempotent — re-running just refreshes the targets and icons. Run once
# after install-cortex-prod.cmd completes (so prod EXE exists), or run
# anytime to re-point shortcuts after moving things.
#
# If the icons haven't been generated yet, this script invokes
# build-shortcut-icons.py first.

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$InstallDir = Join-Path $env:LOCALAPPDATA 'Cortex'
$ProdIco = Join-Path $InstallDir 'Cortex.ico'
$DevIco = Join-Path $InstallDir 'Cortex-Dev.ico'
$ProdScript = Join-Path $RepoRoot 'scripts\launch-cortex.cmd'
$DevScript = Join-Path $RepoRoot 'scripts\launch-cortex-dev.bat'

# --- Generate icons if missing -------------------------------------------
if (-not (Test-Path $ProdIco) -or -not (Test-Path $DevIco)) {
    Write-Host "Icons missing - running build-shortcut-icons.py..."
    $py = Join-Path $RepoRoot 'scripts\build-shortcut-icons.py'
    & python $py
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Icon generation failed."
        exit 1
    }
}

# --- Validate prerequisites ----------------------------------------------
if (-not (Test-Path $ProdScript)) {
    Write-Error "Missing $ProdScript - did you intend to run install-cortex-prod.cmd first?"
    exit 1
}
if (-not (Test-Path $DevScript)) {
    Write-Error "Missing $DevScript"
    exit 1
}

# --- Helper: stamp System.AppUserModel.ID onto a .lnk via IPropertyStore.
#     WScript.Shell.CreateShortcut (used below) cannot write this property,
#     so after creating the .lnk we re-open it via IShellLinkW + IPersistFile,
#     write the PKEY_AppUserModel_ID property, Commit, and Save.
#
#     Why this matters: the running EXE sets its AUMID at startup
#     (app/src/lib.rs Windows block). For Windows to correctly merge a pinned
#     taskbar shortcut with the running window's taskbar group, the .lnk must
#     advertise the same AUMID. Without that, clicking the pinned dev icon
#     while dev is running launches a second instance instead of focusing the
#     existing window. Inno Setup achieves the same thing on the prod
#     installer path via its `AppUserModelID:` directive (see
#     script/windows/windows-installer.iss:141-142); we replicate that
#     behavior here for the dev shortcut.
$AumidSetterSrc = @'
using System;
using System.Runtime.InteropServices;

[StructLayout(LayoutKind.Sequential, Pack = 4)]
public struct CortexPropertyKey {
    public Guid fmtid;
    public uint pid;
}

[StructLayout(LayoutKind.Sequential)]
public struct CortexPropVariant {
    public ushort vt;
    public ushort r1;
    public ushort r2;
    public ushort r3;
    public IntPtr ptr;
    public IntPtr ptr2;
}

[ComImport]
[Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface ICortexPropertyStore {
    uint GetCount(out uint cProps);
    uint GetAt(uint iProp, out CortexPropertyKey pkey);
    uint GetValue(ref CortexPropertyKey key, out CortexPropVariant pv);
    [PreserveSig] uint SetValue(ref CortexPropertyKey key, ref CortexPropVariant pv);
    [PreserveSig] uint Commit();
}

[ComImport]
[Guid("0000010B-0000-0000-C000-000000000046")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface ICortexPersistFile {
    uint GetClassID(out Guid pClassID);
    uint IsDirty();
    [PreserveSig] uint Load([MarshalAs(UnmanagedType.LPWStr)] string pszFileName, uint dwMode);
    [PreserveSig] uint Save([MarshalAs(UnmanagedType.LPWStr)] string pszFileName, [MarshalAs(UnmanagedType.Bool)] bool fRemember);
    uint SaveCompleted([MarshalAs(UnmanagedType.LPWStr)] string pszFileName);
    uint GetCurFile([MarshalAs(UnmanagedType.LPWStr)] out string ppszFileName);
}

public static class CortexShortcutAumid {
    static readonly Guid CLSID_ShellLink = new Guid("00021401-0000-0000-C000-000000000046");
    static readonly Guid FMTID_AppUserModel = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3");

    const ushort VT_LPWSTR = 31;

    [DllImport("ole32.dll")]
    static extern int PropVariantClear(ref CortexPropVariant pvar);

    public static void Set(string lnkPath, string aumid) {
        var shellLinkType = Type.GetTypeFromCLSID(CLSID_ShellLink);
        object shellLink = Activator.CreateInstance(shellLinkType);
        try {
            var persist = (ICortexPersistFile)shellLink;
            uint hr = persist.Load(lnkPath, 2 /* STGM_READWRITE */);
            if (hr != 0) throw new System.ComponentModel.Win32Exception((int)hr, "IPersistFile.Load failed for " + lnkPath);

            var props = (ICortexPropertyStore)shellLink;
            var key = new CortexPropertyKey { fmtid = FMTID_AppUserModel, pid = 5 };

            // Construct PROPVARIANT manually with VT_LPWSTR. The string lives on
            // the COM heap; PropVariantClear frees it for us via CoTaskMemFree.
            // (InitPropVariantFromString is an inline helper in propvarutil.h on
            // modern SDKs, not a propsys.dll export, so we can't P/Invoke it.)
            var pv = new CortexPropVariant {
                vt = VT_LPWSTR,
                ptr = Marshal.StringToCoTaskMemUni(aumid)
            };
            try {
                hr = props.SetValue(ref key, ref pv);
                if (hr != 0) throw new System.ComponentModel.Win32Exception((int)hr, "IPropertyStore.SetValue failed");
                hr = props.Commit();
                if (hr != 0) throw new System.ComponentModel.Win32Exception((int)hr, "IPropertyStore.Commit failed");
            } finally {
                PropVariantClear(ref pv);
            }
            hr = persist.Save(lnkPath, true);
            if (hr != 0) throw new System.ComponentModel.Win32Exception((int)hr, "IPersistFile.Save failed for " + lnkPath);
        } finally {
            Marshal.ReleaseComObject(shellLink);
        }
    }
}
'@
if (-not ('CortexShortcutAumid' -as [type])) {
    Add-Type -TypeDefinition $AumidSetterSrc -Language CSharp
}

# AUMIDs must match what the running EXE sets in app/src/lib.rs's Windows block.
# Prod (release build) uses ChannelState::app_id() verbatim. Dev (debug build)
# appends ".Dev" to the same string. Keep these in lockstep.
$ProdAumid = 'dev.warp.WarpOss'
$DevAumid = 'dev.warp.WarpOss.Dev'

# --- Helper: create a .lnk via WScript.Shell COM -------------------------
function New-Shortcut {
    param(
        [string]$Path,
        [string]$Target,
        [string]$Arguments,
        [string]$IconLocation,
        [string]$Description,
        [string]$WorkingDirectory
    )
    $ws = New-Object -ComObject WScript.Shell
    $sc = $ws.CreateShortcut($Path)
    $sc.TargetPath = $Target
    $sc.Arguments = $Arguments
    $sc.IconLocation = $IconLocation
    $sc.Description = $Description
    $sc.WorkingDirectory = $WorkingDirectory
    # Window style 7 = "minimized" so the cmd wrapper window doesn't grab focus
    # for prod (which detaches the EXE immediately via `start ""` and exits).
    # Dev keeps the default 1 (normal) so the build output is visible.
    $sc.WindowStyle = 1
    $sc.Save()
}

# --- Where to drop shortcuts ---------------------------------------------
$Desktop = [Environment]::GetFolderPath('Desktop')
$StartMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'

$cmdExe = "$env:WINDIR\System32\cmd.exe"

$prodArgs = "/c `"$ProdScript`""
$devArgs = "/c `"$DevScript`""

$prodDesc = 'Cortex Terminal (daily driver)'
$devDesc = 'Cortex Terminal - dev (live rebuild + launch)'

foreach ($Dir in @($Desktop, $StartMenu)) {
    Write-Host "Writing shortcuts to: $Dir"

    $prodLnk = Join-Path $Dir 'Cortex.lnk'
    $devLnk = Join-Path $Dir 'Cortex Dev.lnk'

    New-Shortcut `
        -Path $prodLnk `
        -Target $cmdExe `
        -Arguments $prodArgs `
        -IconLocation "$ProdIco,0" `
        -Description $prodDesc `
        -WorkingDirectory $env:USERPROFILE

    New-Shortcut `
        -Path $devLnk `
        -Target $cmdExe `
        -Arguments $devArgs `
        -IconLocation "$DevIco,0" `
        -Description $devDesc `
        -WorkingDirectory $RepoRoot

    [CortexShortcutAumid]::Set($prodLnk, $ProdAumid)
    [CortexShortcutAumid]::Set($devLnk, $DevAumid)
}

Write-Host ""
Write-Host "Done. To pin to taskbar:"
Write-Host "  1. Right-click 'Cortex' on Desktop -> 'Show more options' -> 'Pin to taskbar'"
Write-Host "  2. Same for 'Cortex Dev'"
Write-Host ""
Write-Host "Targets:"
Write-Host "  Cortex.lnk      -> cmd /c $ProdScript     (icon: Cortex.ico,     AUMID: $ProdAumid)"
Write-Host "  Cortex Dev.lnk  -> cmd /c $DevScript      (icon: Cortex-Dev.ico, AUMID: $DevAumid)"
