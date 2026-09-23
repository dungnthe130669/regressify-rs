function Test-TitlebarFlows {
    Invoke-Control 'Commands' -Pointer
    Wait-Control 'New suite' | Out-Null
    Capture-Window 'titlebar-command-pointer'
    Send-FixtureKeys Escape
    Invoke-Control 'Hide navigator' -Pointer
    Wait-Control 'Show navigator' | Out-Null
    Invoke-Control 'Show navigator' -Pointer
    Wait-Control 'Hide navigator' | Out-Null
    Invoke-Control 'Runs' -Pointer
    Wait-Control 'Run settings' | Out-Null
    Invoke-Control 'Results' -Pointer
    Wait-Control 'Approve This Scenario' | Out-Null
    Invoke-Control 'Editor' -Pointer
    Invoke-Control 'smoke.tests.json'
    Wait-Control 'Run Suite Reference' | Out-Null
    Invoke-Control 'Deploy' -Pointer
    Wait-Control 'Yes, deploy only these 1 of 2 suites' | Out-Null
    Capture-Window 'titlebar-deploy-pointer'
    Invoke-Control 'Close'
    $before = [RegressifySmokeNative+Rect]::new()
    [RegressifySmokeNative]::GetWindowRect($script:window, [ref]$before) | Out-Null
    $drag = (Wait-Control ($workspace.Replace('\', '/'))).Current.BoundingRectangle
    Focus-Fixture
    $horizontal = [int]($drag.Left + $drag.Width / 2)
    $vertical = [int]($drag.Top + $drag.Height / 2)
    [RegressifySmokePointer]::SetPhysicalCursorPos($horizontal, $vertical) | Out-Null
    [RegressifySmokeNative]::DwmFlush() | Out-Null
    [RegressifySmokeNative]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
    try {
        foreach ($step in 1..10) {
            [RegressifySmokePointer]::SetPhysicalCursorPos($horizontal + 6 * $step, $vertical + 4 * $step) | Out-Null
            [RegressifySmokeNative]::DwmFlush() | Out-Null
        }
    } finally {
        [RegressifySmokeNative]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    }
    Wait-WindowState {
        $after = [RegressifySmokeNative+Rect]::new()
        [RegressifySmokeNative]::GetWindowRect($script:window, [ref]$after) | Out-Null
        [Math]::Abs($after.Left - $before.Left) -ge 20 -and [Math]::Abs($after.Top - $before.Top) -ge 10
    } 'Dragging the workspace title did not move the window.'
    Capture-Window 'titlebar-dragged'
    Invoke-WindowButton 'Maximize'
    Wait-WindowState { [RegressifySmokeWindowState]::IsZoomed($script:window) } 'Native maximize button did not maximize the window.'
    Capture-Window 'titlebar-maximized'
    Invoke-WindowButton 'Maximize'
    Wait-WindowState { -not [RegressifySmokeWindowState]::IsZoomed($script:window) } 'Native restore button did not restore the window.'
    Capture-Window 'titlebar-restored'
    Invoke-WindowButton 'Minimize'
    Wait-WindowState { [RegressifySmokeWindowState]::IsIconic($script:window) } 'Native minimize button did not minimize the window.'
    [RegressifySmokeWindowState]::ShowWindowAsync($script:window, 9) | Out-Null
    Wait-WindowState { -not [RegressifySmokeWindowState]::IsIconic($script:window) } 'Minimized fixture did not restore.'
    Focus-Fixture
    Capture-Window 'titlebar-unminimized'
    Invoke-Control 'smoke.tests.json'
    Invoke-Control 'Checkout'
    Invoke-Control 'Source'
    Invoke-Control 'Configuration source' -Pointer
    $script:cleanupPath = Join-Path $workspace 'visual_tests/smoke.tests.json'
    $script:cleanupSnapshot = (Get-FileHash $script:cleanupPath -Algorithm SHA256).Hash
    Send-FixtureKeys End -Control
    Send-FixtureText 'titlebar-close-probe'
    Wait-SourceText 'titlebar-close-probe'
    Invoke-WindowButton 'Close'
    Wait-Control 'Discard' | Out-Null
    Wait-Control 'Cancel' | Out-Null
    Capture-Window 'titlebar-close-guard'
    Invoke-Control 'Cancel'
    if ($process.HasExited) { throw 'Cancelling titlebar close discarded the window.' }
    Wait-SourceText 'titlebar-close-probe'
    if ((Get-FileHash $script:cleanupPath -Algorithm SHA256).Hash -ne $script:cleanupSnapshot) { throw 'Titlebar close changed the saved source.' }
    Write-Output 'Native pointer controls, dragging, maximize/restore, minimize, and unsaved-close guard passed.'
}
