param(
    [ValidateSet('light', 'dark')][string]$Appearance = 'light',
    [ValidateSet('resident', 'summon')][string]$Presence = 'resident',
    [string]$OutputDirectory = 'target/input-window-acceptance/native',
    [switch]$RestartOnly
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$out = [IO.Path]::GetFullPath((Join-Path $repo $OutputDirectory))
New-Item -ItemType Directory -Force -Path $out | Out-Null
if (Get-Process nanatype -ErrorAction SilentlyContinue) { throw 'Close the existing NanaType instance before native acceptance.' }
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
public static class InputWindowProbe {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int L,T,R,B; }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
    public delegate bool EnumProc(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr p);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint id);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder s,int n);
    [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h,int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h,out Rect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h,ref Point p);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int n);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int z,uint flags);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(Point p);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int n);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point p);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint x,uint y,uint d,UIntPtr e);
    [DllImport("user32.dll")] public static extern void keybd_event(byte k,byte s,uint f,UIntPtr e);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    public static IntPtr[] Windows(int pid) {
        var list=new List<IntPtr>();
        EnumWindows((h,p)=> { uint id; GetWindowThreadProcessId(h,out id); if(id==pid && Title(h).Length>0) list.Add(h); return true; },IntPtr.Zero);
        return list.ToArray();
    }
    public static string Title(IntPtr h) { var s=new StringBuilder(256); GetWindowText(h,s,256); return s.ToString(); }
    public static Rect Bounds(IntPtr h) { Rect r; GetWindowRect(h,out r); return r; }
    public static Rect Client(IntPtr h) { Rect r; GetClientRect(h,out r); return r; }
    public static void Key(byte k) { keybd_event(k,0,0,UIntPtr.Zero); Thread.Sleep(40); keybd_event(k,0,2,UIntPtr.Zero); }
    // Position and button transition share one injection, so other pointer
    // motion cannot move the target between SetCursorPos and button-down.
    static void MouseAt(uint flags,int x,int y) {
        int left=GetSystemMetrics(76),top=GetSystemMetrics(77),w=GetSystemMetrics(78),h=GetSystemMetrics(79);
        uint ax=(uint)Math.Round((x-left)*65535.0/Math.Max(1,w-1));
        uint ay=(uint)Math.Round((y-top)*65535.0/Math.Max(1,h-1));
        mouse_event(0xC001|flags,ax,ay,0,UIntPtr.Zero);
    }
    public static void Click(int x,int y) { SetCursorPos(x,y); Thread.Sleep(120); MouseAt(2,x,y); Thread.Sleep(80); MouseAt(4,x,y); Thread.Sleep(60); }
    public static void Drag(int x,int y,int dx,int dy) {
        SetCursorPos(x,y); Thread.Sleep(120); MouseAt(2,x,y); Thread.Sleep(250);
        for(int i=1;i<=20;i++) { MouseAt(0,x+dx*i/20,y+dy*i/20); Thread.Sleep(50); }
        Thread.Sleep(200); MouseAt(4,x+dx,y+dy); Thread.Sleep(400);
    }
}
'@
[InputWindowProbe]::SetProcessDPIAware() | Out-Null
$checks = [Collections.Generic.List[object]]::new()
$cursorBefore = [InputWindowProbe+Point]::new()
[InputWindowProbe]::GetCursorPos([ref]$cursorBefore) | Out-Null
function Check([string]$Name, [bool]$Passed, $Evidence) {
    $checks.Add([pscustomobject][ordered]@{name=$Name; passed=$Passed; evidence=$Evidence})
    if (!$Passed) { throw "Failed: $Name ($Evidence)" }
}
function Capture([IntPtr]$Handle, [string]$Name) {
    $r = [InputWindowProbe]::Bounds($Handle)
    $bitmap = [Drawing.Bitmap]::new($r.R-$r.L, $r.B-$r.T)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($r.L,$r.T,0,0,$bitmap.Size)
        $bitmap.Save((Join-Path $out "$Name.png"), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
$profile = Join-Path $out "profile-$Appearance-$Presence"
$config = Join-Path $profile 'roaming/dev.sena-nana.nanatype/config'
New-Item -ItemType Directory -Force -Path $config | Out-Null
$settingsJson = @{schema=2; general=@{locale='en-US'; onboarding_done=$true; input_presence=$Presence};
    skin=@{chrome=$Appearance}; overlays=@{input=@{always_on_top=$true}; display=@{always_on_top=$false}; history=@{always_on_top=$false}};
    output=@{display=@{enabled=$true; name="NanaType Input Acceptance $Appearance"}}} |
    ConvertTo-Json -Depth 8
[IO.File]::WriteAllText((Join-Path $config 'settings.json'), $settingsJson, [Text.UTF8Encoding]::new($false))
$savedSize = $null
$savedSidebarWidth = 300.0
$geometryPath = Join-Path $config 'geometry.json'
if (Test-Path $geometryPath) {
    $geometry = Get-Content $geometryPath -Raw | ConvertFrom-Json
    if ($null -ne $geometry.input_sidebar_width) { $savedSidebarWidth = [Math]::Max(160.0,[Math]::Min(560.0,[double]$geometry.input_sidebar_width)) }
    foreach ($entry in $geometry.profiles.PSObject.Properties) {
        if ($entry.Value.input) { $savedSize = @($entry.Value.input.size); break }
    }
}
$variables = @{
    APPDATA=(Join-Path $profile 'roaming'); LOCALAPPDATA=(Join-Path $profile 'local');
    NANATYPE_DEBUG=(Join-Path $out "$Appearance-$Presence.log");
    NANATYPE_PERF=(Join-Path $out "$Appearance-$Presence-perf.json");
    NANATYPE_EXIT_AFTER='45000'; NANATYPE_DEMO='1'
}
$previous = @{}
$process = $null
foreach($name in $variables.Keys) { $previous[$name]=[Environment]::GetEnvironmentVariable($name,'Process') }
try {
    if (!$RestartOnly) {
    foreach($name in $variables.Keys) { $previous[$name]=[Environment]::GetEnvironmentVariable($name,'Process'); [Environment]::SetEnvironmentVariable($name,$variables[$name],'Process') }
    $process = Start-Process -FilePath (Join-Path $repo 'target/debug/nanatype.exe') -WorkingDirectory $repo -WindowStyle Hidden -PassThru
    foreach($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name,$previous[$name],'Process') }
    $inputWindow = [IntPtr]::Zero
    for($i=0; $i -lt 80 -and $inputWindow -eq [IntPtr]::Zero; $i++) {
        Start-Sleep -Milliseconds 200
        foreach($handle in [InputWindowProbe]::Windows($process.Id)) {
            $candidate = [InputWindowProbe]::Client($handle)
            $caption = [InputWindowProbe]::GetWindowLong($handle,-16) -band 0xC00000
            if([InputWindowProbe]::Title($handle) -in @('NanaType','Input','输入框') -and $caption -eq 0xC00000 -and $candidate.R -gt 100 -and $candidate.B -gt 100) { $inputWindow=$handle; break }
        }
    }
    Check 'input created' ($inputWindow -ne [IntPtr]::Zero) $process.Id
    # Let the scheduled demo submission finish before toggling summon mode.
    Start-Sleep -Milliseconds 3000
    [InputWindowProbe]::Key(0x77) # Real registered F8 hotkey, also opens summon mode.
    Start-Sleep -Milliseconds 500
    Check 'F8 shows input' ([InputWindowProbe]::IsWindowVisible($inputWindow)) $Presence
    $style = [InputWindowProbe]::GetWindowLong($inputWindow,-16)
    $extended = [InputWindowProbe]::GetWindowLong($inputWindow,-20)
    Check 'caption and sizing controls' (($style -band 0xCF0000) -eq 0xCF0000) ('0x{0:X}' -f $style)
    Check 'taskbar window' (($extended -band 0x80) -eq 0) ('0x{0:X}' -f $extended)
    $client = [InputWindowProbe]::Client($inputWindow)
    $dpi = [InputWindowProbe]::GetDpiForWindow($inputWindow)
    $scale = $dpi / 96.0
    # A reused acceptance profile intentionally tests saved dimensions on later runs.
    Check 'client minimum size' ($client.R/$scale -ge 719 -and $client.B/$scale -ge 359) "$($client.R)x$($client.B) dpi=$dpi"
    $expected = if ($savedSize) { $savedSize } else { @(1200,640) }
    Check 'initial or saved client dimensions restored' ([Math]::Abs($client.R/$scale-$expected[0]) -le 1 -and [Math]::Abs($client.B/$scale-$expected[1]) -le 1) "expected=$expected actual=$($client.R/$scale)x$($client.B/$scale)"
    [InputWindowProbe]::SetForegroundWindow($inputWindow) | Out-Null
    # Keep the drag away from desktop snap zones and the taskbar.
    [InputWindowProbe]::SetWindowPos($inputWindow,[IntPtr]::Zero,100,100,0,0,5) | Out-Null
    Start-Sleep -Milliseconds 200
    Capture $inputWindow "$Appearance-$Presence-initial"
    # Initial monitor placement may complete after native renderer Ready.
    Start-Sleep -Milliseconds 1000
    # The sidebar is always visible and controlled by top tabs.
    $dockOrigin = [InputWindowProbe+Point]::new()
    [InputWindowProbe]::ClientToScreen($inputWindow,[ref]$dockOrigin) | Out-Null
    $dockLeft = $dockOrigin.X + $client.R - [int]($savedSidebarWidth * $scale)
    $hitPoint = [InputWindowProbe+Point]::new()
    $hitPoint.X=$dockLeft+[int](40*$scale); $hitPoint.Y=$dockOrigin.Y+[int](24*$scale)
    $hitWindow = [InputWindowProbe]::WindowFromPoint($hitPoint)
    Check 'dock pointer reaches input window' ($hitWindow -eq $inputWindow) $null
    [InputWindowProbe]::Click($dockLeft + [int](40*$scale), $dockOrigin.Y + [int](24*$scale))
    Start-Sleep -Milliseconds 300
    Capture $inputWindow "$Appearance-$Presence-commands"
    [InputWindowProbe]::Click($dockLeft + [int](145*$scale), $dockOrigin.Y + [int](24*$scale))
    Start-Sleep -Milliseconds 300
    Capture $inputWindow "$Appearance-$Presence-stickers"
    [InputWindowProbe]::Click($dockLeft + [int](145*$scale), $dockOrigin.Y + [int](24*$scale))
    Start-Sleep -Milliseconds 200
    Capture $inputWindow "$Appearance-$Presence-active-tab"
    $dockClient = [InputWindowProbe]::Client($inputWindow)
    Check 'tab switching keeps client dimensions' ($dockClient.R -eq $client.R -and $dockClient.B -eq $client.B) "$($dockClient.R)x$($dockClient.B)"
    $dockOrigin = [InputWindowProbe+Point]::new()
    [InputWindowProbe]::ClientToScreen($inputWindow,[ref]$dockOrigin) | Out-Null
    $dockLeft = $dockOrigin.X + $client.R - [int]($savedSidebarWidth * $scale)
    $expectedSidebarWidth = [Math]::Min(560.0,$savedSidebarWidth+80.0)
    [InputWindowProbe]::Drag($dockLeft-[int](4*$scale),$dockOrigin.Y+[int](160*$scale),-[int](80*$scale),0)
    Start-Sleep -Milliseconds 800
    Capture $inputWindow "$Appearance-$Presence-sidebar-resized"
    $savedDock = Get-Content $geometryPath -Raw | ConvertFrom-Json
    Check 'sidebar drag persists preferred width' ([Math]::Abs($savedDock.input_sidebar_width-$expectedSidebarWidth) -le 1) "expected=$expectedSidebarWidth actual=$($savedDock.input_sidebar_width)"
    $r = [InputWindowProbe]::Bounds($inputWindow)
    $captionHit = [InputWindowProbe]::SendMessage($inputWindow,0x84,[IntPtr]::Zero,[IntPtr]((($r.T+16) -shl 16) -bor (($r.L+180) -band 0xffff)))
    Check 'native titlebar hit target' ($captionHit.ToInt64() -eq 2) $captionHit.ToInt64()
    [InputWindowProbe]::Drag($r.L+180,$r.T+16,70,30)
    $moved = [InputWindowProbe]::Bounds($inputWindow)
    for($i=0; $i -lt 20 -and $moved.L -eq $r.L; $i++) { Start-Sleep -Milliseconds 100; $moved = [InputWindowProbe]::Bounds($inputWindow) }
    Check 'titlebar drag' ([Math]::Abs($moved.L-$r.L) -gt 20) "$($r.L),$($r.T) -> $($moved.L),$($moved.T)"
    $edgeX = $moved.R-3
    $edgeY = $moved.B-3
    $edgePoint = [InputWindowProbe+Point]::new()
    $edgePoint.X=$edgeX; $edgePoint.Y=$edgeY
    $hit = [InputWindowProbe]::SendMessage($inputWindow,0x84,[IntPtr]::Zero,[IntPtr](($edgeY -shl 16) -bor ($edgeX -band 0xffff)))
    Check 'native resize hit target' ($hit.ToInt64() -eq 17) "hit=$hit top=$([InputWindowProbe]::Title([InputWindowProbe]::WindowFromPoint($edgePoint)))"
    [InputWindowProbe]::Drag($edgeX,$edgeY,120,70)
    $sized = [InputWindowProbe]::Bounds($inputWindow)
    for($i=0; $i -lt 20 -and ($sized.R-$sized.L) -eq ($moved.R-$moved.L); $i++) { Start-Sleep -Milliseconds 100; $sized = [InputWindowProbe]::Bounds($inputWindow) }
    Check 'edge resize' (($sized.R-$sized.L) -gt ($moved.R-$moved.L)+30) "$($sized.R-$sized.L)x$($sized.B-$sized.T)"
    Capture $inputWindow "$Appearance-$Presence-resized"
    [InputWindowProbe]::ShowWindow($inputWindow,3) | Out-Null
    Start-Sleep -Milliseconds 400
    Check 'maximize' ([InputWindowProbe]::IsZoomed($inputWindow)) $null
    Capture $inputWindow "$Appearance-$Presence-maximized"
    [InputWindowProbe]::ShowWindow($inputWindow,9) | Out-Null
    Start-Sleep -Milliseconds 400
    $restored = [InputWindowProbe]::Bounds($inputWindow)
    Check 'restore keeps resized dimensions' (($restored.R-$restored.L) -eq ($sized.R-$sized.L) -and ($restored.B-$restored.T) -eq ($sized.B-$sized.T)) "before=$($sized.R-$sized.L)x$($sized.B-$sized.T) after=$($restored.R-$restored.L)x$($restored.B-$restored.T)"
    [InputWindowProbe]::ShowWindow($inputWindow,6) | Out-Null
    Start-Sleep -Milliseconds 300
    Check 'minimize' ([InputWindowProbe]::IsIconic($inputWindow)) $null
    [InputWindowProbe]::Key(0x77)
    Start-Sleep -Milliseconds 500
    Check 'F8 restores minimized input' (![InputWindowProbe]::IsIconic($inputWindow) -and [InputWindowProbe]::IsWindowVisible($inputWindow)) $null
    # Type through real key events, then verify the draft visually across hide/show.
    [InputWindowProbe]::SetForegroundWindow($inputWindow) | Out-Null
    $origin = [InputWindowProbe+Point]::new()
    [InputWindowProbe]::ClientToScreen($inputWindow,[ref]$origin) | Out-Null
    $client = [InputWindowProbe]::Client($inputWindow)
    [InputWindowProbe]::Click($origin.X+[int]($client.R/2),$origin.Y+[int]((84*$scale + $client.B-32*$scale)/2))
    foreach($key in @(0x44,0x52,0x41,0x46,0x54)) { [InputWindowProbe]::Key($key) }
    # A Chinese system IME can keep these keys in preedit. Commit its first
    # candidate (or insert a space in English) before testing close/F8.
    [InputWindowProbe]::Key(0x20)
    Capture $inputWindow "$Appearance-$Presence-draft-before-hide"
    [InputWindowProbe]::PostMessage($inputWindow,0x10,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 600
    Check 'close hides without exiting' (![InputWindowProbe]::IsWindowVisible($inputWindow) -and !$process.HasExited) $null
    $display = @([InputWindowProbe]::Windows($process.Id) | Where-Object { [InputWindowProbe]::Title($_) -in @('Display','展示框') })
    Check 'display survives closing input' ($display.Count -gt 0 -and [InputWindowProbe]::IsWindowVisible($display[0])) $null
    # Leave input hidden through the live output measurement interval.
    Start-Sleep -Milliseconds 2000
    [InputWindowProbe]::Key(0x77)
    Start-Sleep -Milliseconds 500
    for($i=0; $i -lt 20 -and ![InputWindowProbe]::IsWindowVisible($inputWindow); $i++) { Start-Sleep -Milliseconds 100 }
    Check 'F8 reopens closed input' ([InputWindowProbe]::IsWindowVisible($inputWindow)) $null
    Capture $inputWindow "$Appearance-$Presence-draft-after-show"
    $after = [InputWindowProbe]::Bounds($inputWindow)
    Check 'hide/show preserves user dimensions' (($after.R-$after.L) -eq ($sized.R-$sized.L)) $null
    # Wait for the application's own graceful quit so persistence and perf flush.
    while(!$process.HasExited) { Start-Sleep -Milliseconds 500 }
    Check 'normal application exit' ($process.ExitCode -eq 0) $process.ExitCode
    $perf = Get-Content (Join-Path $out "$Appearance-$Presence-perf.json") -Raw | ConvertFrom-Json
    $output = @($perf.windows | Where-Object { $_.window -eq 'Some(Display)' })[0]
    Check 'live output continues' ($output.output_after_6s -gt 0) $output
    Check 'geometry persisted' (Test-Path (Join-Path $config 'geometry.json')) $null
    } else {
        # Keep successful native control/output evidence while retrying only
        # process restart (for example after another instance held the port).
        $prior = Get-Content (Join-Path $out "$Appearance-$Presence-checks.json") -Raw | ConvertFrom-Json
        $restartChecks = @('input recreated after restart','restart input is visible','saved sidebar handle restored after restart','restart exits normally')
        $primary = @($prior | Where-Object { $_.name -notin $restartChecks })
        if ($primary.Count -ne 24 -or @($primary | Where-Object { !$_.passed }).Count -gt 0) { throw 'Restart-only requires all 24 primary native checks to have passed.' }
        foreach($check in $primary) { $checks.Add($check) }
        $expectedSidebarWidth = $savedSidebarWidth
    }
    Start-Sleep -Milliseconds 2000
    # A second real process must restore the handle to its saved location.
    $restartVariables = $variables.Clone()
    $restartVariables.NANATYPE_EXIT_AFTER = '12000'
    $restartVariables.NANATYPE_DEMO = $null
    $restartVariables.NANATYPE_DEBUG = Join-Path $out "$Appearance-$Presence-restart.log"
    $restartVariables.NANATYPE_PERF = Join-Path $out "$Appearance-$Presence-restart-perf.json"
    foreach($name in $restartVariables.Keys) { [Environment]::SetEnvironmentVariable($name,$restartVariables[$name],'Process') }
    $process = Start-Process -FilePath (Join-Path $repo 'target/debug/nanatype.exe') -WorkingDirectory $repo -WindowStyle Hidden -PassThru
    foreach($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name,$previous[$name],'Process') }
    $inputWindow = [IntPtr]::Zero
    for($i=0; $i -lt 60 -and $inputWindow -eq [IntPtr]::Zero; $i++) {
        Start-Sleep -Milliseconds 100
        foreach($handle in [InputWindowProbe]::Windows($process.Id)) {
            $candidate = [InputWindowProbe]::Client($handle)
            if([InputWindowProbe]::Title($handle) -in @('NanaType','Input','输入框') -and $candidate.R -gt 100 -and $candidate.B -gt 100) { $inputWindow=$handle; break }
        }
    }
    Check 'input recreated after restart' ($inputWindow -ne [IntPtr]::Zero) $process.Id
    # Caption/client creation precedes renderer Ready and hotkey registration.
    Start-Sleep -Milliseconds 3000
    [InputWindowProbe]::Key(0x77)
    [InputWindowProbe]::SetForegroundWindow($inputWindow) | Out-Null
    [InputWindowProbe]::SetWindowPos($inputWindow,[IntPtr]::Zero,100,100,0,0,5) | Out-Null
    Start-Sleep -Milliseconds 400
    $client = [InputWindowProbe]::Client($inputWindow)
    $scale = [InputWindowProbe]::GetDpiForWindow($inputWindow) / 96.0
    $restoredOrigin = [InputWindowProbe+Point]::new()
    [InputWindowProbe]::ClientToScreen($inputWindow,[ref]$restoredOrigin) | Out-Null
    Check 'restart input is visible' ([InputWindowProbe]::IsWindowVisible($inputWindow)) $null
    Capture $inputWindow "$Appearance-$Presence-sidebar-restored"
    $restoredDivider = $restoredOrigin.X + $client.R - [int](($expectedSidebarWidth+4)*$scale)
    [InputWindowProbe]::Drag($restoredDivider,$restoredOrigin.Y+[int](160*$scale),-[int](20*$scale),0)
    Start-Sleep -Milliseconds 800
    $restoredDock = Get-Content $geometryPath -Raw | ConvertFrom-Json
    Check 'saved sidebar handle restored after restart' ([Math]::Abs($restoredDock.input_sidebar_width-($expectedSidebarWidth+20)) -le 1) "expected=$($expectedSidebarWidth+20) actual=$($restoredDock.input_sidebar_width)"
    Capture $inputWindow "$Appearance-$Presence-sidebar-restored-drag"
    while(!$process.HasExited) { Start-Sleep -Milliseconds 500 }
    Check 'restart exits normally' ($process.ExitCode -eq 0) $process.ExitCode

} finally {
    foreach($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name,$previous[$name],'Process') }
    # Only this script's test process is owned by the harness.
    if($process -and !$process.HasExited) { $process.Kill(); $process.WaitForExit() }
    [InputWindowProbe]::SetCursorPos($cursorBefore.X,$cursorBefore.Y) | Out-Null
    $checks | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 (Join-Path $out "$Appearance-$Presence-checks.json")
}
$checks | Format-Table name,passed
