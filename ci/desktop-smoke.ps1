<#
.SYNOPSIS
Build and exercise Regressify Desktop in a synthetic Windows workspace.
.DESCRIPTION
Requires an interactive Windows desktop. Captures only the application window.
Artifacts and the fixture workspace remain under target/desktop-smoke.
#>
param(
    [switch]$SkipBuild,
    [switch]$KeepOpen,
    [switch]$Capture,
    [switch]$ScenarioFlows,
    [switch]$ArtifactFlows,
    [switch]$InstanceFlows,
    [switch]$UpdateFlows
)

$ErrorActionPreference = 'Stop'
if ($ScenarioFlows -or $UpdateFlows) { $Capture = $true }
$repository = Split-Path -Parent $PSScriptRoot
$artifacts = Join-Path $repository 'target/desktop-smoke'
$workspace = Join-Path $artifacts ('workspace-' + [guid]::NewGuid().ToString('N'))
$executable = Join-Path $repository 'target/debug/regressify-desktop.exe'

if (-not $IsWindows) { throw 'This smoke test requires Windows.' }
if (Get-Process regressify-desktop -ErrorAction SilentlyContinue) { throw 'Close desktop windows before native smoke checks because the fixture temporarily changes shared preferences.' }
if (-not $SkipBuild) {
    Push-Location $repository
    try {
        & cargo build --features gui --bin regressify-desktop --quiet
        if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed.' }
    } finally { Pop-Location }
}

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
if (-not ('RegressifySmokeNative' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RegressifySmokeNative {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rectangle);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
    [DllImport("dwmapi.dll")] public static extern int DwmFlush();
}
'@
}

if (-not ('RegressifyWindowCapture' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RegressifyWindowCapture {
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr window, IntPtr deviceContext, uint flags);
}
'@
}

function Write-JsonFixture([string]$Path, $Value) {
    [System.IO.Directory]::CreateDirectory((Split-Path -Parent $Path)) | Out-Null
    [System.IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 20), [System.Text.UTF8Encoding]::new($false))
}

