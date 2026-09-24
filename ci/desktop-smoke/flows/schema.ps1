function Test-SchemaFlows {
    function Invoke-SchemaClipboard([scriptblock]$Operation) {
        $deadline = [System.Diagnostics.Stopwatch]::StartNew()
        do {
            try { return (& $Operation) } catch {
                if ($_.Exception.GetBaseException() -isnot [System.Runtime.InteropServices.ExternalException] -or $deadline.Elapsed.TotalSeconds -ge 10) { throw }
                [RegressifySmokeNative]::DwmFlush() | Out-Null
            }
        } while ($true)
    }
    function Write-SchemaSource([string]$Text) {
        Invoke-SchemaClipboard { [System.Windows.Forms.Clipboard]::SetText($Text) }
        Invoke-Control 'Configuration source' -Pointer
        Send-FixtureKeys A -Control
        Send-FixtureKeys V -Control
        try {
            Wait-SourceText $Text
        } catch {
            $actual = Read-ControlText 'Configuration source'
            $offset = 0
            while ($offset -lt [Math]::Min($actual.Length, $Text.Length) -and $actual[$offset] -eq $Text[$offset]) { $offset++ }
            $expectedCode = if ($offset -lt $Text.Length) { [int]$Text[$offset] } else { -1 }
            $actualCode = if ($offset -lt $actual.Length) { [int]$actual[$offset] } else { -1 }
            $normalizedMatch = $actual.Replace("`r`n", "`n") -ceq $Text.Replace("`r`n", "`n")
            Write-Output ("Schema paste mismatch: expectedLength={0}, actualLength={1}, expectedLF={2}, actualLF={3}, actualCR={4}, firstDifference={5}, expectedCode={6}, actualCode={7}, normalizedMatch={8}" -f $Text.Length, $actual.Length, ([regex]::Matches($Text, "`n").Count), ([regex]::Matches($actual, "`n").Count), ([regex]::Matches($actual, "`r").Count), $offset, $expectedCode, $actualCode, $normalizedMatch)
            Capture-Window 'schema-input-mismatch'
            throw 'The schema fixture paste did not preserve the expected source text.'
        }
    }
    function Test-SchemaFieldsRoundTrip([string]$Source) {
        Invoke-Control 'Fields' -Pointer
        Wait-Control 'Overrides only' | Out-Null
        if ($null -ne (Find-Control 'Configuration source')) { throw 'Fields did not replace the Source editor.' }
        Invoke-Control 'Source' -Pointer
        Wait-Control 'Configuration source' | Out-Null
        if ((Read-ControlText 'Configuration source').Replace("`r`n", "`n") -cne $Source) {
            throw 'Switching Source and Fields changed the invalid draft.'
        }
    }
    function Test-SchemaCompletionKeys {
        $switchWindow = [System.Windows.Forms.Form]::new()
        $switchWindow.Text = 'Regressify focus fixture'
        $switchWindow.Size = [System.Drawing.Size]::new(320, 180)
        try {
            $switchWindow.Show()
            $switchWindow.Activate()
            Wait-WindowState {
                [System.Windows.Forms.Application]::DoEvents()
                [RegressifySmokeFocus]::GetForegroundWindow() -eq $switchWindow.Handle
            } 'Cannot activate the temporary focus fixture.'
            foreach ($key in @('Enter', 'Tab', 'Escape')) {
                $draft = "scenarios: []`nbasicAuth:`n  ori"
                Write-SchemaSource $draft
                Send-FixtureKeys Spacebar -Control
                Wait-Control '1 suggestion' | Out-Null
                $focusedBefore = [System.Windows.Automation.AutomationElement]::FocusedElement
                if ($focusedBefore.Current.ProcessId -ne $process.Id) {
                    throw 'The Regressify fixture was not focused before Alt+Tab.'
                }
                Send-FixtureKeys Tab -Alt
                Wait-WindowState {
                    [System.Windows.Forms.Application]::DoEvents()
                    [RegressifySmokeFocus]::GetForegroundWindow() -eq $switchWindow.Handle
                } 'Alt+Tab did not switch to the temporary focus fixture.'
                Focus-Fixture
                try {
                    Wait-WindowState {
                        $focusedAfter = [System.Windows.Automation.AutomationElement]::FocusedElement
                        [System.Windows.Automation.Automation]::Compare($focusedBefore, $focusedAfter)
                    } 'Returning from Alt+Tab did not restore source editor focus.'
                } catch {
                    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
                    Write-Output ("Focus after Alt+Tab: name='{0}', type='{1}'" -f $focused.Current.Name, $focused.Current.ControlType.ProgrammaticName)
                    Capture-Window 'schema-completion-focus-failure'
                    throw
                }
                Capture-Window ('schema-completion-reactivated-' + $key.ToLowerInvariant())
                Send-FixtureKeys $key
                Wait-WindowState { $null -eq (Find-Control '1 suggestion') } "$key did not close the suggestion menu after Alt+Tab."
                if ($key -eq 'Escape') {
                    if ((Read-ControlText 'Configuration source').Replace("`r`n", "`n") -cne $draft) {
                        throw 'Escape changed the source instead of cancelling the suggestion.'
                    }
                } else {
                    Wait-SourceText 'origin:'
                    Send-FixtureText 'https://example.com'
                    if ((Read-ControlText 'Configuration source').Replace("`r`n", "`n") -cne "scenarios: []`nbasicAuth:`n  origin: https://example.com") {
                        throw "$key did not accept exactly one suggestion and preserve typing focus."
                    }
                }
            }
        } finally {
            $switchWindow.Close()
            $switchWindow.Dispose()
        }
    }
    function Test-SchemaLineCommands {
        function Wait-SchemaLineText([string]$Expected) {
            Wait-WindowState {
                (Read-ControlText 'Configuration source').Replace("`r`n", "`n") -ceq $Expected
            } 'A source line command did not preserve the expected text.'
        }
        function Select-SchemaFirstLines {
            Send-FixtureKeys Home -Control
            Send-FixtureKeys DownArrow -Shift
            Send-FixtureKeys DownArrow -Shift
            Wait-Control 'Ln 3, Col 1' | Out-Null
        }
        $draft = "debug: true`nheadless: true`nscenarios: []`n"
        $commented = "# debug: true`n# headless: true`nscenarios: []`n"
        Write-SchemaSource $draft
        Send-FixtureKeys Escape
        Select-SchemaFirstLines
        Send-FixtureKeys Oem2 -Control
        Wait-SchemaLineText $commented
        Send-FixtureKeys Oem2 -Control
        Wait-SchemaLineText $draft
        Send-FixtureKeys Z -Control
        Wait-SchemaLineText $commented
        Send-FixtureKeys Z -Control -Shift
        Wait-SchemaLineText $draft
        Select-SchemaFirstLines
        Send-FixtureKeys Tab
        Wait-SchemaLineText "  debug: true`n  headless: true`nscenarios: []`n"
        Send-FixtureKeys Tab -Shift
        Wait-SchemaLineText $draft
        Send-FixtureKeys DownArrow -Alt -Shift
        Wait-SchemaLineText "debug: true`nheadless: true`ndebug: true`nheadless: true`nscenarios: []`n"
        Send-FixtureKeys Z -Control
        Wait-SchemaLineText $draft
        Send-FixtureKeys DownArrow -Alt
        Wait-SchemaLineText "scenarios: []`ndebug: true`nheadless: true`n"
        Send-FixtureKeys UpArrow -Alt
        Wait-SchemaLineText $draft
        Send-FixtureKeys K -Control -Shift
        Wait-SchemaLineText "scenarios: []`n"
        Send-FixtureKeys Z -Control
        Wait-SchemaLineText $draft
        Invoke-Control 'Edit source lines' -Pointer
        Wait-Control 'Toggle Line Comment' | Out-Null
        Capture-Window 'source-line-menu'
        Invoke-Control 'Toggle Line Comment' -Pointer
        Wait-SchemaLineText $commented
        Capture-Window 'source-line-edits'
    }
    function Get-SchemaIssueDescription([int]$Line) {
        $row = Wait-Control "Issue on line $Line" 'Source issues'
        $description = $row.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
            Where-Object { $_.Current.Name.Contains(': ') } | Select-Object -First 1
        if ($null -eq $description) { throw "Issue on line $Line has no accessible description." }
        return $description
    }
    function Scroll-SchemaIssues([switch]$Bottom) {
        $bounds = (Wait-Control 'Source issues').Current.BoundingRectangle
        Focus-Fixture
        if (-not [RegressifySmokePointer]::SetPhysicalCursorPos([int]($bounds.Left + $bounds.Width / 2), [int]($bounds.Top + $bounds.Height / 2))) {
            throw 'Cannot position the mouse over the source issues table.'
        }
        $delta = if ($Bottom) { [uint32]::MaxValue - 1199 } else { 1200 }
        [RegressifySmokeNative]::mouse_event(0x0800, 0, 0, $delta, [UIntPtr]::Zero)
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    }
    function Test-SchemaIssueCopy([int]$Line) {
        $description = Get-SchemaIssueDescription $Line
        $expected = $description.Current.Name
        $bounds = $description.Current.BoundingRectangle
        $scale = [RegressifySmokeWindowState]::GetDpiForWindow($script:window) / 96.0
        Invoke-SchemaClipboard { [System.Windows.Forms.Clipboard]::SetText('No diagnostic selection copied') }
        Focus-Fixture
        try {
            if (-not [RegressifySmokePointer]::SetPhysicalCursorPos([int]($bounds.Left + 1 * $scale), [int]($bounds.Top + 10 * $scale))) {
                throw 'Cannot position the mouse at the issue description.'
            }
            [RegressifySmokeNative]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
            [RegressifySmokeNative]::DwmFlush() | Out-Null
            if (-not [RegressifySmokePointer]::SetPhysicalCursorPos([int]($bounds.Left + 150 * $scale), [int]($bounds.Top + 10 * $scale))) {
                throw 'Cannot select the issue description.'
            }
            [RegressifySmokeNative]::DwmFlush() | Out-Null
        } finally { [RegressifySmokeNative]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero) }
        Capture-Window 'schema-issue-selection'
        Send-FixtureKeys C -Control
        Wait-WindowState {
            $copied = Invoke-SchemaClipboard { [System.Windows.Forms.Clipboard]::GetText() }
            $copied.Length -gt 0 -and $copied.Length -lt $expected.Length -and $expected.Contains($copied)
        } 'Dragging across an issue did not select copyable text.'
        Send-FixtureKeys A -Control
        Send-FixtureKeys C -Control
        Wait-WindowState {
            (Invoke-SchemaClipboard { [System.Windows.Forms.Clipboard]::GetText() }).Replace("`r`n", "`n") -ceq $expected
        } 'Copying an entire issue lost text, punctuation, or line breaks.'
    }
    function Test-SchemaIssueTable([string]$CaptureName) {
        Scroll-SchemaIssues
        $table = (Wait-Control 'Source issues').Current.BoundingRectangle
        $headers = @('Line', 'Type', 'Description') | ForEach-Object { (Wait-Control $_ 'Source issues').Current.BoundingRectangle }
        Wait-WindowState {
            $first = Find-Control 'Go to issue on line 4' 'Source issues'
            $null -ne $first -and $first.Current.BoundingRectangle.Top -ge $headers[0].Bottom
        } 'Scrolling did not reveal the first issue below the fixed table header.'
        $line = (Wait-Control 'Go to issue on line 4' 'Source issues').Current.BoundingRectangle
        $severity = (Wait-Control 'Error' 'Issue on line 4').Current.BoundingRectangle
        $description = (Get-SchemaIssueDescription 4).Current.BoundingRectangle
        $scale = [RegressifySmokeWindowState]::GetDpiForWindow($script:window) / 96.0
        $tolerance = 2 * $scale
        if ([Math]::Abs($line.Top - $description.Top) -gt $tolerance -or [Math]::Abs($severity.Top - $description.Top) -gt $tolerance) {
            throw 'The issue line link or severity dot is not aligned with the first description line.'
        }
        if ([Math]::Abs(($line.Left + $line.Width / 2) - ($headers[0].Left + $headers[0].Width / 2)) -gt $tolerance -or [Math]::Abs(($severity.Left + $severity.Width / 2) - ($headers[1].Left + $headers[1].Width / 2)) -gt $tolerance -or [Math]::Abs($description.Left - $headers[2].Left) -gt $tolerance) {
            throw 'The source issue columns do not line up with their headers.'
        }
        if ($headers[0].Right -gt $headers[1].Left -or $headers[1].Right -gt $headers[2].Left -or $line.Right -gt $severity.Left -or $severity.Right -gt $description.Left -or $description.Right -gt $table.Right + $tolerance) {
            throw 'Source issue columns overlap or extend beyond the panel.'
        }
        Capture-Window $CaptureName
        $windowBounds = [RegressifySmokeNative+Rect]::new()
        if (-not [RegressifySmokeNative]::GetWindowRect($script:window, [ref]$windowBounds)) { throw 'Cannot locate the source issue screenshot.' }
        $bitmap = [System.Drawing.Bitmap]::new((Join-Path $artifacts ($CaptureName + '.png')))
        try {
            $color = $bitmap.GetPixel([int]($severity.Left - $windowBounds.Left + $severity.Width / 2), [int]($severity.Top - $windowBounds.Top + $severity.Height / 2))
            if ($color.R -le $color.G * 1.2 -or $color.R -le $color.B * 1.1) { throw 'The error severity indicator is not a centered red dot.' }
        } finally { $bitmap.Dispose() }
        Scroll-SchemaIssues -Bottom
        Wait-WindowState {
            $last = Find-Control 'Go to issue on line 8' 'Source issues'
            $null -ne $last -and $last.Current.BoundingRectangle.Top -ge $headers[0].Bottom -and $last.Current.BoundingRectangle.Bottom -le $table.Bottom
        } 'The final source issue cannot be scrolled fully into view.'
        Invoke-Control 'Go to issue on line 8' -Pointer
        Wait-Control 'Ln 8, Col 1' | Out-Null
    }
    $clipboard = Invoke-SchemaClipboard { [System.Windows.Forms.Clipboard]::GetDataObject() }
    $backup = [System.Windows.Forms.DataObject]::new()
    if ($null -ne $clipboard) {
        foreach ($format in $clipboard.GetFormats($false)) {
            $data = $clipboard.GetData($format, $false)
            if ($data -is [System.ICloneable]) { $data = $data.Clone() }
            $backup.SetData($format, $false, $data)
        }
    }
    try {
    $path = Join-Path $workspace 'visual_tests/schema.tests.yaml'
    $original = (Get-FileHash -LiteralPath $path).Hash
    Invoke-Control 'schema.tests.yaml'
    Wait-Control 'Configuration source' | Out-Null
    $incomplete = "scenarios:`n  - url: https://example.com`n    actions:`n      - click:`n      - hover:`n"
    Write-SchemaSource $incomplete
    Send-FixtureKeys Escape
    Wait-Control '2 issues' | Out-Null
    Test-SchemaFieldsRoundTrip $incomplete
    Wait-Control '2 issues' | Out-Null
    Write-SchemaSource 'scenarios: [}'
    Send-FixtureKeys Escape
    Wait-Control '1 issue' | Out-Null
    Invoke-Control 'Fields' -Pointer
    Wait-Control 'Correct the source syntax errors before switching to Fields' | Out-Null
    if ((Read-ControlText 'Configuration source') -cne 'scenarios: [}') { throw 'Attempting Fields changed an unparseable draft.' }
    $invalid = "scenarios:`n  - url: https://example.com`n    actions:`n      - press: '#search'`nbasicAuth:`n  - username: qa-user`n    password: synthetic-password`ndebugg: true`n"
    Write-SchemaSource $invalid
    Send-FixtureKeys Escape
    Wait-Control '3 issues' | Out-Null
    Test-SchemaFieldsRoundTrip $invalid
    Wait-Control '3 issues' | Out-Null
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($script:window)
    $messages = @($root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
        ForEach-Object { $_.Current.Name } | Where-Object { $_ -like '*basicAuth[[]0].origin: Missing required field*' })
    if ($messages.Count -eq 0) { throw 'The native editor did not explain the missing Basic Auth origin.' }
    foreach ($message in $messages) {
        if ($message.Contains('synthetic-password') -or $message.Contains('qa-user') -or $message.Contains('BasicAuthConfig')) {
            throw 'Schema diagnostics exposed entered credentials or an internal enum name.'
        }
    }
    Send-FixtureKeys S -Control
    if ((Get-FileHash -LiteralPath $path).Hash -ne $original) { throw 'Saving an invalid draft changed the saved suite.' }
    Invoke-Control '3 issues'
    Invoke-Control '3 issues'
    Wait-WindowState {
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($script:window)
        @($root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
            Where-Object { $_.Current.Name.StartsWith('Ln 6, Col ') }).Count -gt 0
    } 'Issue navigation did not reach the missing-origin entry.'
    Test-SchemaIssueCopy 4
    if ((Read-ControlText 'Configuration source').Replace("`r`n", "`n") -cne $invalid) { throw 'Selecting issue text changed the source draft.' }
    foreach ($theme in @('Light', 'Dark')) {
        Select-Theme $theme
        foreach ($dimensions in @(@(1600, 1000), @(1000, 720))) {
            [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, $dimensions[0], $dimensions[1], 0x0040) | Out-Null
            Wait-Control '3 issues' | Out-Null
            Test-SchemaIssueTable ('schema-errors-' + $theme.ToLowerInvariant() + '-' + $dimensions[0])
        }
    }
    Write-SchemaSource "scenarios: []`nbasicAuth:`n  ori"
    Send-FixtureKeys Spacebar -Control
    Wait-Control '1 suggestion' | Out-Null
    Capture-Window 'schema-origin-completion'
    Send-FixtureKeys Enter
    Wait-SourceText 'origin:'
    Send-FixtureKeys Escape
    Test-SchemaCompletionKeys
    Test-SchemaLineCommands
    $valid = "scenarios:`n  - url: https://example.com`n    label: Schema example`nbasicAuth:`n  origin: https://example.com`n  username: qa-user`n  password: synthetic-password`n"
    Write-SchemaSource $valid
    Send-FixtureKeys Escape
    Wait-WindowState { $null -eq (Find-Control '1 issue') -and $null -eq (Find-Control '3 issues') } 'Correcting the source did not clear its issues.'
    Send-FixtureKeys S -Control
    Wait-Control 'Saved' | Out-Null
    Wait-Control '0 reference images / 0 saved test runs' | Out-Null
    if ($null -ne (Find-Control "Suite 'schema' has configuration errors")) { throw 'The corrected suite retained a stale configuration error.' }
    if ((Get-Content -LiteralPath $path -Raw).Replace("`r`n", "`n") -ne $valid) { throw 'Saving the corrected suite changed its source.' }
    Capture-Window 'schema-corrected'
    Write-Output 'Native Source/Fields navigation, syntax protection, selectable issue descriptions, exact clipboard text, aligned table columns, red severity dots, issue scrolling/navigation, safe diagnostics, Enter/Tab completion and Escape after Alt+Tab, selected-line editing and undo/redo, corrected save, and compact light/dark layouts passed.'
    } finally {
        if ($null -ne $clipboard) {
            Invoke-SchemaClipboard { [System.Windows.Forms.Clipboard]::SetDataObject($backup, $true) }
        } else {
            Invoke-SchemaClipboard { [System.Windows.Forms.Clipboard]::Clear() }
        }
    }
}
