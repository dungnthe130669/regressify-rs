function Invoke-FixtureGit([string]$Root, [string[]]$Arguments) {
    $result = & git -C $Root @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Fixture Git failed: $($result -join [Environment]::NewLine)" }
    $result
}

function Test-GitFlows {
    Send-FixtureKeys G -Control -Shift
    Wait-Control 'Workspace is not tracked by Git' | Out-Null
    Capture-Window 'git-not-tracked'
    Invoke-Control 'Initialize Git'
    Invoke-Control 'Initialize repository' -Within 'Git confirmation'
    Wait-Control 'Git repository initialized; files have not been staged' | Out-Null
    if (-not (Test-Path (Join-Path $workspace '.git'))) { throw 'Git UI did not initialize the fixture.' }
    Invoke-FixtureGit $workspace @('symbolic-ref', 'HEAD', 'refs/heads/main') | Out-Null
    foreach ($config in @(@('user.name', 'Git UI smoke'), @('user.email', 'git-ui@example.test'), @('commit.gpgsign', 'false'), @('core.hooksPath', 'disabled-hooks'), @('core.autocrlf', 'false'))) {
        Invoke-FixtureGit $workspace (@('config') + $config) | Out-Null
    }
    [System.IO.File]::WriteAllText((Join-Path $workspace '.gitignore'), ".regressify*/`noriginal-desktop-settings.json`n")
    [System.IO.Directory]::CreateDirectory((Join-Path $workspace 'nested')) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $workspace 'nested/selected.txt'), "initial`n")
    [System.IO.File]::WriteAllText((Join-Path $workspace 'nested/staged.txt'), "initial`n")
    $reviewSuite = 'visual_tests/qa-review.tests.json'
    $reviewResult = '.regressify/qa-review/report.json'
    $unselectedResult = '.regressify__staging/qa-review/report.json'
    Write-JsonFixture (Join-Path $workspace $reviewSuite) @{ scenarios = @(@{ id = 'checkout'; label = 'Checkout'; url = '/checkout'; actions = @(@{ wait = 1000 }) }) }
    $reviewBefore = [System.IO.File]::ReadAllText((Join-Path $workspace $reviewSuite))
    foreach ($path in @($reviewResult, $unselectedResult)) { Write-JsonFixture (Join-Path $workspace $path) @{ status = 'before' } }
    Invoke-FixtureGit $workspace @('add', '.') | Out-Null
    Invoke-FixtureGit $workspace @('add', '-f', '--', $reviewResult, $unselectedResult) | Out-Null
    Invoke-FixtureGit $workspace @('commit', '-m', 'initial fixture') | Out-Null
    Write-JsonFixture (Join-Path $workspace $reviewSuite) @{ scenarios = @(@{ id = 'checkout'; label = 'Checkout'; url = '/checkout'; actions = @(@{ wait = 2500 }) }) }
    $reviewAfter = [System.IO.File]::ReadAllText((Join-Path $workspace $reviewSuite))
    foreach ($path in @($reviewResult, $unselectedResult)) { Write-JsonFixture (Join-Path $workspace $path) @{ status = 'after' } }
    Invoke-FixtureGit $workspace @('add', '-u', '--', $reviewResult, $unselectedResult) | Out-Null
    Invoke-Control 'Refresh Git status'
    Wait-Control '1 changed files' | Out-Null
    Wait-Control '0 comparison files selected' | Out-Null
    if ($null -ne (Find-Control "Select $reviewResult")) { throw 'Comparison Changes must start collapsed.' }
    Invoke-Control "Review $reviewSuite"
    Wait-Control 'Copy last committed file' | Out-Null
    Wait-Control 'Copy saved file' | Out-Null
    Wait-WindowState {
        Invoke-Control 'Copy last committed file'
        (Get-Clipboard -Raw) -ceq $reviewBefore
    } 'Git comparison did not copy the exact committed text.'
    Wait-WindowState {
        Invoke-Control 'Copy saved file'
        (Get-Clipboard -Raw) -ceq $reviewAfter
    } 'Git comparison did not copy the exact saved text.'
    Invoke-Control 'Next change'
    foreach ($theme in @('Light', 'Dark')) {
        Select-Theme $theme
        foreach ($dimensions in @(@(1600, 1000), @(1080, 720))) {
            [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, $dimensions[0], $dimensions[1], 0x0040) | Out-Null
            Capture-Window ('git-text-diff-' + $theme.ToLowerInvariant() + '-' + $dimensions[0])
        }
    }
    $invalidReview = '{ "scenarios": ['
    [System.IO.File]::WriteAllText((Join-Path $workspace $reviewSuite), $invalidReview)
    Invoke-Control 'Refresh Git status'
    Invoke-Control "Review $reviewSuite"
    Wait-WindowState {
        Invoke-Control 'Copy saved file'
        (Get-Clipboard -Raw) -ceq $invalidReview
    } 'Malformed JSON did not remain available in the text comparison.'
    Capture-Window 'git-text-diff-invalid-json'
    [System.IO.File]::WriteAllText((Join-Path $workspace $reviewSuite), $reviewAfter)
    Invoke-Control 'Refresh Git status'
    Invoke-Control "Review $reviewSuite"
    Wait-WindowState {
        Invoke-Control 'Copy saved file'
        (Get-Clipboard -Raw) -ceq $reviewAfter
    } 'Restored source did not refresh the text comparison.'
    Invoke-Control 'All' -Within 'Git changes'
    Wait-Control '0 comparison files selected' | Out-Null
    Write-FixtureControlText 'Git commit message' 'Review configuration without comparison results'
    Invoke-Control 'Commit selected files'
    Invoke-Control 'Commit selected files' -Within 'Git confirmation'
    Wait-Control 'Committed 1 selected file(s)' | Out-Null
    foreach ($path in @($reviewResult, $unselectedResult)) {
        $committed = (Invoke-FixtureGit $workspace @('show', "HEAD:$path")) -join "`n"
        $staged = (Invoke-FixtureGit $workspace @('show', ":$path")) -join "`n"
        if ($committed -notmatch 'before' -or $staged -notmatch 'after') { throw 'Ordinary All included comparison changes in the commit.' }
    }
    Invoke-Control 'Comparison Changes'
    Invoke-Control "Select $reviewResult"
    Wait-Control '1 comparison files selected' | Out-Null
    Capture-Window 'git-comparison-changes-selected'
    Write-FixtureControlText 'Git commit message' 'Explicitly include reviewed comparison result'
    Invoke-Control 'Commit selected files'
    Capture-Window 'git-comparison-commit-confirmation'
    Invoke-Control 'Commit selected files' -Within 'Git confirmation'
    Wait-WindowState { ((Invoke-FixtureGit $workspace @('show', "HEAD:$reviewResult")) -join "`n") -match 'after' } 'Explicitly selected comparison file was not committed.'
    Wait-Control '0 comparison files selected' | Out-Null
    Wait-Control 'Committed 1 selected file(s)' | Out-Null
    if (((Invoke-FixtureGit $workspace @('show', "HEAD:$unselectedResult")) -join "`n") -notmatch 'before') { throw 'An unchecked comparison result was committed.' }
    Invoke-Control "Select $unselectedResult"
    Write-FixtureControlText 'Git commit message' 'Finish comparison fixture'
    Invoke-Control 'Commit selected files'
    Invoke-Control 'Commit selected files' -Within 'Git confirmation'
    Wait-WindowState { ((Invoke-FixtureGit $workspace @('show', "HEAD:$unselectedResult")) -join "`n") -match 'after' } 'Remaining comparison fixture was not committed.'
    Wait-Control '0 comparison files selected' | Out-Null
    Invoke-Control 'Comparison Changes'
    Select-Theme 'Light'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1600, 1000, 0x0040) | Out-Null
    $remote = Join-Path $artifacts ('git-remote-' + [guid]::NewGuid().ToString('N'))
    [System.IO.Directory]::CreateDirectory($remote) | Out-Null
    Invoke-FixtureGit $remote @('init', '--bare', '-b', 'main') | Out-Null
    $alternate = Join-Path $artifacts ('git-remote-alternate-' + [guid]::NewGuid().ToString('N'))
    [System.IO.Directory]::CreateDirectory($alternate) | Out-Null
    Invoke-FixtureGit $alternate @('init', '--bare', '-b', 'main') | Out-Null
    foreach ($name in @('origin', 'review')) {
        Invoke-Control 'Add remote'
        Write-FixtureControlText 'Remote name' $name
        Write-FixtureControlText 'Remote URL' $remote
        Invoke-Control 'Save remote'
        Invoke-Control 'Add remote' -Within 'Git confirmation'
        Wait-Control "Remote '$name' added" | Out-Null
    }
    Invoke-Control 'Select Git remote' -Pointer
    Invoke-Control 'review'
    Invoke-Control 'Select remote' -Within 'Git confirmation'
    Wait-Control "Remote 'review' selected" | Out-Null
    Invoke-Control 'Edit fetch URL'
    Write-FixtureControlText 'Remote URL' $alternate
    Invoke-Control 'Save remote'
    Invoke-Control 'Update remote URL' -Within 'Git confirmation'
    Wait-Control "Remote 'review' fetch URL updated" | Out-Null
    Invoke-Control 'Edit push URL'
    Write-FixtureControlText 'Remote URL' $remote
    Invoke-Control 'Save remote'
    Invoke-Control 'Update remote URL' -Within 'Git confirmation'
    Wait-Control "Remote 'review' push URL updated" | Out-Null
    Invoke-Control 'Rename remote'
    Write-FixtureControlText 'Remote name' 'backup'
    Invoke-Control 'Save remote'
    Invoke-Control 'Rename remote' -Within 'Git confirmation'
    Wait-Control "Remote 'review' renamed to 'backup'" | Out-Null
    if (((Invoke-FixtureGit $workspace @('remote', 'get-url', 'backup')) -join '').Trim() -ne $alternate) { throw 'Fetch URL update was not retained after rename.' }
    if (((Invoke-FixtureGit $workspace @('remote', 'get-url', '--push', 'backup')) -join '').Trim() -ne $remote) { throw 'Push URL update was not retained after rename.' }
    foreach ($theme in @('Light', 'Dark')) {
        Select-Theme $theme
        foreach ($dimensions in @(@(1600, 1000), @(1080, 720))) {
            [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, $dimensions[0], $dimensions[1], 0x0040) | Out-Null
            Wait-Control 'Copy Git logs' | Out-Null
            Capture-Window ('git-remotes-' + $theme.ToLowerInvariant() + '-' + $dimensions[0])
        }
    }
    Invoke-Control 'Remove remote'
    Capture-Window 'git-remove-remote-confirmation'
    Invoke-Control 'Cancel' -Within 'Git confirmation'
    if (-not ((Invoke-FixtureGit $workspace @('remote')) -contains 'backup')) { throw 'Cancelled removal changed remotes.' }
    Invoke-Control 'Remove remote'
    Invoke-Control 'Remove remote' -Within 'Git confirmation'
    Wait-Control "Remote 'backup' removed locally" | Out-Null
    if (Test-ControlInvokable 'Push') { throw 'Removing the selected remote silently selected another destination.' }
    Invoke-Control 'Select Git remote' -Pointer
    Invoke-Control 'origin'
    Invoke-Control 'Select remote' -Within 'Git confirmation'
    Wait-Control "Remote 'origin' selected" | Out-Null
    Invoke-Control 'Push'
    Invoke-Control 'Push' -Within 'Git confirmation'
    Wait-Control 'Push completed' | Out-Null
    Wait-SourceText 'Running git push' 'Git logs'
    Capture-Window 'git-push-activity'
    Select-Theme 'Light'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1600, 1000, 0x0040) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $workspace 'nested/selected.txt'), "reviewed local change`n")
    [System.IO.File]::WriteAllText((Join-Path $workspace 'nested/staged.txt'), "keep staged`n")
    Invoke-FixtureGit $workspace @('add', 'nested/staged.txt') | Out-Null
    Invoke-Control 'Refresh Git status'
    Wait-Control '2 changed files' | Out-Null
    Wait-Control 'Review nested/selected.txt' | Out-Null
    Capture-Window 'git-change-tree'
    Invoke-Control 'Review nested/selected.txt'
    Wait-Control 'Copy last committed file' | Out-Null
    Wait-WindowState {
        Invoke-Control 'Copy saved file'
        (Get-Clipboard -Raw) -ceq "reviewed local change`n"
    } 'Split Git comparison did not show the exact saved file.'
    Capture-Window 'git-diff-review'
    Invoke-Control 'Select nested/selected.txt'
    Write-FixtureControlText 'Git commit message' 'Commit selected fixture change'
    Invoke-Control 'Commit selected files'
    Capture-Window 'git-commit-confirmation'
    Invoke-Control 'Commit selected files' -Within 'Git confirmation'
    Wait-Control 'Committed 1 selected file(s)' | Out-Null
    $stagedHead = (Invoke-FixtureGit $workspace @('show', 'HEAD:nested/staged.txt')) -join "`n"
    $stagedIndex = (Invoke-FixtureGit $workspace @('show', ':nested/staged.txt')) -join "`n"
    if ($stagedHead.Trim() -ne 'initial' -or $stagedIndex.Trim() -ne 'keep staged') { throw 'Selected commit changed the unrelated staged file.' }
    Invoke-Control 'Push'
    Invoke-Control 'Push' -Within 'Git confirmation'
    Wait-Control 'Push completed' | Out-Null
    Capture-Window 'git-pushed'
    $clone = Join-Path $artifacts ('git-peer-' + [guid]::NewGuid().ToString('N'))
    Invoke-FixtureGit $artifacts @('clone', $remote, $clone) | Out-Null
    foreach ($config in @(@('user.name', 'Git peer fixture'), @('user.email', 'peer@example.test'), @('commit.gpgsign', 'false'), @('core.hooksPath', 'disabled-hooks'))) {
        Invoke-FixtureGit $clone (@('config') + $config) | Out-Null
    }
    [System.IO.File]::WriteAllText((Join-Path $clone 'incoming.txt'), "incoming remote commit`n")
    Invoke-FixtureGit $clone @('add', 'incoming.txt') | Out-Null
    Invoke-FixtureGit $clone @('commit', '-m', 'remote incoming') | Out-Null
    Invoke-FixtureGit $clone @('push', 'origin', 'main') | Out-Null
    Wait-Control '1 incoming commits' -TimeoutSeconds 100 | Out-Null
    Capture-Window 'git-remote-ahead'
    if (Test-ControlInvokable 'Pull') { throw 'Pull must be disabled while local changes remain.' }
    Invoke-Control 'Select nested/staged.txt'
    Write-FixtureControlText 'Git commit message' 'Commit retained staged fixture'
    Invoke-Control 'Commit selected files'
    Invoke-Control 'Commit selected files' -Within 'Git confirmation'
    Wait-Control 'Committed 1 selected file(s)' | Out-Null
    Invoke-Control 'Fetch'
    Wait-Control 'Branches diverged. Resolve in VS Code, then refresh Git status.' | Out-Null
    Capture-Window 'git-diverged'
    if (Test-ControlInvokable 'Pull') { throw 'Diverged branch must not pull silently.' }
    Invoke-FixtureGit $workspace @('merge', '--no-edit', 'origin/main') | Out-Null
    Invoke-FixtureGit $workspace @('push', 'origin', 'main') | Out-Null
    Invoke-FixtureGit $clone @('pull', '--ff-only', 'origin', 'main') | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $clone 'incoming.txt'), "newest remote commit`n")
    Invoke-FixtureGit $clone @('commit', '-am', 'remote fast-forward') | Out-Null
    Invoke-FixtureGit $clone @('push', 'origin', 'main') | Out-Null
    Invoke-Control 'Refresh Git status'
    Invoke-Control 'Fetch'
    Wait-Control '1 incoming commits' | Out-Null
    Wait-WindowState { Test-ControlInvokable 'Pull' } 'Pull remained disabled after the clean remote refresh.'
    Invoke-Control 'Pull'
    Invoke-Control 'Pull' -Within 'Git confirmation'
    Wait-Control 'Pull completed (fast-forward only)' | Out-Null
    if ((Get-Content (Join-Path $workspace 'incoming.txt') -Raw).Trim() -ne 'newest remote commit') { throw 'Pull did not update the selected branch.' }
    Capture-Window 'git-pulled'
    Invoke-FixtureGit $workspace @('checkout', '-b', 'conflict-peer') | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $workspace 'incoming.txt'), "peer conflict`n")
    Invoke-FixtureGit $workspace @('commit', '-am', 'peer conflict') | Out-Null
    Invoke-FixtureGit $workspace @('checkout', 'main') | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $workspace 'incoming.txt'), "local conflict`n")
    Invoke-FixtureGit $workspace @('commit', '-am', 'local conflict') | Out-Null
    & git -C $workspace merge conflict-peer 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) { throw 'Expected a synthetic merge conflict.' }
    Invoke-Control 'Refresh Git status'
    Wait-Control 'Review incoming.txt' | Out-Null
    Invoke-Control 'Review incoming.txt'
    Wait-Control 'Open in VS Code' | Out-Null
    Capture-Window 'git-conflicts'
    Select-Theme 'Dark'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'git-conflicts-dark-compact'
    # Task 019 owns compact header fit; keep both conflict body captures compact.
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1600, 1000, 0x0040) | Out-Null
    Select-Theme 'Light'
    [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
    Capture-Window 'git-conflicts-light-compact'
    Write-Output 'Git remote add/select/update/rename/remove, cancellation, activity logs, init, selected commits, diff/tree review, incoming detection, push, fast-forward pull, and conflict handoff passed using only local repositories.'
}
