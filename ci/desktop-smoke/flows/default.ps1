function Test-DefaultFlows {
    $detailsClipboard = Invoke-FixtureClipboard { [System.Windows.Forms.Clipboard]::GetDataObject() }
    try {
        foreach ($theme in @('Light', 'Dark')) {
            [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1600, 1000, 0x0040) | Out-Null
            Select-Theme $theme
            foreach ($dimensions in @(@(1600, 1000), @(1080, 720))) {
                [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, $dimensions[0], $dimensions[1], 0x0040) | Out-Null
                Capture-Window "overview-table-$($theme.ToLowerInvariant())-$($dimensions[0])"
                Invoke-Control 'Details: smoke' -Pointer
                Wait-Control 'Suite details text' | Out-Null
                $details = Read-ControlText 'Suite details text'
                if ($details -notmatch 'Suite:' -or $details -notmatch 'File: visual_tests/smoke.tests.json' -or $details -notmatch 'Scenarios: 2' -or $details -notmatch 'Status:') {
                    throw 'Suite details did not expose the complete fixture metadata.'
                }
                Invoke-Control 'Suite details text' -Pointer
                Invoke-FixtureClipboard { [System.Windows.Forms.Clipboard]::SetText('overview-details-sentinel') }
                Send-FixtureKeys A -Control
                Send-FixtureKeys C -Control
                Wait-WindowState { (Invoke-FixtureClipboard { [System.Windows.Forms.Clipboard]::GetText() }) -ceq $details } 'Selecting suite details did not copy its exact full text.'
                Capture-Window "overview-details-$($theme.ToLowerInvariant())-$($dimensions[0])"
                Invoke-FixtureClipboard { [System.Windows.Forms.Clipboard]::SetText('overview-details-sentinel') }
                Invoke-Control 'Copy suite details' -Pointer
                Wait-WindowState { (Invoke-FixtureClipboard { [System.Windows.Forms.Clipboard]::GetText() }) -ceq $details } 'Copy suite details did not preserve its exact full text.'
                Invoke-Control 'Close suite details' -Pointer
                Wait-WindowState { $null -eq (Find-Control 'Suite details text') } 'Suite details did not close.'
            }
        }
    } finally {
        Invoke-FixtureClipboard {
            if ($null -ne $detailsClipboard) { [System.Windows.Forms.Clipboard]::SetDataObject($detailsClipboard, $true) }
            else { [System.Windows.Forms.Clipboard]::Clear() }
        }
    }
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1600, 1000, 0x0040) | Out-Null
    Select-Theme 'Light'
    Capture-Window 'overview'
    Invoke-Control 'Appearance: Light' -Pointer
    Wait-Control 'Dark' | Out-Null
    Capture-Window 'theme-menu'
    Invoke-Control 'Dark'
    Wait-Control 'Appearance: Dark' | Out-Null
    Wait-Settings { param($settings) $settings.theme -eq 'dark' }
    Capture-Window 'overview-dark'
    Send-FixtureKeys P -Control -Shift
    Send-FixtureText 'light theme'
    Wait-Control 'Light theme' | Out-Null
    Capture-Window 'command-palette-dark'
    Send-FixtureKeys Enter
    Wait-Control 'Appearance: Light' | Out-Null
    Wait-Settings { param($settings) $settings.theme -eq 'light' }
    Invoke-Control 'account.tests.json'
    Wait-Control 'Profile' | Out-Null
    if ($null -ne (Find-Control 'Checkout')) { throw 'Scenarios from an unselected suite leaked into the navigator.' }
    Capture-Window 'sidebar-account'
    Invoke-Control 'smoke.tests.json'
    Wait-Control 'Checkout' | Out-Null
    if ($null -ne (Find-Control 'Profile')) { throw 'The scenario pane did not switch with the selected suite.' }
    Invoke-Control 'smoke.tests.json' -Pointer
    Send-FixtureKeys A -Control
    Invoke-Control 'Runs'
    Invoke-Control 'Suites'
    Write-Host 'Checking suite-list Ctrl+A'
    Wait-Control 'account (1/1 scenarios)' | Out-Null
    Wait-Control 'smoke (2/2 scenarios)' | Out-Null
    Write-Host 'Checking run-checklist selection shortcuts'
    Invoke-Control 'Select all' -Pointer
    Send-FixtureKeys A -Control
    Wait-Control 'account (1/1 scenarios)' | Out-Null
    Send-FixtureKeys A -Control -Shift
    Wait-Control 'account (0/1 scenarios)' | Out-Null
    Wait-Control 'smoke (0/2 scenarios)' | Out-Null
    Invoke-Control 'Editor'
    Invoke-Control 'Checkout' -Pointer
    Wait-Control 'Choose viewports' | Out-Null
    Send-FixtureKeys A -Control
    Invoke-Control 'Runs'
    Invoke-Control 'Suites'
    Write-Host 'Checking scenario-list Ctrl+A'
    Wait-Control 'smoke (2/2 scenarios)' | Out-Null
    Wait-Control 'account (0/1 scenarios)' | Out-Null
    Capture-Window 'keyboard-scenario-selection'
    Invoke-Control 'Editor'
    Invoke-Control 'Checkout' -Pointer
    Wait-Control 'Choose viewports' | Out-Null
    Send-FixtureKeys Escape
    Invoke-Control 'Runs'
    Invoke-Control 'Suites'
    Wait-Control 'smoke (0/2 scenarios)' | Out-Null
    Invoke-Control 'Editor'
    Invoke-Control 'Checkout' -Pointer
    Wait-Control 'Document loaded' | Out-Null
    Capture-Window 'editor'
    (Wait-Control 'Add' -Within 'Editor commands').SetFocus()
    Wait-WindowState { [System.Windows.Automation.AutomationElement]::FocusedElement.Current.Name -eq 'Add' } 'Editor Add did not receive native keyboard focus.'
    Send-FixtureKeys LeftArrow
    Wait-WindowState { [System.Windows.Automation.AutomationElement]::FocusedElement.Current.Name -eq 'More suite actions' } 'Editor toolbar Left did not wrap from Add to More suite actions.'
    Invoke-Control 'Add action' -Pointer
    Wait-Control 'goto' | Out-Null
    Capture-Window 'action-menu'
    Send-FixtureKeys Escape
    Send-FixtureKeys F -Control -Shift
    Send-FixtureText 'viewportNames'
    Invoke-Control 'Choose viewports' -Pointer
    try { Wait-Control 'phone (390 x 844)' | Out-Null } catch {
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($script:window)
        $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
            Where-Object { $_.Current.Name -match '^(Choose viewports|phone|desktop|Search|Viewport names)' -or $_.Current.ControlType -in @([System.Windows.Automation.ControlType]::ComboBox, [System.Windows.Automation.ControlType]::List, [System.Windows.Automation.ControlType]::ListItem) } |
            ForEach-Object { Write-Output ("Choice node: {0}, role={1}, offscreen={2}, bounds={3}" -f $_.Current.Name, $_.Current.ControlType.ProgrammaticName, $_.Current.IsOffscreen, $_.Current.BoundingRectangle) }
        throw
    }
    Capture-Window 'viewport-choices'
    Invoke-Control 'phone (390 x 844)' -Pointer
    Wait-Control 'desktop (1280 x 720)' | Out-Null
    Invoke-Control 'desktop (1280 x 720)' -Pointer
    Send-FixtureKeys Escape
    Send-FixtureKeys F -Control -Shift
    Send-FixtureKeys A -Control
    Send-FixtureText 'needs'
    Invoke-Control 'Choose dependencies' -Pointer
    Wait-Control 'Signed-in account (signed-in-account)' | Out-Null
    Capture-Window 'dependency-choices'
    Send-FixtureKeys Escape
    Send-FixtureKeys F -Control -Shift
    Send-FixtureKeys A -Control
    Send-FixtureKeys Backspace
    Invoke-Control 'Source'
    Wait-Control 'Configuration source' | Out-Null
    Invoke-Control 'Configuration source' -Pointer
    Send-FixtureKeys Home -Control
    Wait-Control 'Ln 1, Col 1' | Out-Null
    Send-FixtureKeys RightArrow
    Wait-Control 'Ln 1, Col 2' | Out-Null
    Send-FixtureKeys Enter
    Send-FixtureText '"deb'
    Wait-Control '1 suggestion' | Out-Null
    Capture-Window 'source-completion'
    Send-FixtureKeys Enter
    Wait-SourceText '"debug":'
    Send-FixtureText 'false'
    Wait-SourceText '"debug": false'
    Send-FixtureKeys Escape
    Send-FixtureText ','
    Send-FixtureKeys S -Control
    Wait-Control 'Saved' | Out-Null
    $savedSuite = Get-Content (Join-Path $workspace 'visual_tests/smoke.tests.json') -Raw | ConvertFrom-Json
    if ($savedSuite.debug -ne $false) { throw 'Native completion did not save the expected property.' }
    if ($savedSuite.scenarios[0].viewportNames -notcontains 'phone') { throw 'Guided viewport choice was not saved.' }
    Invoke-Control 'Larger source text'
    Invoke-Control 'Wrap'
    Invoke-Control 'Line numbers'
    Wait-Settings { param($settings) $settings.sourceFontSize -eq 15 -and $settings.sourceWrap -eq $false -and $settings.sourceLineNumbers -eq $false }
    Capture-Window 'source'
    Select-Theme 'Dark'
    Capture-Window 'source-dark'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'source-dark-compact'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    Select-Theme 'Light'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'source-compact'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    Invoke-Control 'Results'
    Wait-Control 'Test 640 x 420' | Out-Null
    Wait-Control 'Mismatch Checkout' | Out-Null
    Test-CaptureInformation 'results-scenario-information'
    Invoke-Control 'Capture Log' -Pointer
    Wait-Control 'Capture log commands' | Out-Null
    Invoke-Control 'Search log' -Within 'Log controls' -Pointer
    Wait-Control 'Search log' -Within 'Log controls' | Out-Null
    Send-FixtureText 'ab'
    Send-FixtureKeys LeftArrow
    Send-FixtureText 'X'
    Wait-WindowState { (Read-ControlText 'Search log') -ceq 'aXb' } 'Log toolbar stole the search input Left key.'
    Send-FixtureKeys RightArrow
    Send-FixtureText 'Y'
    Wait-WindowState { (Read-ControlText 'Search log') -ceq 'aXbY' } 'Log toolbar stole the search input Right key.'
    Send-FixtureKeys A -Control
    Send-FixtureKeys Backspace
    Wait-WindowState { (Read-ControlText 'Search log') -ceq '' } 'Log toolbar search did not clear.'
    Invoke-Control 'Reference' -Within 'Capture log commands' -Pointer
    Send-FixtureKeys RightArrow
    Wait-WindowState { [System.Windows.Automation.AutomationElement]::FocusedElement.Current.Name -eq 'Test' } 'Capture Log Right did not focus the Test source tab.'
    Send-FixtureKeys Enter
    Wait-WindowState {
        $tab = Find-Control 'Test' -Within 'Capture log commands'
        $pattern = $null
        $null -ne $tab -and $tab.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern) -and ([System.Windows.Automation.SelectionItemPattern]$pattern).Current.IsSelected
    } 'Capture Log Enter did not select the Test source.'
    Send-FixtureKeys LeftArrow
    Wait-WindowState { [System.Windows.Automation.AutomationElement]::FocusedElement.Current.Name -eq 'Reference' } 'Capture Log Left did not focus the Reference source tab.'
    Send-FixtureKeys Spacebar
    Wait-WindowState {
        $tab = Find-Control 'Reference' -Within 'Capture log commands'
        $pattern = $null
        $null -ne $tab -and $tab.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern) -and ([System.Windows.Automation.SelectionItemPattern]$pattern).Current.IsSelected
    } 'Capture Log Space did not select the Reference source.'
    Capture-Window 'capture-log-toolbar'
    Invoke-Control 'Back to comparison' -Pointer
    Wait-Control 'Edit suite' | Out-Null
    Capture-Window 'results'
    Select-Theme 'Dark'
    Capture-Window 'results-dark'
    Select-Theme 'Light'
    Invoke-Control 'Side by side'
    Wait-Control 'Reference 640 x 420' | Out-Null
    Capture-Window 'side-by-side'
    Select-Theme 'Dark'
    Capture-Window 'side-by-side-dark'
    Select-Theme 'Light'
    Invoke-Control 'Hide navigator'
    Wait-Control 'Preferences saved' | Out-Null
    Capture-Window 'comparison-focus'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'comparison-compact'
    Invoke-Control 'Actual size (100%)'
    Capture-Window 'comparison-actual-size'
    Invoke-Control 'Fit area'
    Invoke-Control 'Show navigator'
    Wait-Control 'Preferences saved' | Out-Null
    Invoke-Control 'Editor'
    Invoke-Control 'Fields'
    Wait-Control 'Checkout' | Out-Null
    Capture-Window 'editor-compact'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    Invoke-Control 'Results'
    Test-ReportApproval
    Invoke-Control 'Runs'
    Invoke-Control 'Suites'
    Invoke-Control 'Select all'
    Capture-Window 'suite-selection'
    Invoke-Control 'Run settings'
    Wait-Control 'Run Test' | Out-Null
    Capture-Window 'run-settings'
    Select-Theme 'Dark'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'run-settings-dark-compact'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
    Select-Theme 'Light'
    if ($Capture) {
        Invoke-Control 'Run Reference'
        Wait-Control 'Run confirmation' | Out-Null
        Capture-Window 'direct-reference-confirmation'
        Invoke-Control 'Run Reference' -Within 'Run confirmation'
        Wait-Control 'Capturing scenarios' | Out-Null
        Capture-Window 'live-capture'
        Wait-Control 'Completed' | Out-Null
        Wait-Control 'Passed: 3' | Out-Null
        Wait-Control '2 passed' | Out-Null
        $referenceReports = Get-ChildItem (Join-Path $workspace '.regressify') -Recurse -Filter report.json |
            Where-Object { $_.Directory.Name.StartsWith('desktop-') }
        $referencePairs = @($referenceReports | ForEach-Object { (Get-Content $_.FullName -Raw | ConvertFrom-Json).tests.pair })
        if ($referencePairs.Count -ne 3) { throw 'Expected three reference capture report entries.' }
        Invoke-Control 'Run Test'
        Wait-Control 'Capturing scenarios' | Out-Null
        Invoke-Control 'Stop'
        Wait-Control 'Run stopped' | Out-Null
        Capture-Window 'run-stopped'
        Invoke-Control 'Run Test'
        Wait-Control 'Capturing scenarios' | Out-Null
        Wait-Control 'Completed' | Out-Null
        Wait-Control 'Passed: 3' | Out-Null
        Capture-Window 'live-completed'
        Select-Theme 'Dark'
        Capture-Window 'live-completed-dark'
        Select-Theme 'System'
        Select-Theme 'Light'
        $capturedReports = Get-ChildItem (Join-Path $workspace '.regressify') -Recurse -Filter report.json |
            Where-Object { $_.Directory.Name.StartsWith('desktop-') }
        $capturedPairs = @($capturedReports | ForEach-Object { (Get-Content $_.FullName -Raw | ConvertFrom-Json).tests.pair })
        if ($capturedPairs.Count -ne 3) { throw 'Expected three current test report entries.' }
        foreach ($pair in $capturedPairs) {
            if ([string]::IsNullOrWhiteSpace($pair.scenarioLabel) -or [string]::IsNullOrWhiteSpace($pair.scenarioId)) {
                throw 'Capture reports are missing readable scenario metadata.'
            }
        }
        Invoke-Control 'Open Report'
        Wait-Control 'Approve This Scenario' | Out-Null
        Capture-Window 'completed-run-results'
        Invoke-Control 'Runs'
        $siblingReferences = @(Get-ChildItem (Join-Path $workspace '.regressify') -Recurse -Filter '*.png' |
            Where-Object { $_.Directory.Name -eq 'bitmaps_reference' -and -not $_.Name.StartsWith('8ac900fea19642cc0000') } |
            ForEach-Object { @{ Path = $_.FullName; Hash = (Get-FileHash $_.FullName).Hash } })
        Invoke-Control 'Run Test: Checkout'
        Wait-Control 'Capturing scenarios' | Out-Null
        Wait-Control 'Scenarios: smoke / Checkout' | Out-Null
        Capture-Window 'scenario-only-running'
        Wait-Control 'Completed' | Out-Null
        Wait-Control 'Passed: 1' | Out-Null
        $scenarioReports = Get-ChildItem (Join-Path $output 'bitmaps_test') -Recurse -Filter report.json |
            Where-Object { $_.Directory.Name.StartsWith('desktop-') }
        $scenarioPairs = @($scenarioReports | ForEach-Object { (Get-Content $_.FullName -Raw | ConvertFrom-Json).tests.pair })
        if ($scenarioPairs.Count -ne 1 -or $scenarioPairs[0].scenarioId -ne '8ac900fea19642cc0000') {
            throw 'Individual scenario execution captured an unselected sibling.'
        }
        foreach ($reference in $siblingReferences) {
            if ((Get-FileHash $reference.Path).Hash -ne $reference.Hash) { throw 'A scenario test changed a sibling reference.' }
        }
        Invoke-Control 'View log: Checkout (phone)'
        Wait-SourceText 'Clicking element: #continue' 'Scenario run log'
        $scenarioLog = Read-ControlText 'Scenario run log'
        if ($scenarioLog.Contains('Signed-in account') -or $scenarioLog.Contains('Executing scenario: Profile')) {
            throw 'Scenario log contains another scenario output.'
        }
        Capture-Window 'scenario-log'
        Select-Theme 'Dark'
        [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
        Capture-Window 'scenario-log-dark-compact'
        Select-Theme 'Light'
        [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
        Invoke-Control 'Back to Activity'
        Invoke-Control 'View run log: Checkout'
        Wait-SourceText 'Clicking element: #continue' 'Scenario run log'
        Invoke-Control 'All run logs'
        Wait-Control 'Run log' | Out-Null
        Invoke-Control 'Back to Activity'
    }
    Invoke-Control 'More actions' -Pointer
    Invoke-Control 'Initialize Workspace'
    Wait-Control 'Workspace initialized' | Out-Null
    $progress = Wait-Control 'Scenario job progress'
    if ($progress.Current.ControlType -ne [System.Windows.Automation.ControlType]::ProgressBar) { throw 'Scenario progress must expose the native progress role.' }
    $progressPattern = $null
    if (-not $progress.TryGetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern, [ref]$progressPattern)) { throw 'Scenario progress must expose its numeric range.' }
    $progressRange = ([System.Windows.Automation.RangeValuePattern]$progressPattern).Current
    if ($progressRange.Minimum -ne 0 -or $progressRange.Maximum -ne 100 -or $progressRange.Value -ne 0) { throw 'A zero-scenario command must expose zero percent progress in the 0..100 range.' }
    Capture-Window 'run-completed'
    if (-not (Test-Path (Join-Path $workspace 'common/_override.css'))) { throw 'GUI init command did not create starter files.' }
    Select-Theme 'Dark'
    $process.CloseMainWindow() | Out-Null
    if (-not $process.WaitForExit(10000)) { throw 'The clean fixture did not close after saving appearance.' }
    Start-Fixture
    Wait-Control 'Appearance: Dark' | Out-Null
    Invoke-Control 'smoke.tests.json'
    Invoke-Control 'Checkout'
    Invoke-Control 'Source'
    Wait-Control '15 px' | Out-Null
    Wait-Settings { param($settings) $settings.theme -eq 'dark' -and $settings.sourceFontSize -eq 15 -and $settings.sourceWrap -eq $false -and $settings.sourceLineNumbers -eq $false }
    Capture-Window 'appearance-restored'
    if (-not $KeepOpen) {
        $script:cleanupPath = Join-Path $workspace 'visual_tests/smoke.tests.json'
        $script:cleanupSnapshot = (Get-FileHash $cleanupPath -Algorithm SHA256).Hash
        Invoke-Control 'Editor'
        Invoke-Control 'Source'
        Invoke-Control 'Configuration source' -Pointer
        Send-FixtureKeys End -Control
        Send-FixtureText 'smoke-cleanup-probe'
        Wait-SourceText 'smoke-cleanup-probe'
    }
}
