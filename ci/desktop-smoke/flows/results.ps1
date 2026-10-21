function Test-CaptureInformation([string]$Name, [switch]$SameUrl, [int]$DiffX = 0, [int]$DiffY = 0) {
    $clipboard = Invoke-FixtureClipboard { Copy-FixtureClipboardData ([System.Windows.Forms.Clipboard]::GetDataObject()) -FixtureOwned:$script:fixtureClipboardOwned }
    try {
        Hover-Control 'Scenario information'
        Wait-Control 'Scenario information details' | Out-Null
        $label = if ($SameUrl) { 'Account desktop' } else { 'Checkout' }
        $selector = if ($SameUrl) { 'body .carousel' } else { 'document' }
        Wait-Control "Label: $label" | Out-Null
        Wait-Control "Selector: $selector" | Out-Null
        if ($SameUrl) {
            Wait-Control 'Same URL' | Out-Null
            Wait-Control 'Diff %: Unavailable' | Out-Null
            Wait-Control 'Diff X: Unavailable' | Out-Null
            Wait-Control 'Diff Y: Unavailable' | Out-Null
            Wait-Control 'Mismatch pixels: Unavailable' | Out-Null
        } else {
            Wait-Control 'Diff %: 4.20%' | Out-Null
            Wait-Control "Diff X: $DiffX" | Out-Null
            Wait-Control "Diff Y: $DiffY" | Out-Null
            Wait-Control 'Mismatch pixels: 42' | Out-Null
        }
        foreach ($kind in @('Reference', 'Test')) {
            if (-not (Test-ControlInvokable "Open $kind URL")) { throw "$kind URL must be clickable." }
            Invoke-FixtureClipboard { [System.Windows.Forms.Clipboard]::SetText('Regressify URL copy pending') }
            $clipboardSequence = [RegressifySmokeClipboard]::GetClipboardSequenceNumber()
            Hover-Control "Copy $kind URL"
            Invoke-Control "Copy $kind URL" -Pointer
            $expected = if ($SameUrl) { 'https://example.com/account' } elseif ($kind -eq 'Reference') { 'https://example.com/checkout' } else { 'https://staging.example.com/checkout' }
            try {
                Wait-FixtureClipboardText $clipboardSequence $expected "$kind URL was not copied correctly."
            } catch {
                Capture-Window 'capture-information-copy-failure'
                if ((Invoke-FixtureClipboard { [System.Windows.Forms.Clipboard]::GetText() }) -ceq 'Regressify URL copy pending') { throw "$kind URL copy did not replace the synthetic clipboard sentinel." }
                throw
            }
        }
        Capture-Window $Name
        Hover-Control 'Help'
        Wait-WindowState { $null -eq (Find-Control 'Scenario information details') } 'Scenario information did not close after the pointer left.'
    } finally {
        Invoke-FixtureClipboard {
            if ($null -ne $clipboard) { [System.Windows.Forms.Clipboard]::SetDataObject($clipboard, $true) } else { [System.Windows.Forms.Clipboard]::Clear() }
        }
    }
}

function Test-ImageContextMenus {
    $clipboard = Invoke-FixtureClipboard { Copy-FixtureClipboardData ([System.Windows.Forms.Clipboard]::GetDataObject()) -FixtureOwned:$script:fixtureClipboardOwned }
    try {
        foreach ($pane in @('Reference', 'Test', 'Diff')) {
            Invoke-FixtureClipboard { [System.Windows.Forms.Clipboard]::SetText('Regressify URL copy pending') }
            Hover-Control "$pane image preview"
            Send-FixturePointerClick -Right
            foreach ($name in @('Open in default browser', 'Open in default image viewer', 'Show in file manager')) {
                if (-not (Test-MenuActionEnabled $name)) { throw "Image action '$name' is unavailable." }
            }
            if ($pane -ne 'Diff') {
                if (-not (Test-MenuActionEnabled "Open $pane URL")) { throw "$pane URL must be available in the image menu." }
                $clipboardSequence = [RegressifySmokeClipboard]::GetClipboardSequenceNumber()
                Invoke-Control "Copy $pane URL" -Pointer
                $expected = if ($pane -eq 'Reference') { 'https://example.com/checkout' } else { 'https://staging.example.com/checkout' }
                try {
                    Wait-FixtureClipboardText $clipboardSequence $expected "Image menu copied an incorrect $pane URL."
                } catch {
                    Capture-Window 'image-menu-copy-failure'
                    if ((Invoke-FixtureClipboard { [System.Windows.Forms.Clipboard]::GetText() }) -ceq 'Regressify URL copy pending') { throw "Image menu $pane URL copy did not replace the synthetic clipboard sentinel." }
                    throw
                }
            } else {
                if ($null -ne (Find-Control 'Open Reference URL') -or $null -ne (Find-Control 'Copy Test URL')) { throw 'Diff must not offer scenario URL actions.' }
                Send-FixtureKeys Escape
            }
            Wait-WindowState { $null -eq (Find-Control 'Open in default browser') } 'Image context menu did not close.'
        }
    } finally {
        Invoke-FixtureClipboard {
            if ($null -ne $clipboard) { [System.Windows.Forms.Clipboard]::SetDataObject($clipboard, $true) } else { [System.Windows.Forms.Clipboard]::Clear() }
        }
    }
}

