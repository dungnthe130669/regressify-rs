function Test-UpdateFlows {
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
}
