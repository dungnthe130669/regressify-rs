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
        [ordered]@{ label = 'Checkout'; url = 'https://example.com/checkout'; actions = @(@{ click = '#continue' }, @{ wait = 1000 }) },
        [ordered]@{ label = 'Signed-in account'; url = 'https://example.com/account'; actions = @() }
    )
})
Write-JsonFixture (Join-Path $workspace 'visual_tests/account.tests.json') ([ordered]@{
    scenarios = @([ordered]@{ label = 'Profile'; url = 'https://example.com/profile'; actions = @() })
})
[System.IO.File]::WriteAllText((Join-Path $workspace 'regressify.yaml'), "bypassCsp: true`npostInteractionWait: 1000`n")
if ($Capture) {
    $page = Join-Path $workspace 'fixture.html'
    [System.IO.File]::WriteAllText($page, '<!doctype html><html><head><title>QA fixture</title></head><body style="font:24px sans-serif;padding:48px;background:#f0f4f3"><h1>Checkout</h1><button id="continue">Continue</button><p>Order total: $42.00</p></body></html>')
    $url = [System.Uri]::new($page).AbsoluteUri
    Write-JsonFixture (Join-Path $workspace 'visual_tests/smoke.tests.json') ([ordered]@{
        scenarios = @(
            [ordered]@{ label = 'Checkout'; url = $url; actions = @(@{ wait = 5000 }, @{ click = '#continue' }) },
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
        @{ status = 'fail'; pair = @{ label = 'Checkout desktop'; reference = '../bitmaps_reference/checkout.png'; test = '../bitmaps_test/smoke-run/checkout.png'; diffImage = '../bitmaps_test/smoke-run/checkout_diff.png'; diff = @{ rawMisMatchPercentage = 4.2; misMatchPercentage = '4.20' } } },
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
    do {
        $element = Find-Control $Name
        if ($null -ne $element -and -not $element.Current.IsOffscreen) { return $element }
        if ($process.HasExited) { throw "Desktop exited with code $($process.ExitCode)." }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($deadline.Elapsed.TotalSeconds -lt 60)
    throw "Native control '$Name' was not found."
}

function Invoke-Control([string]$Name) {
    $element = Wait-Control $Name
    $pattern = $null
    if ($element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        ([System.Windows.Automation.InvokePattern]$pattern).Invoke()
    } elseif ($element.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) {
        ([System.Windows.Automation.SelectionItemPattern]$pattern).Select()
    } elseif ($element.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) {
        ([System.Windows.Automation.TogglePattern]$pattern).Toggle()
    } else {
        $bounds = $element.Current.BoundingRectangle
        [RegressifySmokeNative]::SetCursorPos([int]($bounds.Left + $bounds.Width / 2), [int]($bounds.Top + $bounds.Height / 2)) | Out-Null
        [RegressifySmokeNative]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
        [RegressifySmokeNative]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    }
    [RegressifySmokeNative]::DwmFlush() | Out-Null
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
    Capture-Window 'overview'
    Invoke-Control 'smoke.tests.json'
    Invoke-Control 'Checkout'
    Wait-Control 'Document loaded' | Out-Null
    Capture-Window 'editor'
    Invoke-Control 'Source'
    Wait-Control 'Configuration source' | Out-Null
    Capture-Window 'source'
    Invoke-Control 'Results'
    Wait-Control 'Current 640 x 420' | Out-Null
    Capture-Window 'results'
    Invoke-Control 'Side by side'
    Wait-Control 'Reference 640 x 420' | Out-Null
    Capture-Window 'side-by-side'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
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
        Capture-Window 'live-completed'
        Invoke-Control 'Capture baseline'
    } else { Invoke-Control 'Test' }
    Invoke-Control 'Initialize workspace'
    Invoke-Control 'Run'
    Wait-Control 'Completed' | Out-Null
    Capture-Window 'run-completed'
    if (-not (Test-Path (Join-Path $workspace 'common'))) { throw 'GUI init command did not create starter files.' }
    Write-Output "Native smoke test passed. Fixture: $workspace"
} finally {
    if (-not $KeepOpen -and -not $process.HasExited) {
        $process.CloseMainWindow() | Out-Null
        if (-not $process.WaitForExit(10000)) { $process.Kill($true) }
    }
}