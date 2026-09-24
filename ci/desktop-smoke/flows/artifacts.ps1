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
    Wait-Control 'Test 640 x 420' | Out-Null
    Capture-Window 'artifacts-existing-report'
    if ((Get-FileHash $reportPath).Hash -ne $reportHash) { throw 'Opening an existing report rewrote its data.' }
    Invoke-Control 'Runs'
    Invoke-Control 'Activity'
    Wait-Control 'Saved comparison results' | Out-Null
    Wait-Control 'Screenshot comparisons: 2' | Out-Null
    Wait-Control 'Failed: 2' | Out-Null
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
    Wait-Control 'Test 640 x 420' | Out-Null
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
    Invoke-Control 'Runs'
    Wait-Control 'Last run: Compare Results' | Out-Null
    Wait-Control 'Passed: 1' | Out-Null
    Wait-Control 'Failed: 0' | Out-Null
    Wait-Control 'Completed' | Out-Null
    Capture-Window 'artifacts-activity-after-restart'
    Invoke-Control 'Editor'
    Invoke-Control 'account.tests.json'
    Invoke-Control 'Profile'
    Wait-Control '1 reference image / 1 saved test run' | Out-Null
    Invoke-Control 'Open Report'
    Wait-Control 'Reference 640 x 420' | Out-Null
    Wait-Control 'Test 640 x 420' | Out-Null
    Capture-Window 'artifacts-report-after-restart'
    Invoke-Control 'Runs'
    Wait-Control 'Last run: Compare Results' | Out-Null
    Wait-Control 'Passed: 1' | Out-Null
    Invoke-Control 'Editor'
    Invoke-Control 'account.tests.json' -Pointer
    Send-FixtureKeys F2
    Wait-Control 'Filename' | Out-Null
    Write-FixtureControlText 'Filename' 'customers.tests.json'
    Write-FixtureControlText 'Display name' 'Customer accounts'
    Capture-Window 'suite-rename-dialog'
    Invoke-Control 'Save'
    Wait-Control 'File saved' | Out-Null
    Wait-Control 'Customer accounts' | Out-Null
    $renamedOutput = Join-Path $workspace '.regressify/customers'
    if ((Test-Path $accountOutput) -or (Test-Path (Join-Path $workspace 'visual_tests/account.tests.json'))) { throw 'Suite rename retained old source/output paths.' }
    $renamedSource = Join-Path $workspace 'visual_tests/customers.tests.json'
    $renamedSuite = Get-Content $renamedSource -Raw | ConvertFrom-Json
    if ($renamedSuite.displayName -ne 'Customer accounts') { throw 'Rename did not save the independent display name.' }
    if ((Get-FileHash (Join-Path $renamedOutput 'bitmaps_reference/profile__FINAL.png')).Hash -ne $referenceHash) { throw 'Suite rename changed reference pixels.' }
    if ((Get-FileHash (Join-Path $renamedOutput 'bitmaps_test/smoke-run/profile__FINAL.png')).Hash -ne $captureHash) { throw 'Suite rename changed capture pixels.' }
    $renamedReport = Get-Content (Join-Path $renamedOutput 'bitmaps_test/smoke-run/report.json') -Raw | ConvertFrom-Json
    if ($renamedReport.id -ne 'customers' -or $renamedReport.displayName -ne 'Customer accounts') { throw 'Suite rename did not update report identity and label.' }
    Invoke-Control 'Fields'
    Send-FixtureKeys F -Control -Shift
    Send-FixtureText 'displayName'
    Write-FixtureControlText 'Display name' 'Customer account review'
    Send-FixtureKeys S -Control
    Wait-Control 'Saved' | Out-Null
    if (-not (Test-Path $renamedSource)) { throw 'Editing the display name renamed the suite file.' }
    Capture-Window 'suite-display-name-fields'
    Invoke-Control 'Customer account review' -Pointer
    Send-FixtureKeys Delete
    Wait-Control 'Delete suite and its data?' | Out-Null
    Capture-Window 'suite-delete-confirmation'
    Invoke-Control 'Cancel'
    if (-not (Test-Path $renamedOutput)) { throw 'Cancelling suite deletion removed its artifacts.' }
    Invoke-Control 'Customer account review' -Pointer
    Send-FixtureKeys Delete
    Invoke-Control 'Delete'
    Wait-Control 'Suite deleted' | Out-Null
    if ((Test-Path $renamedSource) -or (Test-Path $renamedOutput)) { throw 'Suite deletion did not remove its source and artifacts.' }
    if (-not (Test-Path $reportPath)) { throw 'Suite deletion removed another suite report.' }
    Write-Output 'Artifact gating, persistent activity, suite rename/display labels, and confirmed deletion passed without browser capture.'
}
