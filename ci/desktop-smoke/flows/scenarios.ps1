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
    Invoke-Control 'Checkout'
    Invoke-Control 'Runs'
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
    $reportHashes = @($reports | ForEach-Object { @{ Path = $_.FullName; Hash = (Get-FileHash $_.FullName).Hash } })
    $process.CloseMainWindow() | Out-Null
    if (-not $process.WaitForExit(10000)) { throw 'Completed scenario fixture did not close cleanly.' }
    Start-Fixture
    Invoke-Control 'Runs'
    Wait-Control 'Last run: Run Test' | Out-Null
    Wait-Control 'Scenarios: smoke / Checkout' | Out-Null
    Wait-Control 'Passed: 1' | Out-Null
    Wait-Control 'Failed: 0' | Out-Null
    Wait-Control 'Completed' | Out-Null
    Capture-Window 'scenario-activity-after-restart'
    foreach ($report in $reportHashes) {
        if ((Get-FileHash $report.Path).Hash -ne $report.Hash) { throw 'Restoring activity modified a saved report.' }
    }
    Invoke-Control 'smoke.tests.json'
    Invoke-Control 'Checkout'
    Invoke-Control 'Runs'
    Invoke-Control 'View run log: Checkout'
    Wait-SourceText 'Clicking element: #continue' 'Scenario run log'
    Invoke-Control 'All run logs'
    Wait-Control 'Run log' | Out-Null
    Invoke-Control 'Back to Activity'
    Invoke-Control 'Open Report'
    Wait-Control 'Approve This Scenario' | Out-Null
    Capture-Window 'scenario-results-dark-compact'
    Invoke-Control 'Approve This Scenario'
    Wait-Control 'Scenarios: smoke / Checkout' | Out-Null
    Invoke-Control 'Approve This Scenario' -Within 'Run confirmation'
    Wait-Control 'Passed Checkout' | Out-Null
    Invoke-Control 'Runs'
    Invoke-Control 'Compare This Run'
    Wait-Control 'Last run: Compare Results' | Out-Null
    Wait-Control 'Completed' | Out-Null
    Wait-Control '1 passed' | Out-Null
    Invoke-Control 'View run log: Checkout'
    Wait-SourceText 'Comparison passed:' 'Scenario run log'
    Capture-Window 'scenario-comparison-log'
    foreach ($reference in $siblingReferences) {
        if ((Get-FileHash $reference.Path).Hash -ne $reference.Hash) { throw 'Scenario approval changed a sibling baseline.' }
    }
    Write-Output 'Scenario-only capture, persistent activity and logs after restart, scoped approval/comparison, and sibling baseline checks passed.'
}
