function Open-PublishingMenu([string]$Name) {
    Invoke-Control 'Publish' -Pointer
    Invoke-Control $Name -Pointer
}

function Remember-PublishingCredential([string]$Name) {
    $path = Join-Path (Split-Path -Parent $settingsPath) 'deployment-tokens.json'
    $token = (Get-Content -LiteralPath $path -Raw | ConvertFrom-Json).tokens | Where-Object name -eq $Name
    if ($null -eq $token) { throw 'The synthetic named token was not persisted.' }
    $credential = "named:$($token.id).Regressify Desktop Deployment"
    $script:publishingCredentials.Add($credential)
    if (-not [RegressifySmokeCredentials]::ExistsLocally($credential)) { throw 'Named token was not saved in the machine-local credential store.' }
    return $token
}

function Write-PublishingField([string]$Name, [string]$Text) {
    $viewport = (Wait-Control 'Landing editor').Current.BoundingRectangle
    Focus-Fixture
    [RegressifySmokePointer]::SetPhysicalCursorPos([int]($viewport.Right - 3), [int]($viewport.Top + $viewport.Height / 2)) | Out-Null
    [RegressifySmokeNative]::mouse_event(0x0800, 0, 0, 2400, [UIntPtr]::Zero)
    [RegressifySmokeNative]::DwmFlush() | Out-Null
    $deadline = [System.Diagnostics.Stopwatch]::StartNew()
    do {
        $viewport = (Wait-Control 'Landing editor').Current.BoundingRectangle
        $control = Find-Control $Name
        if ($null -ne $control) {
            $bounds = $control.Current.BoundingRectangle
            if ($bounds.Width -gt 0 -and $bounds.Height -gt 0 -and $bounds.Top -ge $viewport.Top -and $bounds.Bottom -le $viewport.Bottom) {
                Write-FixtureControlText $Name $Text
                return
            }
        }
        Focus-Fixture
        [RegressifySmokePointer]::SetPhysicalCursorPos([int]($viewport.Right - 3), [int]($viewport.Top + $viewport.Height / 2)) | Out-Null
        [RegressifySmokeNative]::mouse_event(0x0800, 0, 0, ([uint32]::MaxValue - 119), [UIntPtr]::Zero)
        [RegressifySmokeNative]::DwmFlush() | Out-Null
    } while ($deadline.Elapsed.TotalSeconds -lt 10)
    throw "Landing field '$Name' could not be scrolled into view."
}

