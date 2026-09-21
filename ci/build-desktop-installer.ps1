param(
    [Parameter(Mandatory)][string]$Version,
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$BinaryDirectory,
    [string]$OutputDirectory,
    [string]$Compiler,
    [switch]$SmokeTest,
    [switch]$RequireStaticRuntime
)

$ErrorActionPreference = 'Stop'
if ($Version -cnotmatch '^[0-9]{1,4}(\.[0-9]{1,4}){3}$') {
    throw 'Version must be canonical N.N.N.N, with each component 0-9999.'
}
if (([version]$Version).ToString() -cne $Version) { throw 'Version must not contain leading zeros.' }
$root = Split-Path $PSScriptRoot -Parent
if (-not $BinaryDirectory) { $BinaryDirectory = Join-Path $root "target/$Target/release" }
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $root 'target/installer' }
$binary = Get-Item -LiteralPath (Join-Path $BinaryDirectory 'regressify-desktop.exe')
if ([version]$binary.VersionInfo.FileVersion -ne [version]$Version) {
    throw "Desktop executable version $($binary.VersionInfo.FileVersion) does not match installer version $Version."
}
if ($RequireStaticRuntime) {
    $locator = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $dumpbin = & $locator -latest -products '*' -find 'VC/Tools/MSVC/**/bin/Hostx64/x64/dumpbin.exe' | Select-Object -First 1
    if (-not $dumpbin) { throw 'dumpbin.exe is required to verify the installer runtime dependencies.' }
    $dependencies = & $dumpbin /DEPENDENTS $binary.FullName
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect executable dependencies.' }
    if ($dependencies -match '(?i)(vcruntime|msvcp)[0-9].*\.dll') {
        throw 'The standalone installer must use the static MSVC runtime.'
    }
}
if (-not $Compiler) {
    $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    $Compiler = if ($command) { $command.Source } else { "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" }
}
if (-not (Test-Path -LiteralPath $Compiler)) { throw 'Install Inno Setup 6 or pass -Compiler with the ISCC.exe path.' }
$source = $binary.Directory.FullName
$output = [System.IO.Path]::GetFullPath($OutputDirectory)
& $Compiler (Join-Path $root 'installer/regressify-desktop.iss') "/DAppVersion=$Version" "/DSourceDir=$source" "/DInstallerOutput=$output"
if ($LASTEXITCODE -ne 0) { throw "Installer compilation failed ($LASTEXITCODE)." }
$installer = Join-Path $output "Regressify-Desktop-Setup-$Version-x64.exe"
if (-not (Test-Path -LiteralPath $installer)) { throw 'The compiler did not create the expected installer.' }
if ($SmokeTest) {
    $registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{ED6F0908-7E11-4608-A532-E43482AA117B}_is1'
    if (Test-Path -LiteralPath $registration) { throw 'Installer smoke tests will not modify an existing Regressify Desktop installation.' }
    $desktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'Regressify Desktop.lnk'
    $programGroup = Join-Path ([Environment]::GetFolderPath('Programs')) 'Regressify Desktop'
    $startShortcut = Join-Path $programGroup 'Regressify Desktop.lnk'
    $contextMenus = @(
        @{ Path = 'HKCU:\Software\Classes\Directory\shell\RegressifyDesktop'; Argument = '%1' },
        @{ Path = 'HKCU:\Software\Classes\Directory\Background\shell\RegressifyDesktop'; Argument = '%V' }
    )
    foreach ($path in @($desktopShortcut, $programGroup) + @($contextMenus.Path)) {
        if (Test-Path -LiteralPath $path) { throw "Installer smoke tests will not modify existing shell integration: $path" }
    }
    $fixture = Join-Path $root ('target/installer-smoke-' + [guid]::NewGuid().ToString('N'))
    $destination = Join-Path $fixture 'app'
    [System.IO.Directory]::CreateDirectory($fixture) | Out-Null
    $arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/SP-', '/NORESTART', '/NORELAUNCH=1', ('/DIR="' + $destination + '"'))
    $uninstaller = Join-Path $destination 'unins000.exe'
    $installed = Join-Path $destination 'regressify-desktop.exe'
    $originalHash = (Get-FileHash -LiteralPath $binary.FullName -Algorithm SHA256).Hash
    function Invoke-InstallerFixture([string]$Path, [string[]]$Parameters) {
        $process = Start-Process -FilePath $Path -ArgumentList $Parameters -PassThru
        if (-not $process.WaitForExit(120000)) {
            $process.Kill($true)
            throw 'Installer fixture timed out.'
        }
        return $process.ExitCode
    }
    function Assert-InstallerShellIntegration {
        $shortcuts = New-Object -ComObject WScript.Shell
        $shell = New-Object -ComObject Shell.Application
        foreach ($path in @($desktopShortcut, $startShortcut)) {
            if (-not (Test-Path -LiteralPath $path)) { throw "Installer did not create shortcut: $path" }
            $shortcut = $shortcuts.CreateShortcut($path)
            if ($shortcut.TargetPath -ne $installed -or $shortcut.Arguments -ne '') {
                throw "Shortcut does not launch the installed desktop app: $path"
            }
            $item = $shell.NameSpace((Split-Path $path -Parent)).ParseName((Split-Path $path -Leaf))
            if ($item.ExtendedProperty('System.AppUserModel.ID') -cne 'Regressify.Desktop') {
                throw "Shortcut does not share the desktop app's Jump List identity: $path"
            }
        }
        foreach ($entry in $contextMenus) {
            $key = Get-Item -LiteralPath $entry.Path
            if ($key.GetValue('') -cne 'Open with Regressify Desktop') { throw 'Folder context-menu label is incorrect.' }
            if ($key.GetValue('Icon') -ne ('"' + $installed + '",0')) { throw 'Folder context-menu icon is incorrect.' }
            $command = (Get-Item -LiteralPath (Join-Path $entry.Path 'command')).GetValue('')
            $expected = '"' + $installed + '" "' + $entry.Argument + '\."'
            if ($command -cne $expected) { throw "Folder context-menu command is incorrect: $command" }
        }
    }
    try {
        $result = Invoke-InstallerFixture $installer ($arguments + ('/LOG="' + (Join-Path $fixture 'install.log') + '"'))
        if ($result -ne 0) { throw "Fresh installation failed ($result). See $fixture." }
        if ((Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash -ne $originalHash) { throw 'Installed executable differs from the built payload.' }
        Assert-InstallerShellIntegration
        $sentinel = Join-Path $destination 'user-workspace.txt'
        [System.IO.File]::WriteAllText($sentinel, 'User data must survive updates and uninstall.')
        $setupMutex = [System.Threading.Mutex]::new($false, 'Local\Regressify.Desktop.Setup.ED6F0908-7E11-4608-A532-E43482AA117B')
        try {
            $workerInput = Join-Path $fixture 'worker-input.json'
            $workerError = Join-Path $fixture 'worker-error.log'
            [System.IO.File]::WriteAllText($workerInput, '{}')
            $worker = Start-Process -FilePath $installed -ArgumentList '--internal-worker' -RedirectStandardInput $workerInput -RedirectStandardError $workerError -RedirectStandardOutput (Join-Path $fixture 'worker-output.log') -PassThru
            if (-not $worker.WaitForExit(15000)) {
                $worker.Kill($true)
                throw 'Worker did not reject startup during setup.'
            }
            if ($worker.ExitCode -eq 0 -or (Get-Content -LiteralPath $workerError -Raw) -notmatch 'Regressify setup is running') {
                throw 'Worker bypassed the setup guard.'
            }
        } finally { $setupMutex.Dispose() }
        $running = [System.Threading.Mutex]::new($false, 'Local\Regressify.Desktop.Running.ED6F0908-7E11-4608-A532-E43482AA117B')
        try {
            if ((Invoke-InstallerFixture $installer $arguments) -eq 0) { throw 'Installer ignored the running application mutex.' }
        } finally { $running.Dispose() }
        if ((Invoke-InstallerFixture $installer $arguments) -ne 0) { throw 'Reinstallation failed.' }
        Assert-InstallerShellIntegration
        if (-not (Test-Path -LiteralPath $sentinel)) { throw 'Reinstallation removed user data.' }
        $parts = @($Version.Split('.') | ForEach-Object { [int]$_ })
        for ($index = 3; $index -ge 0; $index--) {
            if ($parts[$index] -gt 0) {
                $parts[$index]--
                $older = $parts -join '.'
                & $Compiler /Qp (Join-Path $root 'installer/regressify-desktop.iss') "/DAppVersion=$older" "/DSourceDir=$source" "/DInstallerOutput=$fixture"
                if ($LASTEXITCODE -ne 0) { throw 'Cannot compile downgrade fixture.' }
                $olderInstaller = Join-Path $fixture "Regressify-Desktop-Setup-$older-x64.exe"
                $result = Invoke-InstallerFixture $olderInstaller ($arguments + ('/LOG="' + (Join-Path $fixture 'downgrade.log') + '"'))
                if ($result -ne 7) { throw "Downgrade must fail before copying files (got $result)." }
                break
            }
        }
        if ((Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash -ne $originalHash) { throw 'An unsuccessful installation changed the executable.' }
        Write-Output 'Fresh install, reinstallation, shortcuts, folder context menus, running-app protection and downgrade protection passed.'
    } finally {
        if (Test-Path -LiteralPath $uninstaller) {
            $result = Invoke-InstallerFixture $uninstaller @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART')
            if ($result -ne 0) { throw "Fixture uninstall failed ($result). See $fixture." }
            if (Test-Path -LiteralPath $installed) { throw 'Uninstall left the installed executable behind.' }
            if (Test-Path -LiteralPath $registration) { throw 'Uninstall left the fixture registration behind.' }
            foreach ($path in @($desktopShortcut, $startShortcut) + @($contextMenus.Path)) {
                if (Test-Path -LiteralPath $path) { throw "Uninstall left shell integration behind: $path" }
            }
            if ($sentinel -and -not (Test-Path -LiteralPath $sentinel)) { throw 'Uninstall removed user data.' }
            Write-Output 'Uninstall removed application files, shortcuts and context menus; user data was preserved.'
        }
    }
}
if ($env:GITHUB_OUTPUT) { "installer=$installer" | Out-File -LiteralPath $env:GITHUB_OUTPUT -Encoding utf8 -Append }
Write-Output $installer