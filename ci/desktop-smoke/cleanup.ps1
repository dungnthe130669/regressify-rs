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
    if (-not $KeepOpen) {
        $activityName = [System.IO.Path]::GetFileName((Get-FixtureDeploymentSettingsPath))
        $activityPath = Join-Path (Split-Path -Parent $settingsPath) ('runs/' + $activityName)
        if (Test-Path -LiteralPath $activityPath) { Remove-Item -LiteralPath $activityPath }
    }
    if ($DeploymentFlows -and -not $KeepOpen) {
        $targetSettings = Get-FixtureDeploymentSettingsPath
        if (Test-Path -LiteralPath $targetSettings) {
            $targets = (Get-Content -LiteralPath $targetSettings -Raw | ConvertFrom-Json).targets
            foreach ($target in $targets) {
                if ($target.credentialId) { [RegressifySmokeCredentials]::Remove((Get-FixtureCredentialName $target.credentialId)) }
            }
            Remove-Item -LiteralPath $targetSettings
        }
    }
}
