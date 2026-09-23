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

function Send-FixtureText([string]$Text) {
    Focus-Fixture
    [RegressifySmokeText]::Type($Text)
    [RegressifySmokeNative]::DwmFlush() | Out-Null
}

function Send-FixtureKeys([ConsoleKey]$Key, [switch]$Control, [switch]$Shift) {
    Focus-Fixture
    [RegressifySmokeText]::Press($Key, $Control.IsPresent, $Shift.IsPresent)
    [RegressifySmokeNative]::DwmFlush() | Out-Null
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

function Wait-Control([string]$Name, [string]$Within = '', [int]$TimeoutSeconds = 60) {
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
    } while ($deadline.Elapsed.TotalSeconds -lt $TimeoutSeconds)
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

function Test-MenuActionEnabled([string]$Name) {
    $element = Wait-Control $Name
    $element.Current.ControlType -eq [System.Windows.Automation.ControlType]::MenuItem -and $element.Current.IsEnabled
}

function Invoke-Control([string]$Name, [switch]$Pointer, [string]$Within = '') {
    $element = Wait-Control $Name $Within
    $pattern = $null
    if (-not $Pointer -and $element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        ([System.Windows.Automation.InvokePattern]$pattern).Invoke()
    } elseif (-not $Pointer -and $element.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) {
        ([System.Windows.Automation.SelectionItemPattern]$pattern).Select()
    } elseif (-not $Pointer -and $element.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) {
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

function Hover-Control([string]$Name) {
    $bounds = (Wait-Control $Name).Current.BoundingRectangle
    Focus-Fixture
    if (-not [RegressifySmokePointer]::SetPhysicalCursorPos([int]($bounds.Left + $bounds.Width / 2), [int]($bounds.Top + $bounds.Height / 2))) {
        throw "Cannot hover '$Name'."
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

function Write-FixtureControlText([string]$Name, [string]$Text) {
    Invoke-Control $Name -Pointer
    Wait-Control $Name | Out-Null
    Send-FixtureKeys A -Control
    Send-FixtureText $Text
    try { Wait-SourceText $Text $Name } catch {
        Capture-Window 'text-input-failure'
        throw
    }
}

function Test-ApplicationMenus {
    $metadata = & cargo metadata --no-deps --format-version 1 --manifest-path (Join-Path $repository 'Cargo.toml')
    if ($LASTEXITCODE -ne 0) { throw 'Cannot determine the expected desktop version.' }
    $version = (($metadata | ConvertFrom-Json).packages | Where-Object name -eq 'regressify').version
    $versionBounds = (Wait-Control "v$version").Current.BoundingRectangle
    $fileBounds = (Wait-Control 'File').Current.BoundingRectangle
    $statusBounds = (Wait-Control 'Application status').Current.BoundingRectangle
    if ([Math]::Abs($versionBounds.Top - $fileBounds.Top) -gt 20 -or [Math]::Abs($statusBounds.Top - $fileBounds.Top) -gt 20) {
        throw 'Version and application status must be in the title bar, not a bottom status bar.'
    }
    Invoke-Control 'File' -Pointer
    foreach ($name in @('Open in VS Code', 'Open in File Explorer')) {
        if (-not (Test-MenuActionEnabled $name)) { throw "Workspace action '$name' is unavailable." }
    }
    Capture-Window 'file-external-workspace-actions'
    Send-FixtureKeys Escape
    Invoke-Control 'Help' -Pointer
    Invoke-Control 'About' -Pointer
    Wait-Control 'Regressify Desktop' | Out-Null
    Wait-Control "Version: $version" | Out-Null
    Wait-Control 'Author: Tuyen Pham' | Out-Null
    if (-not (Test-ControlInvokable 'https://tuyen.blog/')) { throw 'About website must be an external link.' }
    Capture-Window 'help-about'
    Invoke-Control 'Close'
    Wait-WindowState { $null -eq (Find-Control "Version: $version") } 'About dialog did not close.'
}
