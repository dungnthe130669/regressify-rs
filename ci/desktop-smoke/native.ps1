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

if (-not ('RegressifySmokeFocus' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RegressifySmokeFocus {
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}
'@
}

if (-not ('RegressifySmokePointer' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RegressifySmokePointer {
    [StructLayout(LayoutKind.Sequential)]
    public struct Point { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool SetPhysicalCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetPhysicalCursorPos(out Point point);
}
'@
}

if (-not ('RegressifySmokeClipboard' -as [type])) {
Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
public static class RegressifySmokeClipboard {
    [DllImport("user32.dll")] public static extern uint GetClipboardSequenceNumber();
}
'@
}

if (-not ('RegressifySmokeWindowState' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RegressifySmokeWindowState {
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr window);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern int GetSystemMetricsForDpi(int index, uint dpi);
    [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr window, int command);
}
'@
}

if (-not ('RegressifySmokeText' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class RegressifySmokeText {
    [StructLayout(LayoutKind.Sequential)]
    struct MouseInput { public int dx, dy; public uint data, flags, time; public UIntPtr extra; }
    [StructLayout(LayoutKind.Sequential)]
    public struct KeyInput { public ushort key, scan; public uint flags, time; public UIntPtr extra; }
    [StructLayout(LayoutKind.Explicit)]
    struct InputData { [FieldOffset(0)] public MouseInput mouse; [FieldOffset(0)] public KeyInput keyboard; }
    [StructLayout(LayoutKind.Sequential)]
    struct Input { public uint type; public InputData data; }
    [DllImport("user32.dll", SetLastError = true)]
    static extern uint SendInput(uint count, Input[] inputs, int size);
    public static KeyInput[] KeySequence(ConsoleKey key, bool control, bool shift, bool alt) {
        if (key == 0 || !Enum.IsDefined(typeof(ConsoleKey), key)) throw new ArgumentOutOfRangeException(nameof(key));
        var events = new List<KeyInput>();
        if (control) events.Add(new KeyInput { key = 0x11 });
        if (shift) events.Add(new KeyInput { key = 0x10 });
        if (alt) events.Add(new KeyInput { key = 0x12 });
        // Extended navigation keys must not be interpreted as their numpad variants.
        uint flags = (key >= ConsoleKey.PageUp && key <= ConsoleKey.DownArrow) || key == ConsoleKey.Insert || key == ConsoleKey.Delete ? 1u : 0u;
        events.Add(new KeyInput { key = (ushort)key, flags = flags });
        events.Add(new KeyInput { key = (ushort)key, flags = flags | 2 });
        if (alt) events.Add(new KeyInput { key = 0x12, flags = 2 });
        if (shift) events.Add(new KeyInput { key = 0x10, flags = 2 });
        if (control) events.Add(new KeyInput { key = 0x11, flags = 2 });
        return events.ToArray();
    }
    public static void Press(ConsoleKey key, bool control, bool shift, bool alt) {
        var inputs = Array.ConvertAll(KeySequence(key, control, shift, alt), keyboard =>
            new Input { type = 1, data = new InputData { keyboard = keyboard } });
        Send(inputs);
    }
    public static void Type(string text) {
        var inputs = new Input[text.Length * 2];
        for (var index = 0; index < text.Length; index++) {
            inputs[index * 2] = new Input { type = 1, data = new InputData { keyboard = new KeyInput { scan = text[index], flags = 4 } } };
            inputs[index * 2 + 1] = new Input { type = 1, data = new InputData { keyboard = new KeyInput { scan = text[index], flags = 6 } } };
        }
        Send(inputs);
    }
    static void Send(Input[] inputs) {
        if (inputs.Length == 0 || SendInput((uint)inputs.Length, inputs, Marshal.SizeOf<Input>()) == inputs.Length) return;
        var error = Marshal.GetLastWin32Error();
        var releases = Array.FindAll(inputs, input => (input.data.keyboard.flags & 2) != 0);
        SendInput((uint)releases.Length, releases, Marshal.SizeOf<Input>());
        throw new Win32Exception(error);
    }
}
'@
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

function Wait-WindowState([scriptblock]$Match, [string]$Failure) {
    $deadline = [System.Diagnostics.Stopwatch]::StartNew()
    do {
        if (& $Match) { return }
        if ($process.HasExited) { throw 'The titlebar fixture exited unexpectedly.' }
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($deadline.Elapsed.TotalSeconds -lt 5)
    throw $Failure
}

function Invoke-WindowButton([ValidateSet('Minimize', 'Maximize', 'Close')][string]$Button) {
    Focus-Fixture
    $bounds = [RegressifySmokeNative+Rect]::new()
    if (-not [RegressifySmokeNative]::GetWindowRect($script:window, [ref]$bounds)) { throw 'Cannot read fixture bounds.' }
    $dpi = [RegressifySmokeWindowState]::GetDpiForWindow($script:window)
    $scale = $dpi / 96.0
    $border = if ([RegressifySmokeWindowState]::IsZoomed($script:window)) { 0 } else {
        [RegressifySmokeWindowState]::GetSystemMetricsForDpi(32, $dpi) + [RegressifySmokeWindowState]::GetSystemMetricsForDpi(92, $dpi)
    }
    $offset = switch ($Button) { 'Close' { 0.5 }; 'Maximize' { 1.5 }; 'Minimize' { 2.5 } }
    $horizontal = [int]($bounds.Right - $border - 34 * $scale * $offset)
    $vertical = [int]($bounds.Top + 19 * $scale)
    if (-not [RegressifySmokePointer]::SetPhysicalCursorPos($horizontal, $vertical)) { throw "Cannot reach $Button control." }
    [RegressifySmokeNative]::DwmFlush() | Out-Null
    [RegressifySmokeNative]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
    [RegressifySmokeNative]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
}
