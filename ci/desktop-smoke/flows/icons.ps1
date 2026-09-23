function Test-WindowIcons {
    if (-not ('RegressifySmokeIcons' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RegressifySmokeIcons {
    [DllImport("user32.dll", EntryPoint = "SendMessageTimeoutW", SetLastError = true)]
    static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr parameter, IntPtr data, uint flags, uint timeout, out UIntPtr result);
    [DllImport("user32.dll", EntryPoint = "GetClassLongPtrW")]
    static extern IntPtr GetClassLongPtr(IntPtr window, int index);
    [DllImport("kernel32.dll", EntryPoint = "LoadLibraryExW", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr LoadLibrary(string path, IntPtr file, uint flags);
    [DllImport("kernel32.dll")] public static extern bool FreeLibrary(IntPtr module);
    [DllImport("user32.dll", EntryPoint = "LoadImageW", SetLastError = true)]
    public static extern IntPtr LoadIcon(IntPtr module, IntPtr name, uint type, int width, int height, uint flags);
    [DllImport("user32.dll")] public static extern bool DestroyIcon(IntPtr icon);
    public static IntPtr WindowIcon(IntPtr window, bool small) {
        UIntPtr result;
        if (SendMessageTimeout(window, 0x7f, new UIntPtr(small ? 0u : 1u), IntPtr.Zero, 2, 2000, out result) == IntPtr.Zero)
            throw new InvalidOperationException("Window did not respond to WM_GETICON.");
        if (result != UIntPtr.Zero) return (IntPtr)result;
        if (small) {
            SendMessageTimeout(window, 0x7f, new UIntPtr(2), IntPtr.Zero, 2, 2000, out result);
            if (result != UIntPtr.Zero) return (IntPtr)result;
            var icon = GetClassLongPtr(window, -34);
            if (icon != IntPtr.Zero) return icon;
        }
        return GetClassLongPtr(window, -14);
    }
}
'@
    }
    $module = [RegressifySmokeIcons]::LoadLibrary($executable, [IntPtr]::Zero, 0x22)
    if ($module -eq [IntPtr]::Zero) { throw 'Cannot inspect desktop icon resources.' }
    try {
        foreach ($small in @($false, $true)) {
            $label = if ($small) { 'small' } else { 'large' }
            $handle = [RegressifySmokeIcons]::WindowIcon($script:window, $small)
            if ($handle -eq [IntPtr]::Zero) { throw "The live desktop window has no $label application icon." }
            $icon = [System.Drawing.Icon]::FromHandle($handle)
            $bitmap = $icon.ToBitmap()
            $expectedHandle = [RegressifySmokeIcons]::LoadIcon($module, [IntPtr]1, 1, $bitmap.Width, $bitmap.Height, 0)
            if ($expectedHandle -eq [IntPtr]::Zero) { $bitmap.Dispose(); $icon.Dispose(); throw 'Desktop icon resource 1 is missing.' }
            $expectedIcon = [System.Drawing.Icon]::FromHandle($expectedHandle)
            $expected = $expectedIcon.ToBitmap()
            try {
                for ($row = 0; $row -lt $bitmap.Height; $row++) {
                    for ($column = 0; $column -lt $bitmap.Width; $column++) {
                        if ($bitmap.GetPixel($column, $row).ToArgb() -ne $expected.GetPixel($column, $row).ToArgb()) {
                            throw "The live window's $label icon differs from the executable's icon."
                        }
                    }
                }
                $bitmap.Save((Join-Path $artifacts "window-icon-$label.png"), [System.Drawing.Imaging.ImageFormat]::Png)
                Write-Output "Live $label window icon matches embedded artwork ($($bitmap.Width) px)."
            } finally {
                $bitmap.Dispose()
                $expected.Dispose()
                $icon.Dispose()
                $expectedIcon.Dispose()
                [RegressifySmokeIcons]::DestroyIcon($expectedHandle) | Out-Null
            }
        }
    } finally { [RegressifySmokeIcons]::FreeLibrary($module) | Out-Null }
}

function Test-BrandingIcons {
    function Test-IconPixels([string]$Name, $Bounds, [switch]$WindowCapture) {
        if ($Bounds.Width -le 0 -or $Bounds.Height -le 0) { return $false }
        $bitmap = [System.Drawing.Bitmap]::new([int]$Bounds.Width, [int]$Bounds.Height)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            if ($WindowCapture) {
                $rectangle = [RegressifySmokeNative+Rect]::new()
                if (-not [RegressifySmokeNative]::GetWindowRect($script:window, [ref]$rectangle)) { throw 'Cannot locate the fixture window.' }
                $surface = [System.Drawing.Bitmap]::new($rectangle.Right - $rectangle.Left, $rectangle.Bottom - $rectangle.Top)
                $surfaceGraphics = [System.Drawing.Graphics]::FromImage($surface)
                try {
                    $deviceContext = $surfaceGraphics.GetHdc()
                    try {
                        if (-not [RegressifyWindowCapture]::PrintWindow($script:window, $deviceContext, 2)) { throw 'Cannot capture the title-bar logo.' }
                    } finally { $surfaceGraphics.ReleaseHdc($deviceContext) }
                    $source = [System.Drawing.Rectangle]::new([int]$Bounds.X - $rectangle.Left, [int]$Bounds.Y - $rectangle.Top, $bitmap.Width, $bitmap.Height)
                    $destination = [System.Drawing.Rectangle]::new(0, 0, $bitmap.Width, $bitmap.Height)
                    $graphics.DrawImage($surface, $destination, $source, [System.Drawing.GraphicsUnit]::Pixel)
                } finally { $surfaceGraphics.Dispose(); $surface.Dispose() }
            } else {
                $graphics.CopyFromScreen([int]$Bounds.X, [int]$Bounds.Y, 0, 0, $bitmap.Size)
            }
            $magenta = 0
            for ($row = 0; $row -lt $bitmap.Height; $row++) {
                for ($column = 0; $column -lt $bitmap.Width; $column++) {
                    $pixel = $bitmap.GetPixel($column, $row)
                    if ($pixel.R -gt 200 -and $pixel.G -lt 80 -and $pixel.B -gt 200) { $magenta++ }
                }
            }
            $bitmap.Save((Join-Path $artifacts "$Name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
            if ($magenta -lt 5) { return $false }
            Write-Host "$Name contains the rendered multicolor logo ($magenta magenta pixels)."
            return $true
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
    }
    [RegressifySmokeNative]::DwmFlush() | Out-Null
    $deadline = [System.Diagnostics.Stopwatch]::StartNew()
    do {
        $logo = Wait-Control 'Regressify logo'
        if (Test-IconPixels 'titlebar-logo' $logo.Current.BoundingRectangle -WindowCapture) { break }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
        if ($deadline.Elapsed.TotalSeconds -gt 10) { throw 'The title-bar logo did not render its magenta artwork.' }
    } while ($true)
    $taskbar = [System.Windows.Automation.AutomationElement]::RootElement.FindFirst(
        [System.Windows.Automation.TreeScope]::Children,
        [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ClassNameProperty, 'Shell_TrayWnd'))
    if ($null -eq $taskbar) { throw 'The Windows taskbar is unavailable for native icon verification.' }
    $deadline.Restart()
    $observed = $false
    do {
        $controls = $taskbar.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
        foreach ($control in $controls) {
            if ($control.Current.AutomationId -like 'Appid: Regressify.Desktop*' -and $control.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button) {
                if (-not $observed) {
                    Write-Output "Checking taskbar button: $($control.Current.Name) [$($control.Current.AutomationId)]"
                    $observed = $true
                }
                if (Test-IconPixels 'taskbar-logo' $control.Current.BoundingRectangle) { return }
            }
        }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($deadline.Elapsed.TotalSeconds -lt 10)
    if (-not $observed) { throw 'The running application taskbar button was not found.' }
    throw 'The running taskbar button did not show the application logo; see taskbar-logo.png.'
}
