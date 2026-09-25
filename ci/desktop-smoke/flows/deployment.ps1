if (($DeploymentFlows -or $PublishingFlows) -and -not ('RegressifySmokeCredentials' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class RegressifySmokeCredentials {
    [StructLayout(LayoutKind.Sequential)]
    struct Credential {
        public uint Flags, Type;
        public IntPtr TargetName, Comment;
        public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
        public uint CredentialBlobSize;
        public IntPtr CredentialBlob;
        public uint Persist, AttributeCount;
        public IntPtr Attributes, TargetAlias, UserName;
    }
    [DllImport("advapi32.dll", EntryPoint = "CredReadW", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool Read(string target, uint type, uint flags, out IntPtr credential);
    [DllImport("advapi32.dll", EntryPoint = "CredDeleteW", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool Delete(string target, uint type, uint flags);
    [DllImport("advapi32.dll")]
    static extern void CredFree(IntPtr credential);
    public static bool ExistsLocally(string target) {
        IntPtr pointer;
        if (!Read(target, 1, 0, out pointer)) {
            int error = Marshal.GetLastWin32Error();
            if (error == 1168) return false;
            throw new Win32Exception(error);
        }
        try {
            if (Marshal.PtrToStructure<Credential>(pointer).Persist != 2)
                throw new Exception("Fixture credential must be machine-local.");
            return true;
        } finally { CredFree(pointer); }
    }
    public static void Remove(string target) {
        if (Delete(target, 1, 0)) return;
        int error = Marshal.GetLastWin32Error();
        if (error != 1168) throw new Win32Exception(error);
    }
}
'@
}

function Clear-DeploymentFixtureSelection {
    Wait-Control 'Deselect all suites' | Out-Null
    if (Test-ControlInvokable 'Deselect all suites') {
        Invoke-Control 'Deselect all suites'
    }
}

function Test-DeploymentFlows {
    Clear-DeploymentFixtureSelection
    if (Test-ControlInvokable 'Deploy') { throw 'Deploy must be disabled without selected suites.' }
    Invoke-Control 'Deployment targets'
    Wait-Control 'No deployment targets' | Out-Null
    Capture-Window 'deploy-no-selection'
    Invoke-Control 'Add target'
    Write-FixtureControlText 'Target name' 'QA preview'
    Invoke-Control 'Save target'
    Wait-Control 'Enter a deployment token' | Out-Null
    Invoke-Control 'Deployment token' -Pointer
    Wait-Control 'Deployment token' | Out-Null
    $syntheticToken = ('a' * 64) + '-' + ('b' * 36) + '000'
    Send-FixtureText $syntheticToken
    $tokenInput = Wait-Control 'Deployment token'
    if (-not $tokenInput.Current.IsPassword) { throw 'Deployment token must be a protected password field.' }
    $valuePattern = $null
    if ($tokenInput.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$valuePattern)) {
        if (([System.Windows.Automation.ValuePattern]$valuePattern).Current.Value -eq $syntheticToken) { throw 'The token is exposed through accessibility.' }
    }
    Write-FixtureControlText 'Azure environment' 'preview'
    Capture-Window 'deploy-target-editor'
    Invoke-Control 'Save target'
    Wait-Control 'QA preview' | Out-Null
    $targetSettingsPath = Get-FixtureDeploymentSettingsPath
    $targetSettings = Get-Content -LiteralPath $targetSettingsPath -Raw
    if ($targetSettings.Contains($syntheticToken) -or $targetSettings.Contains('tokenEnv')) { throw 'Target settings must contain only a secure credential reference.' }
    $credentialId = ($targetSettings | ConvertFrom-Json).targets[0].credentialId
    $credentialName = Get-FixtureCredentialName $credentialId
    if (-not [RegressifySmokeCredentials]::ExistsLocally($credentialName)) { throw 'The target token was not saved to the machine-local credential store.' }
    Invoke-Control 'Edit target'
    Write-FixtureControlText 'Target name' 'QA reports'
    Invoke-Control 'Save target'
    Wait-Control 'QA reports' | Out-Null
    if (-not [RegressifySmokeCredentials]::ExistsLocally($credentialName)) { throw 'Editing a target must preserve its saved token.' }
    Invoke-Control 'Close'
    $script:process.CloseMainWindow() | Out-Null
    if (-not $script:process.WaitForExit(10000)) { throw 'Deployment fixture did not close normally.' }
    Start-Fixture
    Invoke-Control 'Deployment targets'
    Wait-Control 'QA reports' | Out-Null
    if (-not [RegressifySmokeCredentials]::ExistsLocally($credentialName)) { throw 'The saved token did not survive an app restart.' }
    Capture-Window 'deploy-target-restored'
    Invoke-Control 'Close'
    Clear-DeploymentFixtureSelection
    Invoke-Control 'smoke.tests.json'
    Wait-Control 'Run Suite Reference' | Out-Null
    Invoke-Control 'Deploy'
    Wait-Control 'Yes, deploy only these 1 of 2 suites' | Out-Null
    if (Test-ControlInvokable 'Deploy selected reports') { throw 'Subset deployment must require confirmation.' }
    Capture-Window 'deploy-partial-selection'
    foreach ($theme in @('Dark', 'Light')) {
        Invoke-Control 'Close'
        Select-Theme $theme
        [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
        Capture-Window ('compact-titlebar-' + $theme.ToLowerInvariant())
        Invoke-Control 'Deploy'
        Wait-Control 'Yes, deploy only these 1 of 2 suites' | Out-Null
        Capture-Window ('deploy-' + $theme.ToLowerInvariant() + '-compact')
    }
    Invoke-Control 'Yes, deploy only these 1 of 2 suites'
    if (-not (Test-ControlInvokable 'Deploy selected reports')) { throw 'Confirmed deployment must be invokable.' }
    [RegressifySmokeCredentials]::Remove($credentialName)
    Invoke-Control 'Deploy selected reports'
    Wait-Control 'Deployment failed' | Out-Null
    Wait-Control 'Report deployment' | Out-Null
    Wait-Control 'Deployment log' | Out-Null
    if ($null -ne (Find-Control 'Scenarios / viewports: 0')) { throw 'Deployment activity must not show scenario counters.' }
    foreach ($theme in @('Light', 'Dark')) {
        Select-Theme $theme
        [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
        Capture-Window ('deploy-outcome-' + $theme.ToLowerInvariant())
        [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
        Capture-Window ('deploy-outcome-' + $theme.ToLowerInvariant() + '-compact')
    }
    Invoke-Control 'View Log'
    Wait-SourceText 'No saved deployment token' 'Run log'
    Capture-Window 'deploy-missing-token'
    Invoke-Control 'Deployment targets'
    Wait-Control 'QA reports' | Out-Null
    Invoke-Control 'Edit target'
    Invoke-Control 'Deployment token' -Pointer
    Wait-Control 'Deployment token' | Out-Null
    Send-FixtureText $syntheticToken
    Invoke-Control 'Save target'
    Wait-Control 'QA reports' | Out-Null
    Wait-WindowState { $null -eq (Find-Control 'Save target') -and [RegressifySmokeCredentials]::ExistsLocally($credentialName) } 'A missing credential must be replaceable.'
    Invoke-Control 'Remove target'
    Wait-Control 'Keep target' | Out-Null
    Capture-Window 'deploy-remove-confirmation'
    Invoke-Control 'Keep target'
    Invoke-Control 'Remove target'
    Invoke-Control 'Confirm remove target'
    Wait-Control 'No deployment targets' | Out-Null
    if ([RegressifySmokeCredentials]::ExistsLocally($credentialName)) { throw 'Removing a target must delete its saved credential.' }
    Invoke-Control 'Close'
    Invoke-Control 'Deployment targets'
    Invoke-Control 'Add target'
    Invoke-Control 'Deployment provider' -Pointer
    Wait-Control 'Netlify' | Out-Null
    Capture-Window 'deploy-provider-menu'
    Invoke-Control 'Netlify'
    Wait-Control 'Netlify Site ID' | Out-Null
    if ($null -ne (Find-Control 'Azure environment')) { throw 'Netlify must not expose Azure-only settings.' }
    Write-FixtureControlText 'Target name' 'Netlify QA'
    Invoke-Control 'Save target'
    Wait-Control 'Enter the Netlify Site ID (Project ID) from project configuration' | Out-Null
    $netlifySiteId = [guid]::NewGuid().ToString('D')
    Write-FixtureControlText 'Netlify Site ID' $netlifySiteId
    Invoke-Control 'Personal access token' -Pointer
    Wait-Control 'Personal access token' | Out-Null
    $netlifyToken = 'synthetic-netlify-token-' + [guid]::NewGuid().ToString('N')
    Send-FixtureText $netlifyToken
    $tokenInput = Wait-Control 'Personal access token'
    if (-not $tokenInput.Current.IsPassword) { throw 'Netlify tokens must be protected password fields.' }
    Capture-Window 'deploy-netlify-editor'
    Invoke-Control 'Save target'
    Wait-Control 'Netlify QA' | Out-Null
    $targetSettings = Get-Content -LiteralPath $targetSettingsPath -Raw
    $target = ($targetSettings | ConvertFrom-Json).targets[0]
    if ($target.provider -ne 'netlify' -or $target.siteId -ne $netlifySiteId -or $targetSettings.Contains($netlifyToken)) { throw 'Netlify settings must persist their provider and Site ID without the token.' }
    $credentialName = Get-FixtureCredentialName $target.credentialId
    if (-not [RegressifySmokeCredentials]::ExistsLocally($credentialName)) { throw 'Netlify token was not saved securely.' }
    Invoke-Control 'Edit target'
    Invoke-Control 'Deployment provider' -Pointer
    Invoke-Control 'Azure Static Web Apps'
    Wait-Control 'Azure environment' | Out-Null
    Invoke-Control 'Save target'
    Wait-Control 'Enter a new deployment token when changing providers' | Out-Null
    Invoke-Control 'Cancel edit'
    Invoke-Control 'Close'
    $script:process.CloseMainWindow() | Out-Null
    if (-not $script:process.WaitForExit(10000)) { throw 'Netlify fixture did not close normally.' }
    Start-Fixture
    Invoke-Control 'Deployment targets'
    Wait-Control 'Netlify QA' | Out-Null
    if (-not [RegressifySmokeCredentials]::ExistsLocally($credentialName)) { throw 'Netlify token did not survive restart.' }
    Capture-Window 'deploy-netlify-restored'
    Invoke-Control 'Close'
    foreach ($theme in @('Light', 'Dark')) {
        Select-Theme $theme
        [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, [Math]::Min(1600, $screen.Width - 40), [Math]::Min(1000, $screen.Height - 40), 0x0040) | Out-Null
        Invoke-Control 'Deployment targets'
        Invoke-Control 'Edit target'
        Wait-Control 'Netlify Site ID' | Out-Null
        if ((Read-ControlText 'Netlify Site ID') -ne $netlifySiteId) { throw 'Saved Netlify Site ID was not restored.' }
        Capture-Window ('deploy-netlify-' + $theme.ToLowerInvariant())
        [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, 1080, 720, 0x0040) | Out-Null
        Capture-Window ('deploy-netlify-' + $theme.ToLowerInvariant() + '-compact')
        Invoke-Control 'Cancel edit'
        Invoke-Control 'Close'
    }
    [RegressifySmokeCredentials]::Remove($credentialName)
    Clear-DeploymentFixtureSelection
    Invoke-Control 'smoke.tests.json'
    Wait-Control 'Run Suite Reference' | Out-Null
    Invoke-Control 'Deploy'
    Wait-Control 'Yes, deploy only these 1 of 2 suites' | Out-Null
    Invoke-Control 'Yes, deploy only these 1 of 2 suites'
    Invoke-Control 'Deploy selected reports'
    Wait-Control 'Deployment failed' | Out-Null
    Wait-Control 'Report deployment' | Out-Null
    Wait-Control 'Deployment log' | Out-Null
    Capture-Window 'deploy-netlify-outcome'
    Invoke-Control 'View Log'
    Wait-SourceText 'No saved deployment token' 'Run log'
    Capture-Window 'deploy-netlify-missing-token'
    Invoke-Control 'Deployment targets'
    Invoke-Control 'Edit target'
    Invoke-Control 'Personal access token' -Pointer
    Wait-Control 'Personal access token' | Out-Null
    Send-FixtureText $netlifyToken
    Invoke-Control 'Save target'
    Wait-WindowState { $null -eq (Find-Control 'Save target') -and [RegressifySmokeCredentials]::ExistsLocally($credentialName) } 'Netlify token replacement did not finish.'
    Invoke-Control 'Remove target'
    Invoke-Control 'Confirm remove target'
    Wait-Control 'No deployment targets' | Out-Null
    if ([RegressifySmokeCredentials]::ExistsLocally($credentialName)) { throw 'Netlify credential was not removed with its target.' }
    Invoke-Control 'Close'
    Write-Output 'Azure and Netlify provider selection, masked credentials, machine-local storage, restart persistence, credential deletion, subset guards, and safe missing-token failures passed without remote deployment.'
}