function Test-ReportApproval {
    $run = Join-Path $output 'bitmaps_test/approval-run'
    $scenario = '8ac900fea19642cc0000'
    $captures = [ordered]@{}
    $tests = @()
    foreach ($viewport in @('desktop', 'phone')) {
        $key = "${scenario}_${viewport}__FINAL"
        Write-TestImage (Join-Path $output "bitmaps_reference/$key.png") ([System.Drawing.Color]::Teal)
        Write-TestImage (Join-Path $run "$key.png") ([System.Drawing.Color]::RoyalBlue)
        Write-TestImage (Join-Path $output "bitmaps_test/zz-newer-run/$key.png") ([System.Drawing.Color]::OrangeRed)
        $captures[$key] = @{ scenarioId = $scenario; scenarioLabel = 'Checkout'; referenceUrl = 'https://example.com/checkout'; url = 'https://staging.example.com/checkout' }
        $tests += @{ status = 'fail'; pair = @{
            label = 'Checkout'; scenarioId = $scenario; scenarioLabel = 'Checkout'; selector = 'document'
            referenceUrl = 'https://example.com/checkout'; url = 'https://staging.example.com/checkout'
            fileName = "$key.png"; reference = "../bitmaps_reference/$key.png"; test = "../bitmaps_test/approval-run/$key.png"; misMatchThreshold = 0.1
        } }
    }
    Write-JsonFixture (Join-Path $run '.report-urls.json') @{ version = 1; captures = $captures }
    Write-JsonFixture (Join-Path $run 'report.json') @{ id = 'smoke'; testSuite = 'smoke'; tests = $tests }
    Invoke-Control 'Editor'
    Invoke-Control 'Results'
    Wait-Control 'Mismatch Checkout' | Out-Null
    Wait-Control 'Test 640 x 420' | Out-Null
    Wait-Control '2 failed / 2 finished' | Out-Null
    if ($null -ne (Find-Control 'Approve This Run')) { throw 'Results still exposes whole-run approval.' }
    $sibling = Join-Path $output "bitmaps_reference/${scenario}_phone__FINAL.png"
    $siblingHash = (Get-FileHash $sibling).Hash
    Invoke-Control 'Approve This Scenario'
    Wait-Control 'Scenario approval confirmation' | Out-Null
    Wait-Control 'Test image preview' | Out-Null
    Wait-Control 'Run: approval-run' | Out-Null
    Wait-Control 'Viewport: desktop' | Out-Null
    if ($null -ne (Find-Control 'Run confirmation')) { throw 'Report approval switched to Runs.' }
    Capture-Window 'approve-scenario-viewport-confirmation'
    Invoke-Control 'Cancel' -Within 'Scenario approval confirmation'
    Wait-WindowState { $null -eq (Find-Control 'Scenario approval confirmation') } 'Cancel did not dismiss report approval.'
    Wait-Control 'Test image preview' | Out-Null
    Wait-Control '2 failed / 2 finished' | Out-Null
    Invoke-Control 'Approve This Scenario'
    Invoke-Control 'Approve This Scenario' -Within 'Scenario approval confirmation'
    Wait-Control 'Test image preview' | Out-Null
    Wait-Control 'Passed Checkout' | Out-Null
    Wait-Control 'Mismatch Checkout' | Out-Null
    Wait-Control '1 failed / 2 finished' | Out-Null
    $report = Get-Content (Join-Path $run 'report.json') -Raw | ConvertFrom-Json
    if ($report.tests.Count -ne 2 -or $report.tests[0].status -ne 'pass' -or $report.tests[1].status -ne 'fail' -or $report.tests[0].pair.diff.rawMisMatchPercentage -ne 0) {
        throw 'Scenario approval did not recalculate the complete report.'
    }
    if ((Get-FileHash $sibling).Hash -ne $siblingHash) { throw 'Approval changed another viewport baseline.' }
    $approved = Join-Path $output "bitmaps_reference/${scenario}_desktop__FINAL.png"
    if ((Get-FileHash $approved).Hash -ne (Get-FileHash (Join-Path $run "${scenario}_desktop__FINAL.png")).Hash) {
        throw 'Approval promoted a different run than the reviewed capture.'
    }
    Capture-Window 'approve-scenario-viewport-results'
    Write-Output 'Inline approval/cancel stayed in Results, refreshed suite failure counts, pinned the historical run, and preserved the sibling viewport baseline.'
}