function Test-PublishingFlows {
    if ($KeepOpen) { throw 'Publishing smoke requires cleanup; do not combine -PublishingFlows with -KeepOpen.' }
    $tokenName = 'Smoke Netlify ' + [guid]::NewGuid().ToString('N')
    $cloudflareName = 'Smoke Cloudflare ' + [guid]::NewGuid().ToString('N')
    $synthetic = 'synthetic-publishing-token-' + [guid]::NewGuid().ToString('N')
    Open-PublishingMenu 'Deployment tokens'
    Invoke-Control 'Add named token'
    Write-FixtureControlText 'Token name' $tokenName
    Invoke-Control 'Token' -Pointer
    Wait-Control 'Token' | Out-Null
    Send-FixtureText $synthetic
    if ((Read-ControlText 'Token name') -ne $tokenName) { throw 'Typing a token unexpectedly changed the token name.' }
    if (-not (Wait-Control 'Token').Current.IsPassword) { throw 'Named deployment tokens must be masked.' }
    Invoke-Control 'Save token'
    try { Wait-Control $tokenName -TimeoutSeconds 15 | Out-Null } catch {
        $catalog = Join-Path (Split-Path -Parent $settingsPath) 'deployment-tokens.json'
        if (Test-Path -LiteralPath $catalog) { Write-JsonFixture (Join-Path $workspace 'publishing-token-metadata.json') (Get-Content -LiteralPath $catalog -Raw | ConvertFrom-Json) }
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($script:window)
        $status = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::AutomationIdProperty, 'named-token-status'))
        if ($null -ne $status) { Write-Output ('Token manager status: ' + $status.Current.Name) }
        $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
            Where-Object { $_.Current.Name -match '^(Enter|Cannot|Invalid|Deployment tokens changed|Token name|Create a new named)' } |
            ForEach-Object { Write-Output ('Publishing validation: ' + $_.Current.Name) }
        throw
    }
    $netlify = Remember-PublishingCredential $tokenName
    $catalogPath = Join-Path (Split-Path -Parent $settingsPath) 'deployment-tokens.json'
    if ((Get-Content -LiteralPath $catalogPath -Raw).Contains($synthetic)) { throw 'The token catalog exposed a secret.' }
    Invoke-Control 'Edit named token'
    Write-FixtureControlText 'Token name' ($tokenName + ' renamed')
    Invoke-Control 'Save token'
    $tokenName += ' renamed'
    Wait-Control $tokenName | Out-Null
    Invoke-Control 'Add named token' -Pointer
    try { Wait-Control 'Token provider' -TimeoutSeconds 10 | Out-Null } catch {
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($script:window)
        $root.FindAll([System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)) |
            Where-Object { -not $_.Current.IsOffscreen } |
            ForEach-Object { Write-Output ("Publishing button: {0}, enabled={1}" -f $_.Current.Name, $_.Current.IsEnabled) }
        throw
    }
    Invoke-Control 'Token provider' -Pointer
    Invoke-Control 'Cloudflare Workers' -Pointer
    Write-FixtureControlText 'Token name' $cloudflareName
    Write-FixtureControlText 'Cloudflare Account ID' ('a' * 32)
    Invoke-Control 'Token' -Pointer
    Wait-Control 'Token' | Out-Null
    Send-FixtureText $synthetic
    Capture-Window 'publishing-token-cloudflare'
    Invoke-Control 'Save token'
    Wait-Control $cloudflareName | Out-Null
    $cloudflare = Remember-PublishingCredential $cloudflareName
    Invoke-Control 'Close'

    Open-PublishingMenu 'Landing apps'
    Write-PublishingField 'Landing heading' 'Customer visual sites'
    Write-PublishingField 'Landing subheading Markdown' '**Customer overview**'
    Invoke-Control 'Add site'
    Write-PublishingField 'Site 1 heading' 'Report owned by another user'
    Write-PublishingField 'Site 1 subheading Markdown' '*Remote report*'
    Write-PublishingField 'Site 1 URL' 'https://someone-else.example/suites/shop'
    Invoke-Control 'Save'
    Wait-Control 'Customer visual sites' | Out-Null
    $landingCatalog = Join-Path (Split-Path -Parent $settingsPath) 'landing-apps.json'
    $landingPath = (Get-Content -LiteralPath $landingCatalog -Raw | ConvertFrom-Json).files[0]
    if ([System.IO.Path]::GetDirectoryName($landingPath).Replace('\\?\', '') -ne (Join-Path (Split-Path -Parent $settingsPath) 'landing-apps')) { throw 'Landing definitions must be managed inside the application settings directory.' }
    Wait-WindowState { (Test-Path -LiteralPath $landingPath) -and (Test-ControlInvokable 'Landing deployment settings') } 'Landing file was not saved.'
    $landing = Get-Content -LiteralPath $landingPath -Raw | ConvertFrom-Json
    $script:publishingLandingIds.Add($landing.id)
    if ($landing.subheading -ne '**Customer overview**' -or $landing.sites[0].url -ne 'https://someone-else.example/suites/shop') { throw 'Landing save changed Markdown or remote URLs.' }
    Capture-Window 'publishing-landing-editor'
    Write-PublishingField 'Landing heading' 'Unsaved landing draft'
    Invoke-Control 'Close'
    Wait-Control 'Keep editing' | Out-Null
    Invoke-Control 'Keep editing'
    if ((Read-ControlText 'Landing heading') -ne 'Unsaved landing draft') { throw 'Landing guard lost the draft.' }
    Invoke-Control 'Close'
    Invoke-Control 'Discard changes'
    Open-PublishingMenu 'Landing apps'
    Wait-SourceText 'Customer visual sites' 'Landing heading'
    Invoke-Control 'Landing deployment settings'
    Wait-Control 'Deploy landing page' | Out-Null
    Invoke-Control 'Add target'
    Invoke-Control 'Deployment provider' -Pointer
    Invoke-Control 'Cloudflare Workers' -Pointer
    Write-FixtureControlText 'Target name' 'Smoke landing worker'
    Write-FixtureControlText 'Worker name' 'synthetic-landing-worker'
    Invoke-Control 'Target-specific token' -Pointer
    Invoke-Control $cloudflareName -Pointer
    Wait-WindowState { $null -eq (Find-Control 'API token') -and $null -eq (Find-Control 'Cloudflare Account ID') } 'Named Cloudflare tokens must supply both token and account.'
    Invoke-Control 'Save target'
    Wait-Control 'Smoke landing worker' | Out-Null
    if (-not (Test-ControlInvokable 'Deploy landing page')) { throw 'Saved landing deployment target must be deployable without suites.' }
    Capture-Window 'publishing-landing-target'
    Invoke-Control 'Remove target'
    Invoke-Control 'Confirm remove target'
    if (-not [RegressifySmokeCredentials]::ExistsLocally("named:$($cloudflare.id).Regressify Desktop Deployment")) { throw 'Removing a target deleted its reusable token.' }
    Invoke-Control 'Close'
    Wait-SourceText 'Customer visual sites' 'Landing heading'
    Invoke-Control 'Close'

    foreach ($theme in @('Light', 'Dark')) {
        Select-Theme $theme
        foreach ($dimensions in @(@(1080, 720), @([Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40)))) {
            [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, $dimensions[0], $dimensions[1], 0x0040) | Out-Null
            Open-PublishingMenu 'Landing apps'
            Wait-Control 'Landing heading' | Out-Null
            Capture-Window ('publishing-landing-' + $theme.ToLowerInvariant() + '-' + $dimensions[0])
            Invoke-Control 'Close'
            Open-PublishingMenu 'Deployment tokens'
            Wait-Control 'Add named token' | Out-Null
            Capture-Window ('publishing-tokens-' + $theme.ToLowerInvariant() + '-' + $dimensions[0])
            Invoke-Control 'Close'
        }
    }

    Open-PublishingMenu 'Landing apps'
    Invoke-Control 'Remove landing app'
    Invoke-Control 'Keep editing'
    if (-not (Test-Path -LiteralPath $landingPath)) { throw 'Cancelling removal deleted the landing file.' }
    Invoke-Control 'Remove landing app'
    Invoke-Control 'Remove locally'
    Wait-Control 'New landing app' | Out-Null
    if (Test-Path -LiteralPath $landingPath) { throw 'Confirmed local landing removal did not remove the definition.' }
    Invoke-Control 'Close'
    Open-PublishingMenu 'Deployment tokens'
    Invoke-Control 'Remove named token'
    Invoke-Control 'Keep token'
    Invoke-Control 'Remove named token'
    Invoke-Control 'Remove token'
    Invoke-Control 'Close'
    Write-Output 'Landing file management, Markdown, remote URLs, draft guards, named-token masking/reuse, Cloudflare account selection, and light/dark compact/large controls passed without remote deployment.'
}