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
    [switch]$Capture
)

$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$artifacts = Join-Path $repository 'target/desktop-smoke'
$workspace = Join-Path $artifacts ('workspace-' + [guid]::NewGuid().ToString('N'))
$executable = Join-Path $repository 'target/debug/regressify-desktop.exe'

if (-not $IsWindows) { throw 'This smoke test requires Windows.' }
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
    if (-not (New-Object -ComObject WScript.Shell).AppActivate([int]$process.Id) -or
        [RegressifySmokeFocus]::GetForegroundWindow() -ne $script:window) {
        throw 'Cannot focus the native fixture. Keep Windows unlocked and the interactive desktop available.'
    }
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
$process = Start-Process -FilePath $executable -ArgumentList ('"' + $workspace + '"') -RedirectStandardError $stderr -RedirectStandardOutput $stdout -PassThru
$script:window = [IntPtr]::Zero

function Find-Control([string]$Name) {
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($script:window)
    $condition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, $Name)
    $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}

function Wait-Control([string]$Name) {
    $deadline = [System.Diagnostics.Stopwatch]::StartNew()
    $stable = [System.Diagnostics.Stopwatch]::StartNew()
    $previousBounds = $null
    do {
        $element = Find-Control $Name
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
    throw "Native control '$Name' was not found."
}

function Invoke-Control([string]$Name, [switch]$Pointer) {
    $element = Wait-Control $Name
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

function Wait-SourceText([string]$Text) {
    $deadline = [System.Diagnostics.Stopwatch]::StartNew()
    do {
        $element = Wait-Control 'Configuration source'
        $pattern = $null
        if ($element.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
            $value = ([System.Windows.Automation.ValuePattern]$pattern).Current.Value
        } elseif ($element.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$pattern)) {
            $value = ([System.Windows.Automation.TextPattern]$pattern).DocumentRange.GetText(-1)
        } else { throw 'Source editor does not expose accessible text.' }
        if ($value.Contains($Text)) { return }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($deadline.Elapsed.TotalSeconds -lt 10)
    throw "Source editor did not contain '$Text'."
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

$cleanupSnapshot = $null
try {
    if (-not $process.WaitForInputIdle(15000)) { throw 'Desktop did not initialize.' }
    $initialization = [System.Diagnostics.Stopwatch]::StartNew()
    do {
        $process.Refresh()
        $script:window = $process.MainWindowHandle
        if ($script:window -ne [IntPtr]::Zero -or $process.HasExited) { break }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($initialization.Elapsed.TotalSeconds -lt 15)
    if ($script:window -eq [IntPtr]::Zero) { throw "Desktop has no native window. $(Get-Content $stderr -Raw)" }
    [RegressifySmokeNative]::SetForegroundWindow($script:window) | Out-Null
    $screen = [System.Windows.Forms.Screen]::FromHandle($script:window).WorkingArea
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    Wait-Control 'Workspace opened' | Out-Null
    Wait-Control ($workspace.Replace('\', '/')) | Out-Null
    Capture-Window 'overview'
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
    Capture-Window 'source'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'source-compact'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    Invoke-Control 'Results'
    Wait-Control 'Current 640 x 420' | Out-Null
    Wait-Control 'Mismatch Checkout' | Out-Null
    Wait-Control 'Desktop / Full page' | Out-Null
    Wait-Control 'Edit suite' | Out-Null
    Capture-Window 'results'
    Invoke-Control 'Side by side'
    Wait-Control 'Reference 640 x 420' | Out-Null
    Capture-Window 'side-by-side'
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
    Invoke-Control 'Runs'
    Invoke-Control 'Suites'
    Invoke-Control 'Select all'
    Capture-Window 'suite-selection'
    if ($Capture) {
        Invoke-Control 'Test'
        Invoke-Control 'Capture baseline'
        Invoke-Control 'Run'
        Invoke-Control 'Confirm'
        Wait-Control 'Capturing scenarios' | Out-Null
        Capture-Window 'live-capture'
        Wait-Control 'Completed' | Out-Null
        Wait-Control 'Passed: 3' | Out-Null
        Wait-Control '2 passed' | Out-Null
        Capture-Window 'live-completed'
        $capturedReports = Get-ChildItem (Join-Path $workspace '.regressify') -Recurse -Filter report.json |
            Where-Object { $_.Directory.Name.StartsWith('desktop-') }
        $capturedPairs = @($capturedReports | ForEach-Object { (Get-Content $_.FullName -Raw | ConvertFrom-Json).tests.pair })
        if ($capturedPairs.Count -ne 3) { throw 'Expected three captured report entries.' }
        foreach ($pair in $capturedPairs) {
            if ([string]::IsNullOrWhiteSpace($pair.scenarioLabel) -or [string]::IsNullOrWhiteSpace($pair.scenarioId)) {
                throw 'Capture reports are missing readable scenario metadata.'
            }
        }
        Invoke-Control 'Capture baseline'
    } else { Invoke-Control 'Test' }
    Invoke-Control 'Initialize workspace'
    Invoke-Control 'Run'
    Wait-Control 'Completed' | Out-Null
    Capture-Window 'run-completed'
    if (-not (Test-Path (Join-Path $workspace 'common/_override.css'))) { throw 'GUI init command did not create starter files.' }
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
    if (-not $KeepOpen -and -not $process.HasExited) {
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
            if ($null -ne $cleanupSnapshot) { throw 'Native draft cleanup did not close the fixture normally.' }
        }
        if ($null -ne $cleanupSnapshot) {
            if (-not $discarded) { throw 'The unsaved fixture draft was not discarded through its confirmation dialog.' }
            if ((Get-FileHash $cleanupPath -Algorithm SHA256).Hash -ne $cleanupSnapshot) {
                throw 'Cleanup unexpectedly changed the saved fixture source.'
            }
            Write-Output 'Dirty fixture closed without changing its saved source.'
        }
    }
}
Write-Output "Native smoke test passed. Fixture: $workspace"