if (-not ('RegressifySmokeFocus' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RegressifySmokeFocus {
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}
'@
}

function Focus-Fixture {
    $activation = New-Object -ComObject WScript.Shell
    $deadline = [System.Diagnostics.Stopwatch]::StartNew()
    do {
        if ($process.HasExited) { throw 'The native fixture exited before input could be delivered.' }
        $activation.AppActivate([int]$process.Id) | Out-Null
        if ([RegressifySmokeFocus]::GetForegroundWindow() -eq $script:window) { return }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($deadline.Elapsed.TotalSeconds -lt 2)
    throw 'Cannot focus the native fixture. Keep Windows unlocked and the interactive desktop available.'
}

if (-not ('RegressifySmokePointer' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RegressifySmokePointer {
    [DllImport("user32.dll")] public static extern bool SetPhysicalCursorPos(int x, int y);
}
'@
}

if (-not ('RegressifySmokeText' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class RegressifySmokeText {
    [StructLayout(LayoutKind.Sequential)]
    struct MouseInput { public int dx, dy; public uint data, flags, time; public UIntPtr extra; }
    [StructLayout(LayoutKind.Sequential)]
    struct KeyInput { public ushort key, scan; public uint flags, time; public UIntPtr extra; }
    [StructLayout(LayoutKind.Explicit)]
    struct InputData { [FieldOffset(0)] public MouseInput mouse; [FieldOffset(0)] public KeyInput keyboard; }
    [StructLayout(LayoutKind.Sequential)]
    struct Input { public uint type; public InputData data; }
    [DllImport("user32.dll", SetLastError = true)]
    static extern uint SendInput(uint count, Input[] inputs, int size);
    public static void Type(string text) {
        var inputs = new Input[text.Length * 2];
        for (var index = 0; index < text.Length; index++) {
            inputs[index * 2] = new Input { type = 1, data = new InputData { keyboard = new KeyInput { scan = text[index], flags = 4 } } };
            inputs[index * 2 + 1] = new Input { type = 1, data = new InputData { keyboard = new KeyInput { scan = text[index], flags = 6 } } };
        }
        if (SendInput((uint)inputs.Length, inputs, Marshal.SizeOf<Input>()) != inputs.Length)
            throw new Win32Exception(Marshal.GetLastWin32Error());
    }
}
'@
}

function Send-FixtureText([string]$Text) {
    Focus-Fixture
    [RegressifySmokeText]::Type($Text)
    [RegressifySmokeNative]::DwmFlush() | Out-Null
}

function Send-FixtureKeys([string]$Keys) {
    Focus-Fixture
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    [RegressifySmokeNative]::DwmFlush() | Out-Null
}

function Write-TestImage([string]$Path, [System.Drawing.Color]$Color) {
    [System.IO.Directory]::CreateDirectory((Split-Path -Parent $Path)) | Out-Null
    $bitmap = [System.Drawing.Bitmap]::new(640, 420)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $brush = [System.Drawing.SolidBrush]::new($Color)
    $font = [System.Drawing.Font]::new('Segoe UI', 28)
    try {
        $graphics.Clear([System.Drawing.Color]::White)
        $graphics.FillRectangle($brush, 30, 30, 580, 80)
        $graphics.DrawString('Checkout fixture', $font, [System.Drawing.Brushes]::White, 50, 45)
        $graphics.DrawString('Order total: $42.00', $font, [System.Drawing.Brushes]::Black, 40, 160)
        $graphics.FillRectangle($brush, 40, 270, 230, 65)
        $graphics.DrawString('Continue', $font, [System.Drawing.Brushes]::White, 50, 280)
        $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $font.Dispose()
        $brush.Dispose()
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

Write-JsonFixture (Join-Path $workspace 'visual_tests/smoke.tests.json') ([ordered]@{
    misMatchThreshold = 0.1
    scenarios = @(
        [ordered]@{ id = '8ac900fea19642cc0000'; label = 'Checkout'; url = 'https://example.com/checkout'; actions = @(@{ click = '#continue' }, @{ wait = 1000 }) },
        [ordered]@{ label = 'Signed-in account'; url = 'https://example.com/account'; actions = @() }
    )
})
Write-JsonFixture (Join-Path $workspace 'visual_tests/account.tests.json') ([ordered]@{
    scenarios = @([ordered]@{ label = 'Profile'; url = 'https://example.com/profile'; actions = @() })
})
Write-JsonFixture (Join-Path $workspace 'common/_viewports.yaml') @(
    @{ label = 'phone'; width = 390; height = 844 },
    @{ label = 'desktop'; width = 1280; height = 720 }
)
[System.IO.File]::WriteAllText((Join-Path $workspace 'regressify.yaml'), "bypassCsp: true`npostInteractionWait: 1000`nviewportNames: [desktop]`n")
if ($Capture) {
    $page = Join-Path $workspace 'fixture.html'
    [System.IO.File]::WriteAllText($page, '<!doctype html><html><head><title>QA fixture</title></head><body style="font:24px sans-serif;padding:48px;background:#f0f4f3"><h1>Checkout</h1><button id="continue">Continue</button><p>Order total: $42.00</p></body></html>')
    $url = [System.Uri]::new($page).AbsoluteUri
    Write-JsonFixture (Join-Path $workspace 'visual_tests/smoke.tests.json') ([ordered]@{
        scenarios = @(
            [ordered]@{ id = '8ac900fea19642cc0000'; label = 'Checkout'; url = $url; actions = @(@{ wait = 5000 }, @{ click = '#continue' }) },
            [ordered]@{ label = 'Signed-in account'; url = $url; actions = @(@{ wait = 2000 }) }
        )
    })
    Write-JsonFixture (Join-Path $workspace 'visual_tests/account.tests.json') ([ordered]@{
        scenarios = @([ordered]@{ label = 'Profile'; url = $url; actions = @(@{ wait = 5000 }) })
    })
}
$output = Join-Path $workspace '.regressify/smoke'
[System.IO.Directory]::CreateDirectory((Join-Path $output 'html_report')) | Out-Null
Write-TestImage (Join-Path $output 'bitmaps_reference/checkout.png') ([System.Drawing.Color]::Teal)
Write-TestImage (Join-Path $output 'bitmaps_test/smoke-run/checkout.png') ([System.Drawing.Color]::RoyalBlue)
Write-TestImage (Join-Path $output 'bitmaps_test/smoke-run/checkout_diff.png') ([System.Drawing.Color]::Crimson)
Write-JsonFixture (Join-Path $output 'bitmaps_test/smoke-run/report.json') ([ordered]@{
    id = 'smoke'
    tests = @(
        @{ status = 'fail'; pair = @{ label = '8ac900fea19642cc0000 desktop FINAL'; fileName = '8ac900fea19642cc0000_desktop__FINAL.png'; reference = '../bitmaps_reference/checkout.png'; test = '../bitmaps_test/smoke-run/checkout.png'; diffImage = '../bitmaps_test/smoke-run/checkout_diff.png'; diff = @{ rawMisMatchPercentage = 4.2; misMatchPercentage = '4.20' } } },
        @{ status = 'fail'; pair = @{ label = 'Account desktop'; engineErrorMsg = 'Missing selector #account' } }
    )
})

$stderr = Join-Path $artifacts 'native-stderr.log'
$stdout = Join-Path $artifacts 'native-stdout.log'
$settingsPath = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'Regressify/Regressify Desktop/config/settings.json'
$settingsExisted = Test-Path -LiteralPath $settingsPath
$settingsBackup = Join-Path $workspace 'original-desktop-settings.json'
if ($settingsExisted) { Copy-Item -LiteralPath $settingsPath -Destination $settingsBackup }
$process = $null
$script:window = [IntPtr]::Zero
$script:fixtureProcesses = [System.Collections.Generic.List[System.Diagnostics.Process]]::new()

function Start-Fixture([string]$Root = $workspace, [string]$Name = 'native', [switch]$Empty) {
    $errorLog = Join-Path $artifacts ($Name + '-stderr.log')
    $start = @{ FilePath = $executable; RedirectStandardError = $errorLog; RedirectStandardOutput = (Join-Path $artifacts ($Name + '-stdout.log')); PassThru = $true }
    if (-not $Empty) { $start.ArgumentList = '"' + $Root + '"' }
    $script:process = Start-Process @start
    $script:fixtureProcesses.Add($process)
    if (-not $process.WaitForInputIdle(15000)) { throw 'Desktop did not initialize.' }
    $initialization = [System.Diagnostics.Stopwatch]::StartNew()
    do {
        $process.Refresh()
        $script:window = $process.MainWindowHandle
        if ($script:window -ne [IntPtr]::Zero -or $process.HasExited) { break }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($initialization.Elapsed.TotalSeconds -lt 15)
    if ($script:window -eq [IntPtr]::Zero) { throw "Desktop has no native window. $(Get-Content $errorLog -Raw)" }
    $script:screen = [System.Windows.Forms.Screen]::FromHandle($script:window).WorkingArea
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    if ($Empty) { Wait-Control 'Open workspace' | Out-Null } else { Wait-Control 'Workspace opened' | Out-Null }
}

function Wait-Settings([scriptblock]$Match) {
    $deadline = [System.Diagnostics.Stopwatch]::StartNew()
    do {
        $settings = Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json
        if (& $Match $settings) { return }
        if ($process.HasExited) { throw 'Desktop exited before preferences were saved.' }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($deadline.Elapsed.TotalSeconds -lt 10)
    throw 'Desktop preferences were not persisted.'
}

function Select-Theme([string]$Theme) {
    $current = @('Light', 'Dark', 'System') | Where-Object { $null -ne (Find-Control ('Appearance: ' + $_)) } | Select-Object -First 1
    if ([string]::IsNullOrEmpty($current)) { throw 'Appearance control is missing.' }
    if ($current -ne $Theme) {
        Invoke-Control ('Appearance: ' + $current) -Pointer
        Invoke-Control $Theme
    }
    Wait-Control ('Appearance: ' + $Theme) | Out-Null
    Wait-Settings { param($settings) $settings.theme -eq $Theme.ToLowerInvariant() }
}

function Find-Control([string]$Name, [string]$Within = '') {
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($script:window)
    if ($Within) {
        $container = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, $Within)
        $root = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $container)
        if ($null -eq $root) { return $null }
    }
    $condition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, $Name)
    $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}

function Wait-Control([string]$Name, [string]$Within = '') {
    $deadline = [System.Diagnostics.Stopwatch]::StartNew()
    $stable = [System.Diagnostics.Stopwatch]::StartNew()
    $previousBounds = $null
    do {
        $element = Find-Control $Name $Within
        if ($null -ne $element -and -not $element.Current.IsOffscreen) {
            $bounds = $element.Current.BoundingRectangle
            if ($bounds -ne $previousBounds) {
                $previousBounds = $bounds
                $stable.Restart()
            } elseif ($stable.ElapsedMilliseconds -ge 150) { return $element }
        } else {
            $previousBounds = $null
            $stable.Restart()
        }
        if ($process.HasExited) { throw "Desktop exited with code $($process.ExitCode)." }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($deadline.Elapsed.TotalSeconds -lt 60)
    Capture-Window 'missing-control'
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($script:window)
    $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
        Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::MenuItem } |
        ForEach-Object { Write-Output ('Available menu item: ' + $_.Current.Name) }
    Capture-Window 'control-failure'
    throw "Native control '$Name' was not found."
}

function Test-ControlInvokable([string]$Name) {
    $pattern = $null
    (Wait-Control $Name).TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)
}

function Invoke-Control([string]$Name, [switch]$Pointer, [string]$Within = '') {
    $element = Wait-Control $Name $Within
    $pattern = $null
    if (-not $Pointer -and $element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        ([System.Windows.Automation.InvokePattern]$pattern).Invoke()
    } elseif ($element.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) {
        ([System.Windows.Automation.SelectionItemPattern]$pattern).Select()
    } elseif ($element.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) {
        ([System.Windows.Automation.TogglePattern]$pattern).Toggle()
    } else {
        [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, 0, 0, 0, 0, 0x0043) | Out-Null
        Focus-Fixture
        $bounds = $element.Current.BoundingRectangle
        if (-not [RegressifySmokePointer]::SetPhysicalCursorPos([int]($bounds.Left + $bounds.Width / 2), [int]($bounds.Top + $bounds.Height / 2))) {
            throw "Cannot position the mouse over '$Name'."
        }
        [RegressifySmokeNative]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
        [RegressifySmokeNative]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    }
    [RegressifySmokeNative]::DwmFlush() | Out-Null
}

function Read-ControlText([string]$Name) {
    $element = Wait-Control $Name
    $pattern = $null
    if ($element.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
        return ([System.Windows.Automation.ValuePattern]$pattern).Current.Value
    }
    if ($element.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$pattern)) {
        return ([System.Windows.Automation.TextPattern]$pattern).DocumentRange.GetText(-1)
    }
    throw "Control '$Name' does not expose accessible text."
}

function Wait-SourceText([string]$Text, [string]$ControlName = 'Configuration source') {
    $deadline = [System.Diagnostics.Stopwatch]::StartNew()
    do {
        $value = Read-ControlText $ControlName
        if ($value.Contains($Text)) { return }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($deadline.Elapsed.TotalSeconds -lt 10)
    throw "Control '$ControlName' did not contain '$Text'."
}

function Capture-Window([string]$Name) {
    [RegressifySmokeNative]::DwmFlush() | Out-Null
    $bounds = [RegressifySmokeNative+Rect]::new()
    if (-not [RegressifySmokeNative]::GetWindowRect($script:window, [ref]$bounds)) { throw 'Cannot read window bounds.' }
    $width = $bounds.Right - $bounds.Left
    $height = $bounds.Bottom - $bounds.Top
    if ($width -lt 640 -or $height -lt 480) { throw 'Unexpected native window dimensions.' }
    $bitmap = [System.Drawing.Bitmap]::new($width, $height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $deviceContext = $graphics.GetHdc()
        try {
            if (-not [RegressifyWindowCapture]::PrintWindow($script:window, $deviceContext, 2)) { throw 'Cannot capture the application window.' }
        } finally { $graphics.ReleaseHdc($deviceContext) }
        $colors = [System.Collections.Generic.HashSet[int]]::new()
        for ($row = 20; $row -lt $height - 20; $row += 11) {
            for ($column = 20; $column -lt $width - 20; $column += 11) {
                $colors.Add($bitmap.GetPixel($column, $row).ToArgb()) | Out-Null
            }
        }
        if ($colors.Count -lt 20) { throw "Window '$Name' appears blank ($($colors.Count) sampled colors)." }
        $path = Join-Path $artifacts ($Name + '.png')
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        Write-Output "$Name`: ${width}x${height}, $($colors.Count) sampled colors, $path"
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}

function Test-InstanceFlows {
    if (-not ('RegressifyInstanceWindow' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RegressifyInstanceWindow {
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr window);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
}
'@
    }
    function Wait-OwnerForeground([System.Diagnostics.Process]$Owner) {
        $deadline = [System.Diagnostics.Stopwatch]::StartNew()
        do {
            if ($Owner.HasExited) { throw 'The workspace owner exited during activation.' }
            $Owner.Refresh()
            $handle = $Owner.MainWindowHandle
            if ($handle -ne [IntPtr]::Zero -and [RegressifySmokeFocus]::GetForegroundWindow() -eq $handle -and -not [RegressifyInstanceWindow]::IsIconic($handle)) { return }
            [RegressifySmokeNative]::DwmFlush() | Out-Null
        } while ($deadline.Elapsed.TotalSeconds -lt 10)
        throw 'The existing workspace window was not restored and foregrounded.'
    }
    function Open-Duplicate([string]$Root, [System.Diagnostics.Process]$Owner, [string]$Name) {
        $duplicate = Start-Process -FilePath $executable -ArgumentList ('"' + $Root + '"') -PassThru `
            -RedirectStandardOutput (Join-Path $artifacts ($Name + '-stdout.log')) -RedirectStandardError (Join-Path $artifacts ($Name + '-stderr.log'))
        $script:fixtureProcesses.Add($duplicate)
        if (-not $duplicate.WaitForExit(15000)) { throw 'A duplicate launcher did not exit after workspace handoff.' }
        if ($duplicate.ExitCode -ne 0) { throw "Duplicate launcher failed with exit code $($duplicate.ExitCode)." }
        Wait-OwnerForeground $Owner
    }
    $first = $process
    $source = Join-Path $workspace 'visual_tests/smoke.tests.json'
    $originalSource = (Get-FileHash $source).Hash
    Invoke-Control 'smoke.tests.json'
    Invoke-Control 'Checkout'
    Invoke-Control 'Duplicate selected item'
    Wait-Control 'Unsaved changes' | Out-Null
    Capture-Window 'instances-first-draft'
    $secondRoot = Join-Path $artifacts ('workspace-' + [guid]::NewGuid().ToString('N'))
    Write-JsonFixture (Join-Path $secondRoot 'visual_tests/second.tests.json') @{ scenarios = @(@{ label = 'Second workspace'; url = 'about:blank' }) }
    Start-Fixture -Root $secondRoot -Name 'instance-second'
    $second = $process
    if ($first.HasExited) { throw 'Opening another workspace closed the first instance.' }
    Wait-Control ($secondRoot.Replace('\', '/')) | Out-Null
    Capture-Window 'instances-second-workspace'
    [RegressifyInstanceWindow]::ShowWindow($first.MainWindowHandle, 6) | Out-Null
    Open-Duplicate (Join-Path $workspace '.') $first 'instance-duplicate'
    $script:process = $first
    $script:window = $first.MainWindowHandle
    Wait-Control 'Unsaved changes' | Out-Null
    Capture-Window 'instances-duplicate-focused'
    if ((Get-FileHash $source).Hash -ne $originalSource) { throw 'Workspace handoff changed the saved document.' }
    $script:process = $second
    $script:window = $second.MainWindowHandle
    [RegressifyInstanceWindow]::ShowWindow($first.MainWindowHandle, 6) | Out-Null
    Open-Duplicate $workspace.ToUpperInvariant() $first 'instance-case-alias'
    Start-Fixture -Empty -Name 'instance-empty'
    $empty = $process
    Invoke-Control (Split-Path -Leaf $workspace)
    Wait-Control 'Switched to the existing workspace window' | Out-Null
    Wait-OwnerForeground $first
    Capture-Window 'instances-empty-redirect'
    Start-Fixture -Empty -Name 'instance-empty-second'
    if ($empty.HasExited) { throw 'An empty desktop instance was globally restricted.' }
    Capture-Window 'instances-concurrent-empty'
    $first.Kill($true)
    $first.WaitForExit()
    Start-Fixture -Root $workspace -Name 'instance-crash-recovery'
    $recovered = $process
    if ($second.HasExited) { throw 'Recovering one workspace affected another instance.' }
    if ((Get-FileHash $source).Hash -ne $originalSource) { throw 'Recovery changed the saved source.' }
    Capture-Window 'instances-crash-recovery'
    Open-Duplicate $workspace $recovered 'instance-recovered-activation'
    Invoke-Control 'File' -Pointer
    Invoke-Control 'Close workspace'
    Wait-Control 'Workspace closed' | Out-Null
    Start-Fixture -Root $workspace -Name 'instance-reopened'
    if ($recovered.HasExited) { throw 'Closing a workspace should leave its window available.' }
    Capture-Window 'instances-reopened-workspace'
    Write-Output 'Multiple workspace/empty windows, duplicate activation, minimized restoration, draft preservation, and close/crash ownership release passed.'
}

function Test-ArtifactFlows {
    Invoke-Control 'account.tests.json'
    Invoke-Control 'Profile'
    Wait-Control '0 reference images / 0 saved test runs' | Out-Null
    Capture-Window 'artifacts-no-baseline'
    if ((Test-ControlInvokable 'Run Suite Test') -or (Test-ControlInvokable 'Run Scenario Test')) {
        throw 'Test was enabled without a baseline.'
    }
    if (-not (Test-ControlInvokable 'Run Suite Reference')) { throw 'Reference capture must remain available.' }
    Invoke-Control 'Runs'
    Invoke-Control 'Suites'
    Invoke-Control 'Select all'
    if (Test-ControlInvokable 'Run Test') { throw 'A multi-suite test was enabled with a missing suite baseline.' }
    Capture-Window 'artifacts-mixed-selection'
    Invoke-Control 'Editor'
    Invoke-Control 'smoke.tests.json'
    Invoke-Control 'Checkout'
    Wait-Control '1 reference image / 1 saved test run' | Out-Null
    if (-not (Test-ControlInvokable 'Run Suite Test')) { throw 'Existing references did not enable Test.' }
    $reportPath = Join-Path $output 'bitmaps_test/smoke-run/report.json'
    $reportHash = (Get-FileHash $reportPath).Hash
    Capture-Window 'artifacts-existing-suite'
    Invoke-Control 'Open Report'
    Wait-Control '1 report, 0 warnings' | Out-Null
    Wait-Control 'Reference 640 x 420' | Out-Null
    Wait-Control 'Current 640 x 420' | Out-Null
    Capture-Window 'artifacts-existing-report'
    if ((Get-FileHash $reportPath).Hash -ne $reportHash) { throw 'Opening an existing report rewrote its data.' }
    Invoke-Control 'Runs'
    Wait-Control 'No command running' | Out-Null
    Invoke-Control 'Editor'
    $accountOutput = Join-Path $workspace '.regressify/account'
    $reference = Join-Path $accountOutput 'bitmaps_reference/profile__FINAL.png'
    $capture = Join-Path $accountOutput 'bitmaps_test/smoke-run/profile__FINAL.png'
    Write-TestImage $reference ([System.Drawing.Color]::Teal)
    Write-TestImage $capture ([System.Drawing.Color]::Teal)
    $captureHash = (Get-FileHash $capture).Hash
    $referenceHash = (Get-FileHash $reference).Hash
    Invoke-Control 'account.tests.json'
    Invoke-Control 'Profile'
    Wait-Control '1 reference image / 1 saved test run' | Out-Null
    if (-not (Test-ControlInvokable 'Run Scenario Test')) { throw 'Selecting a suite did not detect new baselines.' }
    Select-Theme 'Dark'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'artifacts-detected-dark-compact'
    Invoke-Control 'Open Report'
    Wait-Control '1 report, 0 warnings' | Out-Null
    Wait-Control 'Reference 640 x 420' | Out-Null
    Wait-Control 'Current 640 x 420' | Out-Null
    Capture-Window 'artifacts-compared-saved-run'
    $accountReport = Get-Content (Join-Path $accountOutput 'bitmaps_test/smoke-run/report.json') -Raw | ConvertFrom-Json
    if ($accountReport.tests.Count -ne 1 -or $accountReport.tests[0].status -ne 'pass') { throw 'Saved captures were not compared correctly.' }
    if ((Get-FileHash $reference).Hash -ne $referenceHash -or (Get-FileHash $capture).Hash -ne $captureHash) {
        throw 'Opening the saved capture report changed screenshots.'
    }
    if (@(Get-ChildItem (Join-Path $accountOutput 'bitmaps_test') -Directory).Count -ne 1) { throw 'Opening a saved report created a new capture run.' }
    $process.CloseMainWindow() | Out-Null
    if (-not $process.WaitForExit(10000)) { throw 'Artifact fixture did not close cleanly.' }
    Start-Fixture
    Invoke-Control 'account.tests.json'
    Invoke-Control 'Profile'
    Wait-Control '1 reference image / 1 saved test run' | Out-Null
    Invoke-Control 'Open Report'
    Wait-Control 'Reference 640 x 420' | Out-Null
    Wait-Control 'Current 640 x 420' | Out-Null
    Capture-Window 'artifacts-report-after-restart'
    Invoke-Control 'Runs'
    Wait-Control 'No command running' | Out-Null
    Write-Output 'Baseline gating, saved-report opening, capture-only comparison, and restart detection passed without browser capture.'
}

function Test-ScenarioFlows {
    Invoke-Control 'smoke.tests.json'
    Invoke-Control 'Checkout'
    Capture-Window 'scenario-editor'
    Invoke-Control 'Run Suite Reference'
    Wait-Control 'Run confirmation' | Out-Null
    Invoke-Control 'Run Reference' -Within 'Run confirmation'
    Wait-Control 'Capturing scenarios' | Out-Null
    Wait-Control 'Completed' | Out-Null
    Wait-Control 'Passed: 2' | Out-Null
    $siblingReferences = @(Get-ChildItem (Join-Path $output 'bitmaps_reference') -Filter '*.png' |
        Where-Object { -not $_.Name.StartsWith('8ac900fea19642cc0000') } |
        ForEach-Object { @{ Path = $_.FullName; Hash = (Get-FileHash $_.FullName).Hash } })
    Invoke-Control 'Suites'
    Invoke-Control 'Select all'
    Invoke-Control 'Select all'
    Invoke-Control 'Select scenario Checkout'
    Wait-Control 'Scenarios: smoke / Checkout' | Out-Null
    Invoke-Control 'smoke (1/2 scenarios)'
    Capture-Window 'scenario-selection'
    Invoke-Control 'Run settings'
    Capture-Window 'scenario-settings-scrollbars'
    Select-Theme 'Dark'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'scenario-settings-dark-compact'
    Select-Theme 'Light'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    Invoke-Control 'Run Test'
    Wait-Control 'Capturing scenarios' | Out-Null
    Wait-Control 'Scenarios: smoke / Checkout' | Out-Null
    Capture-Window 'scenario-only-running'
    Invoke-Control 'View log: Checkout (desktop)'
    Wait-SourceText 'Executing scenario: Checkout' 'Scenario run log'
    Capture-Window 'scenario-log-live'
    Wait-Control 'Completed' | Out-Null
    Wait-SourceText 'Clicking element: #continue' 'Scenario run log'
    $log = Read-ControlText 'Scenario run log'
    if ($log.Contains('Signed-in account') -or $log.Contains('Executing scenario: Profile')) { throw 'Scenario log contains a sibling scenario.' }
    if (-not $log.Contains('Comparison passed:')) { throw 'Scenario log omitted the final comparison outcome.' }
    Capture-Window 'scenario-log'
    $reports = Get-ChildItem (Join-Path $output 'bitmaps_test') -Recurse -Filter report.json |
        Where-Object { $_.Directory.Name.StartsWith('desktop-') }
    $pairs = @($reports | ForEach-Object { (Get-Content $_.FullName -Raw | ConvertFrom-Json).tests.pair })
    if ($pairs.Count -ne 1 -or $pairs[0].scenarioId -ne '8ac900fea19642cc0000') { throw 'Scenario selection produced sibling captures.' }
    foreach ($reference in $siblingReferences) {
        if ((Get-FileHash $reference.Path).Hash -ne $reference.Hash) { throw 'A sibling baseline was changed.' }
    }
    Select-Theme 'Dark'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'scenario-log-dark-compact'
    Invoke-Control 'Back to Activity'
    Wait-Control 'Passed: 1' | Out-Null
    Capture-Window 'scenario-completed-dark-compact'
    Invoke-Control 'View run log: Checkout'
    Wait-SourceText 'Clicking element: #continue' 'Scenario run log'
    Invoke-Control 'All run logs'
    Wait-Control 'Run log' | Out-Null
    Invoke-Control 'Back to Activity'
    Invoke-Control 'Open Report'
    Wait-Control 'Approve This Run' | Out-Null
    Capture-Window 'scenario-results-dark-compact'
    Invoke-Control 'Approve This Run'
    Wait-Control 'Scenarios: smoke / Checkout' | Out-Null
    Invoke-Control 'Approve Results' -Within 'Run confirmation'
    Wait-Control 'Reference images updated' | Out-Null
    Invoke-Control 'Compare This Run'
    Wait-Control '1 passed' | Out-Null
    Invoke-Control 'View run log: Checkout'
    Wait-SourceText 'Comparison passed:' 'Scenario run log'
    Capture-Window 'scenario-comparison-log'
    foreach ($reference in $siblingReferences) {
        if ((Get-FileHash $reference.Path).Hash -ne $reference.Hash) { throw 'Scenario approval changed a sibling baseline.' }
    }
    Write-Output 'Scenario-only capture, attributed live logs, scoped approval/comparison, and sibling baseline checks passed.'
}

$cleanupSnapshot = $null
try {
    Write-JsonFixture $settingsPath @{ theme = 'light'; sourceFontSize = 14; sourceWrap = $true; sourceLineNumbers = $true; navigatorHidden = $false; recentWorkspaces = @() }
    Start-Fixture
    Wait-Control ($workspace.Replace('\', '/')) | Out-Null
    if ($UpdateFlows) {
        foreach ($theme in @('Light', 'Dark')) {
            Select-Theme $theme
            [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
            Invoke-Control 'Software updates' -Pointer
            Wait-Control 'Updates are unavailable in this build' | Out-Null
            if (Test-ControlInvokable 'Check for updates') { throw 'An unconfigured build must not make update requests.' }
            Capture-Window ('updates-' + $theme.ToLowerInvariant())
            [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
            Capture-Window ('updates-' + $theme.ToLowerInvariant() + '-compact')
            Invoke-Control 'Close'
        }
        Write-Output 'Native update dialog, unavailable-feed state and light/dark compact layouts passed.'
        return
    }
    if ($InstanceFlows) {
        Test-InstanceFlows
        return
    }
    if ($ArtifactFlows) {
        Test-ArtifactFlows
        return
    }
    if ($ScenarioFlows) {
        Test-ScenarioFlows
        return
    }
    Capture-Window 'overview'
    Invoke-Control 'Appearance: Light' -Pointer
    Wait-Control 'Dark' | Out-Null
    Capture-Window 'theme-menu'
    Invoke-Control 'Dark'
    Wait-Control 'Appearance: Dark' | Out-Null
    Wait-Settings { param($settings) $settings.theme -eq 'dark' }
    Capture-Window 'overview-dark'
    Send-FixtureKeys '^+p'
    Send-FixtureText 'light theme'
    Wait-Control 'Light theme' | Out-Null
    Capture-Window 'command-palette-dark'
    Send-FixtureKeys '{ENTER}'
    Wait-Control 'Appearance: Light' | Out-Null
    Wait-Settings { param($settings) $settings.theme -eq 'light' }
    Invoke-Control 'account.tests.json'
    Wait-Control 'Profile' | Out-Null
    if ($null -ne (Find-Control 'Checkout')) { throw 'Scenarios from an unselected suite leaked into the navigator.' }
    Capture-Window 'sidebar-account'
    Invoke-Control 'smoke.tests.json'
    Wait-Control 'Checkout' | Out-Null
    if ($null -ne (Find-Control 'Profile')) { throw 'The scenario pane did not switch with the selected suite.' }
    Invoke-Control 'Checkout'
    Wait-Control 'Document loaded' | Out-Null
    Capture-Window 'editor'
    Invoke-Control 'Add action' -Pointer
    Wait-Control 'goto' | Out-Null
    Capture-Window 'action-menu'
    Send-FixtureKeys '{ESC}'
    Send-FixtureKeys '^+f'
    Send-FixtureText 'viewportNames'
    Invoke-Control 'Choose viewports' -Pointer
    Wait-Control 'phone (390 x 844)' | Out-Null
    Capture-Window 'viewport-choices'
    Invoke-Control 'phone (390 x 844)'
    Invoke-Control 'Choose viewports' -Pointer
    Invoke-Control 'desktop (1280 x 720)'
    Send-FixtureKeys '^+f'
    Send-FixtureKeys '^a'
    Send-FixtureText 'needs'
    Invoke-Control 'Choose dependencies' -Pointer
    Wait-Control 'Signed-in account (signed-in-account)' | Out-Null
    Capture-Window 'dependency-choices'
    Send-FixtureKeys '{ESC}'
    Send-FixtureKeys '^+f'
    Send-FixtureKeys '^a{BACKSPACE}'
    Invoke-Control 'Source'
    Wait-Control 'Configuration source' | Out-Null
    Invoke-Control 'Configuration source' -Pointer
    Send-FixtureKeys '^{HOME}{RIGHT}{ENTER}'
    Send-FixtureText '"deb'
    Wait-Control '1 suggestion' | Out-Null
    Capture-Window 'source-completion'
    Send-FixtureKeys '{ENTER}'
    Wait-SourceText '"debug":'
    Send-FixtureText 'false'
    Wait-SourceText '"debug": false'
    Send-FixtureKeys '{ESC},^s'
    Wait-Control 'Saved' | Out-Null
    $savedSuite = Get-Content (Join-Path $workspace 'visual_tests/smoke.tests.json') -Raw | ConvertFrom-Json
    if ($savedSuite.debug -ne $false) { throw 'Native completion did not save the expected property.' }
    if ($savedSuite.scenarios[0].viewportNames -notcontains 'phone') { throw 'Guided viewport choice was not saved.' }
    Invoke-Control 'Larger source text'
    Invoke-Control 'Wrap'
    Invoke-Control 'Line numbers'
    Wait-Settings { param($settings) $settings.sourceFontSize -eq 15 -and $settings.sourceWrap -eq $false -and $settings.sourceLineNumbers -eq $false }
    Capture-Window 'source'
    Select-Theme 'Dark'
    Capture-Window 'source-dark'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'source-dark-compact'
    Select-Theme 'Light'
    Capture-Window 'source-compact'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    Invoke-Control 'Results'
    Wait-Control 'Current 640 x 420' | Out-Null
    Wait-Control 'Mismatch Checkout' | Out-Null
    Wait-Control 'Desktop / Full page' | Out-Null
    Wait-Control 'Edit suite' | Out-Null
    Capture-Window 'results'
    Select-Theme 'Dark'
    Capture-Window 'results-dark'
    Select-Theme 'Light'
    Invoke-Control 'Side by side'
    Wait-Control 'Reference 640 x 420' | Out-Null
    Capture-Window 'side-by-side'
    Select-Theme 'Dark'
    Capture-Window 'side-by-side-dark'
    Select-Theme 'Light'
    Invoke-Control 'Expand comparison'
    Invoke-Control 'Hide navigator'
    Wait-Control 'Preferences saved' | Out-Null
    Capture-Window 'comparison-focus'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'comparison-compact'
    Invoke-Control 'Actual size (100%)'
    Capture-Window 'comparison-actual-size'
    Invoke-Control 'Fit width'
    Invoke-Control 'Show navigator'
    Wait-Control 'Preferences saved' | Out-Null
    Invoke-Control 'Restore capture list'
    Invoke-Control 'Editor'
    Invoke-Control 'Fields'
    Wait-Control 'Checkout' | Out-Null
    Capture-Window 'editor-compact'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    Invoke-Control 'Results'
    Wait-Control 'Approve This Run' | Out-Null
    $reviewedImage = Join-Path $output 'bitmaps_test/smoke-run/checkout.png'
    Write-TestImage (Join-Path $output 'bitmaps_test/zz-newer-run/checkout.png') ([System.Drawing.Color]::OrangeRed)
    Invoke-Control 'Approve This Run'
    Wait-Control 'Run: smoke-run' | Out-Null
    Capture-Window 'approve-reviewed-run'
    Invoke-Control 'Approve Results' -Within 'Run confirmation'
    Wait-Control 'Reference images updated' | Out-Null
    if ((Get-FileHash (Join-Path $output 'bitmaps_reference/checkout.png')).Hash -ne (Get-FileHash $reviewedImage).Hash) {
        throw 'Report approval used the newer directory instead of the viewed run.'
    }
    Invoke-Control 'Compare This Run'
    Wait-Control '1 passed' | Out-Null
    Capture-Window 'reviewed-run-compared'
    Invoke-Control 'Runs'
    Invoke-Control 'Suites'
    Invoke-Control 'Select all'
    Capture-Window 'suite-selection'
    Invoke-Control 'Run settings'
    Wait-Control 'Run Test' | Out-Null
    Capture-Window 'run-settings'
    Select-Theme 'Dark'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'run-settings-dark-compact'
    Select-Theme 'Light'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    if ($Capture) {
        Invoke-Control 'Run Reference'
        Wait-Control 'Run confirmation' | Out-Null
        Capture-Window 'direct-reference-confirmation'
        Invoke-Control 'Run Reference' -Within 'Run confirmation'
        Wait-Control 'Capturing scenarios' | Out-Null
        Capture-Window 'live-capture'
        Wait-Control 'Completed' | Out-Null
        Wait-Control 'Passed: 3' | Out-Null
        Wait-Control '2 passed' | Out-Null
        $referenceReports = Get-ChildItem (Join-Path $workspace '.regressify') -Recurse -Filter report.json |
            Where-Object { $_.Directory.Name.StartsWith('desktop-') }
        $referencePairs = @($referenceReports | ForEach-Object { (Get-Content $_.FullName -Raw | ConvertFrom-Json).tests.pair })
        if ($referencePairs.Count -ne 3) { throw 'Expected three reference capture report entries.' }
        Invoke-Control 'Run Test'
        Wait-Control 'Capturing scenarios' | Out-Null
        Invoke-Control 'Stop'
        Wait-Control 'Run stopped' | Out-Null
        Capture-Window 'run-stopped'
        Invoke-Control 'Run Test'
        Wait-Control 'Capturing scenarios' | Out-Null
        Wait-Control 'Completed' | Out-Null
        Wait-Control 'Passed: 3' | Out-Null
        Capture-Window 'live-completed'
        Select-Theme 'Dark'
        Capture-Window 'live-completed-dark'
        Select-Theme 'System'
        Select-Theme 'Light'
        $capturedReports = Get-ChildItem (Join-Path $workspace '.regressify') -Recurse -Filter report.json |
            Where-Object { $_.Directory.Name.StartsWith('desktop-') }
        $capturedPairs = @($capturedReports | ForEach-Object { (Get-Content $_.FullName -Raw | ConvertFrom-Json).tests.pair })
        if ($capturedPairs.Count -ne 3) { throw 'Expected three current test report entries.' }
        foreach ($pair in $capturedPairs) {
            if ([string]::IsNullOrWhiteSpace($pair.scenarioLabel) -or [string]::IsNullOrWhiteSpace($pair.scenarioId)) {
                throw 'Capture reports are missing readable scenario metadata.'
            }
        }
        Invoke-Control 'Open Report'
        Wait-Control 'Approve This Run' | Out-Null
        Capture-Window 'completed-run-results'
        Invoke-Control 'Runs'
        $siblingReferences = @(Get-ChildItem (Join-Path $workspace '.regressify') -Recurse -Filter '*.png' |
            Where-Object { $_.Directory.Name -eq 'bitmaps_reference' -and -not $_.Name.StartsWith('8ac900fea19642cc0000') } |
            ForEach-Object { @{ Path = $_.FullName; Hash = (Get-FileHash $_.FullName).Hash } })
        Invoke-Control 'Run Test: Checkout'
        Wait-Control 'Capturing scenarios' | Out-Null
        Wait-Control 'Scenarios: smoke / Checkout' | Out-Null
        Capture-Window 'scenario-only-running'
        Wait-Control 'Completed' | Out-Null
        Wait-Control 'Passed: 1' | Out-Null
        $scenarioReports = Get-ChildItem (Join-Path $output 'bitmaps_test') -Recurse -Filter report.json |
            Where-Object { $_.Directory.Name.StartsWith('desktop-') }
        $scenarioPairs = @($scenarioReports | ForEach-Object { (Get-Content $_.FullName -Raw | ConvertFrom-Json).tests.pair })
        if ($scenarioPairs.Count -ne 1 -or $scenarioPairs[0].scenarioId -ne '8ac900fea19642cc0000') {
            throw 'Individual scenario execution captured an unselected sibling.'
        }
        foreach ($reference in $siblingReferences) {
            if ((Get-FileHash $reference.Path).Hash -ne $reference.Hash) { throw 'A scenario test changed a sibling reference.' }
        }
        Invoke-Control 'View log: Checkout (phone)'
        Wait-SourceText 'Clicking element: #continue' 'Scenario run log'
        $scenarioLog = Read-ControlText 'Scenario run log'
        if ($scenarioLog.Contains('Signed-in account') -or $scenarioLog.Contains('Executing scenario: Profile')) {
            throw 'Scenario log contains another scenario output.'
        }
        Capture-Window 'scenario-log'
        Select-Theme 'Dark'
        [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
        Capture-Window 'scenario-log-dark-compact'
        Select-Theme 'Light'
        [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
        Invoke-Control 'Back to Activity'
        Invoke-Control 'View run log: Checkout'
        Wait-SourceText 'Clicking element: #continue' 'Scenario run log'
        Invoke-Control 'All run logs'
        Wait-Control 'Run log' | Out-Null
        Invoke-Control 'Back to Activity'
    }
    Invoke-Control 'More actions' -Pointer
    Invoke-Control 'Initialize Workspace'
    Wait-Control 'Workspace initialized' | Out-Null
    Capture-Window 'run-completed'
    if (-not (Test-Path (Join-Path $workspace 'common/_override.css'))) { throw 'GUI init command did not create starter files.' }
    Select-Theme 'Dark'
    $process.CloseMainWindow() | Out-Null
    if (-not $process.WaitForExit(10000)) { throw 'The clean fixture did not close after saving appearance.' }
    Start-Fixture
    Wait-Control 'Appearance: Dark' | Out-Null
    Invoke-Control 'smoke.tests.json'
    Invoke-Control 'Checkout'
    Invoke-Control 'Source'
    Wait-Control '15 px' | Out-Null
    Wait-Settings { param($settings) $settings.theme -eq 'dark' -and $settings.sourceFontSize -eq 15 -and $settings.sourceWrap -eq $false -and $settings.sourceLineNumbers -eq $false }
    Capture-Window 'appearance-restored'
    if (-not $KeepOpen) {
        $cleanupPath = Join-Path $workspace 'visual_tests/smoke.tests.json'
        $cleanupSnapshot = (Get-FileHash $cleanupPath -Algorithm SHA256).Hash
        Invoke-Control 'Editor'
        Invoke-Control 'Source'
        Invoke-Control 'Configuration source' -Pointer
        Send-FixtureKeys '^{END}'
        Send-FixtureText 'smoke-cleanup-probe'
        Wait-SourceText 'smoke-cleanup-probe'
    }
} finally {
    try {
    $cleanupProcess = if ($null -ne $process) { $process.Id } else { 0 }
    foreach ($fixture in $script:fixtureProcesses) {
    if (-not $KeepOpen -and -not $fixture.HasExited) {
        $script:process = $fixture
        $process.Refresh()
        $script:window = $process.MainWindowHandle
        $verifyDraft = $null -ne $cleanupSnapshot -and $process.Id -eq $cleanupProcess
        $process.CloseMainWindow() | Out-Null
        $closing = [System.Diagnostics.Stopwatch]::StartNew()
        $discarded = $false
        do {
            if ($process.HasExited) { break }
            try {
                if (-not $discarded) {
                    $discard = Find-Control 'Discard'
                    if ($null -ne $discard -and $discard.Current.IsEnabled) {
                        $invoke = $null
                        if ($discard.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$invoke)) {
                            ([System.Windows.Automation.InvokePattern]$invoke).Invoke()
                            $discarded = $true
                            Write-Output 'Discarded unsaved smoke-test draft.'
                        }
                    }
                }
            } catch {
                if ($process.WaitForExit(500)) { break }
                throw
            }
            [RegressifySmokeNative]::DwmFlush() | Out-Null
        } while ($closing.Elapsed.TotalSeconds -lt 10)
        if (-not $process.HasExited) {
            Write-Warning 'The synthetic test process did not close normally; terminating it.'
            $process.Kill($true)
            $process.WaitForExit()
            if ($verifyDraft) { throw 'Native draft cleanup did not close the fixture normally.' }
        }
        if ($verifyDraft) {
            if (-not $discarded) { throw 'The unsaved fixture draft was not discarded through its confirmation dialog.' }
            if ((Get-FileHash $cleanupPath -Algorithm SHA256).Hash -ne $cleanupSnapshot) {
                throw 'Cleanup unexpectedly changed the saved fixture source.'
            }
            Write-Output 'Dirty fixture closed without changing its saved source.'
        }
    }
    }
    } finally {
        if ($settingsExisted) {
            Copy-Item -LiteralPath $settingsBackup -Destination $settingsPath -Force
            if ((Get-FileHash -LiteralPath $settingsBackup).Hash -ne (Get-FileHash -LiteralPath $settingsPath).Hash) { throw 'Original desktop settings were not restored.' }
        } elseif (Test-Path -LiteralPath $settingsPath) {
            Remove-Item -LiteralPath $settingsPath
        }
        Write-Output 'Original desktop preferences restored.'
    }
}
Write-Output "Native smoke test passed. Fixture: $workspace"