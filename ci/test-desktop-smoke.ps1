<#
.SYNOPSIS
Validate smoke modules, dispatch, and cleanup without building or opening Windows UI.
#>
$ErrorActionPreference = 'Stop'

function Read-SmokeScript([string]$Path) {
    $tokens = $null
    $parseErrors = $null
    $parsed = [System.Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$parseErrors)
    if ($parseErrors.Count) { throw "$Path`: $($parseErrors | Out-String)" }
    return $parsed
}

function Assert-SmokeEqual($Actual, $Expected, [string]$Message) {
    if (($Actual -join '|') -cne ($Expected -join '|')) {
        throw "$Message`: expected '$($Expected -join ', ')', got '$($Actual -join ', ')'."
    }
}

$entry = Read-SmokeScript (Join-Path $PSScriptRoot 'desktop-smoke.ps1')
$parameters = @('SkipBuild', 'KeepOpen', 'Capture', 'ScenarioFlows', 'ArtifactFlows', 'InstanceFlows', 'UpdateFlows', 'DeploymentFlows', 'PublishingFlows', 'TitlebarFlows', 'GitFlows', 'ComparisonFlows', 'ErrorImageFlows', 'IconFlows', 'SchemaFlows')
Assert-SmokeEqual @($entry.ParamBlock.Parameters.Name.VariablePath.UserPath) $parameters 'Public switches changed'
foreach ($parameter in $entry.ParamBlock.Parameters) {
    Assert-SmokeEqual $parameter.StaticType.Name 'SwitchParameter' "Parameter $($parameter.Name) is not a switch"
}

$imports = $entry.FindAll({
    param($node)
    $node -is [System.Management.Automation.Language.CommandAst] -and $node.InvocationOperator -eq [System.Management.Automation.Language.TokenKind]::Dot
}, $true)
$loaded = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
$definitions = @{}
$keyboardSource = $null
foreach ($import in $imports) {
    $join = $import.CommandElements[0].Pipeline.PipelineElements[0]
    Assert-SmokeEqual $join.GetCommandName() 'Join-Path' 'Module imports must resolve relative to the entry point'
    Assert-SmokeEqual $join.CommandElements[1].VariablePath.UserPath 'PSScriptRoot' 'Module imports depend on the current directory'
    $path = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot $join.CommandElements[2].Value))
    if (-not $loaded.Add($path)) { throw "Module imported more than once: $path" }
    $module = Read-SmokeScript $path
    if ([System.IO.Path]::GetFileName($path) -eq 'native.ps1') {
        $keyboardDefinitions = @($module.FindAll({
            param($node)
            $node -is [System.Management.Automation.Language.StringConstantExpressionAst] -and $node.Value.Contains('public static class RegressifySmokeText {')
        }, $true))
        Assert-SmokeEqual $keyboardDefinitions.Count 1 'Native keyboard implementation is missing or ambiguous'
        $keyboardSource = $keyboardDefinitions[0].Value
    }
    foreach ($definition in $module.EndBlock.Statements | Where-Object { $_ -is [System.Management.Automation.Language.FunctionDefinitionAst] }) {
        if ($definitions.ContainsKey($definition.Name)) { throw "Duplicate helper: $($definition.Name)" }
        $definitions[$definition.Name] = $definition
        . ([scriptblock]::Create($definition.Extent.Text))
    }
}
foreach ($file in Get-ChildItem (Join-Path $PSScriptRoot 'desktop-smoke') -Recurse -Filter '*.ps1') {
    if (-not $loaded.Contains($file.FullName)) { throw "Smoke module is not imported: $($file.FullName)" }
}
foreach ($name in $definitions.Keys) {
    if (-not (Get-Command $name -CommandType Function -ErrorAction SilentlyContinue)) { throw "Helper did not load: $name" }
}

$stockSizingChecks = 0
foreach ($sizingCase in @(
    @{ Flow = 'Test-TitlebarFlows'; Compact = @('') },
    @{ Flow = 'Test-DefaultFlows'; Compact = @('1080x720') },
    @{ Flow = 'Test-SchemaFlows'; Compact = @('1000x720', '1000x720', '1000x720') },
    @{ Flow = 'Test-GitFlows'; Compact = @('1080x720', '1080x720', '1080x720', '') },
    @{ Flow = 'Test-ComparisonFlows'; Compact = @('1080x720') },
    @{ Flow = 'Test-ErrorImageFlows'; Compact = @('1080x720') },
    @{ Flow = 'Test-ArtifactFlows'; Compact = @('') },
    @{ Flow = 'Test-PublishingFlows'; Compact = @('1080x720') },
    @{ Flow = 'Test-DeploymentFlows'; Compact = @('1080x720', '1080x720', '1080x720') }
)) {
    $matrices = @($definitions[$sizingCase.Flow].FindAll({
        param($node)
        $node -is [System.Management.Automation.Language.ForEachStatementAst] -and
            $node.Variable.VariablePath.UserPath -in @('dimensions', 'windowSize')
    }, $true))
    Assert-SmokeEqual $matrices.Count $sizingCase.Compact.Count "$($sizingCase.Flow) lost a stock-sizing matrix"
    for ($matrixIndex = 0; $matrixIndex -lt $matrices.Count; $matrixIndex++) {
        $matrix = $matrices[$matrixIndex]
        $sizes = @($matrix.Condition.PipelineElements[0].Expression.SafeGetValue() | ForEach-Object { $_ -join 'x' })
        foreach ($requiredSize in @('1000x640', '1600x1000', $sizingCase.Compact[$matrixIndex]) | Where-Object { $_ }) {
            Assert-SmokeEqual ($sizes -contains $requiredSize) $true "$($sizingCase.Flow) must retain $requiredSize"
        }
        $themes = @($matrix.Body.Statements | Where-Object {
            $_ -is [System.Management.Automation.Language.ForEachStatementAst] -and $_.Variable.VariablePath.UserPath -eq 'theme'
        })
        Assert-SmokeEqual $themes.Count 1 "$($sizingCase.Flow) must switch themes inside each size"
        Assert-SmokeEqual @($themes[0].Condition.PipelineElements[0].Expression.SafeGetValue() | Sort-Object) @('Dark', 'Light') 'Every stock-sizing pass must cover both themes'
        $resizes = @($matrix.Body.FindAll({
            param($node)
            $node -is [System.Management.Automation.Language.InvokeMemberExpressionAst] -and $node.Member.Value -eq 'SetWindowPos'
        }, $true))
        Assert-SmokeEqual $resizes.Count 1 "$($sizingCase.Flow) must not widen inside its theme loop"
        Assert-SmokeEqual ($resizes[0].Extent.StartOffset -lt $themes[0].Extent.StartOffset) $true 'Resize must precede the theme loop'
        Assert-SmokeEqual $resizes[0].Arguments[4].Extent.Text ('$' + $matrix.Variable.VariablePath.UserPath + '[0]') 'Resize must use the matrix width'
        Assert-SmokeEqual $resizes[0].Arguments[5].Extent.Text ('$' + $matrix.Variable.VariablePath.UserPath + '[1]') 'Resize must use the matrix height'
        $selections = @($themes[0].Body.FindAll({
            param($node)
            $node -is [System.Management.Automation.Language.CommandAst] -and $node.GetCommandName() -eq 'Select-Theme'
        }, $true))
        Assert-SmokeEqual $selections.Count 1 'Each theme iteration must select its requested appearance'
        Assert-SmokeEqual $selections[0].CommandElements[1].Extent.Text '$theme' 'Theme selection must use the current matrix theme'
        $stockSizingChecks++
    }
}

$headerStatements = $definitions['Test-TitlebarControlBounds'].Body.EndBlock.Statements
$versionGuards = @($headerStatements | Where-Object {
    $_ -is [System.Management.Automation.Language.IfStatementAst] -and @($_.Clauses[0].Item1.FindAll({
        param($node)
        $node -is [System.Management.Automation.Language.VariableExpressionAst] -and $node.VariablePath.UserPath -eq 'clientWidth'
    }, $true)).Count -gt 0 -and @($_.Clauses[0].Item1.FindAll({
        param($node)
        $node -is [System.Management.Automation.Language.VariableExpressionAst] -and $node.VariablePath.UserPath -eq 'versions'
    }, $true)).Count -gt 0
})
Assert-SmokeEqual $versionGuards.Count 1 'Native version visibility must check the client-width breakpoint'
$versionProbe = [scriptblock]::Create($versionGuards[0].Extent.Text)
foreach ($versionCase in @(
    @{ Width = 1000; Count = 0; Reject = $false },
    @{ Width = 1079; Count = 0; Reject = $false },
    @{ Width = 1079; Count = 1; Reject = $true },
    @{ Width = 1080; Count = 1; Reject = $false },
    @{ Width = 1080; Count = 0; Reject = $true },
    @{ Width = 1081; Count = 1; Reject = $false },
    @{ Width = 1081; Count = 0; Reject = $true },
    @{ Width = 1399; Count = 1; Reject = $false },
    @{ Width = 1400; Count = 1; Reject = $false },
    @{ Width = 1600; Count = 1; Reject = $false },
    @{ Width = 1600; Count = 2; Reject = $true }
)) {
    $clientWidth = $versionCase.Width
    $versions = [object[]]::new($versionCase.Count)
    $rejected = $false
    try { & $versionProbe } catch { $rejected = $true }
    Assert-SmokeEqual $rejected $versionCase.Reject 'Native title-bar version visibility must use the exact 1080px breakpoint'
    $stockSizingChecks++
}
$workspaceGuards = @($headerStatements | Where-Object {
    $_ -is [System.Management.Automation.Language.IfStatementAst] -and @($_.Clauses[0].Item1.FindAll({
        param($node)
        $node -is [System.Management.Automation.Language.VariableExpressionAst] -and $node.VariablePath.UserPath -eq 'clientWidth'
    }, $true)).Count -gt 0 -and @($_.Clauses[0].Item1.FindAll({
        param($node)
        $node -is [System.Management.Automation.Language.VariableExpressionAst] -and $node.VariablePath.UserPath -eq 'workspaceControl'
    }, $true)).Count -gt 0
})
Assert-SmokeEqual $workspaceGuards.Count 1 'Native workspace visibility must check the client-width breakpoint'
$workspaceProbe = [scriptblock]::Create($workspaceGuards[0].Extent.Text)
foreach ($workspaceCase in @(
    @{ Width = 1000; Visible = $false; Reject = $false },
    @{ Width = 1399; Visible = $false; Reject = $false },
    @{ Width = 1399; Visible = $true; Reject = $true },
    @{ Width = 1400; Visible = $true; Reject = $false },
    @{ Width = 1400; Visible = $false; Reject = $true },
    @{ Width = 1600; Visible = $true; Reject = $false },
    @{ Width = 1600; Visible = $false; Reject = $true }
)) {
    $clientWidth = $workspaceCase.Width
    $workspaceControl = if ($workspaceCase.Visible) { [object]::new() } else { $null }
    $rejected = $false
    try { & $workspaceProbe } catch { $rejected = $true }
    Assert-SmokeEqual $rejected $workspaceCase.Reject 'Native workspace visibility must use the exact 1400px breakpoint'
    $stockSizingChecks++
}
$headerMenus = @($definitions['Test-TitlebarControlBounds'].FindAll({
    param($node)
    $node -is [System.Management.Automation.Language.StringConstantExpressionAst] -and
        $node.Value -in @('File', 'Edit', 'Publish', 'Help')
}, $true) | ForEach-Object { $_.Value })
Assert-SmokeEqual $headerMenus @('File', 'Publish', 'Help') 'Native header checks must match the actual application menu catalog'
$windowSizingStatements = @($headerStatements | Where-Object {
    ($_ -is [System.Management.Automation.Language.AssignmentStatementAst] -and $_.Left.Extent.Text -in @('$expectedWidth', '$expectedHeight')) -or
    ($_ -is [System.Management.Automation.Language.IfStatementAst] -and @($_.Clauses[0].Item1.FindAll({
        param($node)
        $node -is [System.Management.Automation.Language.VariableExpressionAst] -and $node.VariablePath.UserPath -eq 'expectedWidth'
    }, $true)).Count -gt 0)
})
Assert-SmokeEqual $windowSizingStatements.Count 3 'Native outer-size validation is missing or ambiguous'
$windowSizingProbe = [scriptblock]::Create(($windowSizingStatements.Extent.Text -join [Environment]::NewLine))
foreach ($sizingCase in @(
    @{ Dimensions = @(1000, 640); Width = 1016; Height = 648; Scale = 1; Border = 8; Reject = $false },
    @{ Dimensions = @(1600, 1000); Width = 1600; Height = 1000; Scale = 1; Border = 8; Reject = $false },
    @{ Dimensions = @(1000, 640); Width = 1216; Height = 728; Scale = 1; Border = 8; Reject = $true },
    @{ Dimensions = @(1000, 640); Width = 1000; Height = 640; Scale = 1; Border = 8; Reject = $true },
    @{ Dimensions = @(1000, 640); Width = 1270; Height = 810; Scale = 1.25; Border = 10; Reject = $false }
)) {
    $Dimensions = $sizingCase.Dimensions
    $scale = $sizingCase.Scale
    $border = $sizingCase.Border
    $bounds = [pscustomobject]@{ Left = 20; Top = 20; Right = 20 + $sizingCase.Width; Bottom = 20 + $sizingCase.Height }
    $rejected = $false
    try { & $windowSizingProbe } catch { $rejected = $true }
    Assert-SmokeEqual $rejected $sizingCase.Reject 'Native captures must enforce the chosen client minimum and actual outer dimensions'
    $stockSizingChecks++
}
$reserve = @($headerStatements | Where-Object { $_ -is [System.Management.Automation.Language.AssignmentStatementAst] -and $_.Left.Extent.Text -eq '$rightLimit' })
Assert-SmokeEqual $reserve.Count 1 'Native window-control reserve is missing or ambiguous'
$controlGuards = @($definitions['Test-TitlebarControlBounds'].FindAll({
    param($node)
    $node -is [System.Management.Automation.Language.IfStatementAst] -and @($node.Clauses[0].Item1.FindAll({
        param($conditionNode)
        $conditionNode -is [System.Management.Automation.Language.VariableExpressionAst] -and $conditionNode.VariablePath.UserPath -eq 'rectangle'
    }, $true)).Count -gt 0
}, $true))
Assert-SmokeEqual $controlGuards.Count 2 'Header action and Git bounds must both be checked'
for ($guardIndex = 0; $guardIndex -lt $controlGuards.Count; $guardIndex++) {
    $boundsProbe = [scriptblock]::Create($reserve[0].Extent.Text + [Environment]::NewLine + $controlGuards[$guardIndex].Extent.Text)
    foreach ($sizingCase in @(
        @{ Left = 860; Top = 23; Width = 32; Height = 32; Reject = $false },
        @{ Left = 800; Top = 23; Width = 112; Height = 32; Reject = $false },
        @{ Left = 800; Top = 23; Width = 113; Height = 32; Reject = ($guardIndex -eq 1) },
        @{ Left = 860; Top = 23; Width = 0; Height = 32; Reject = $true },
        @{ Left = 860; Top = 23; Width = 32; Height = 0; Reject = $true },
        @{ Left = 900; Top = 23; Width = 32; Height = 32; Reject = $true },
        @{ Left = 27; Top = 23; Width = 32; Height = 32; Reject = $true },
        @{ Left = 860; Top = 19; Width = 32; Height = 32; Reject = $true },
        @{ Left = 860; Top = 28; Width = 32; Height = 32; Reject = $true }
    )) {
        $scale = 1
        $border = 8
        $bounds = [pscustomobject]@{ Left = 20; Top = 20; Right = 1036; Bottom = 668 }
        $rectangle = [pscustomobject]@{
            Left = $sizingCase.Left; Top = $sizingCase.Top; Width = $sizingCase.Width; Height = $sizingCase.Height
            Right = $sizingCase.Left + $sizingCase.Width; Bottom = $sizingCase.Top + $sizingCase.Height
        }
        $name = 'Synthetic header action'
        $rejected = $false
        try { & $boundsProbe } catch { $rejected = $true }
        Assert-SmokeEqual $rejected $sizingCase.Reject 'Header geometry must reject clipped, empty, oversized Git, or native-reserve-overlapping controls'
        $stockSizingChecks++
    }
}

$keyboardTypeName = 'SmokeKeyboardCheck' + [guid]::NewGuid().ToString('N')
$keyboardType = Add-Type -TypeDefinition ($keyboardSource.Replace('public static class RegressifySmokeText {', "public static class $keyboardTypeName {")) -PassThru |
    Where-Object Name -eq $keyboardTypeName
if ($null -eq $keyboardType.GetMethod('KeySequence')) { throw 'Native shortcuts must provide testable key sequences instead of SendKeys strings.' }
function Test-SmokeKeySequence([ConsoleKey]$Key, [bool]$Control, [bool]$Shift, [string[]]$Expected, [bool]$Alt = $false) {
    $sequence = @($keyboardType::KeySequence($Key, $Control, $Shift, $Alt))
    $actual = @($sequence | ForEach-Object { "$($_.key):$($_.flags)" })
    Assert-SmokeEqual $actual $Expected "Incorrect key events for $Key (Ctrl=$Control, Shift=$Shift, Alt=$Alt)"
    foreach ($inputEvent in $sequence) {
        Assert-SmokeEqual @($inputEvent.scan, $inputEvent.time, $inputEvent.extra.ToUInt64()) @(0, 0, 0) 'Unexpected keyboard packet data'
    }
}
Test-SmokeKeySequence Home $true $false @('17:0', '36:1', '36:3', '17:2')
Test-SmokeKeySequence Home $false $true @('16:0', '36:1', '36:3', '16:2')
Test-SmokeKeySequence DownArrow $false $true @('16:0', '40:1', '40:3', '16:2')
Test-SmokeKeySequence End $true $false @('17:0', '35:1', '35:3', '17:2')
Test-SmokeKeySequence End $false $true @('16:0', '35:1', '35:3', '16:2')
Test-SmokeKeySequence RightArrow $false $false @('39:1', '39:3')
Test-SmokeKeySequence Delete $false $false @('46:1', '46:3')
Test-SmokeKeySequence P $true $true @('17:0', '16:0', '80:0', '80:2', '16:2', '17:2')
Test-SmokeKeySequence A $true $false @('17:0', '65:0', '65:2', '17:2')
Test-SmokeKeySequence Escape $false $false @('27:0', '27:2')
Test-SmokeKeySequence Enter $false $false @('13:0', '13:2')
Test-SmokeKeySequence Backspace $false $false @('8:0', '8:2')
Test-SmokeKeySequence Spacebar $true $false @('17:0', '32:0', '32:2', '17:2')
Test-SmokeKeySequence Tab $false $false @('9:0', '9:2')
Test-SmokeKeySequence Tab $false $true @('16:0', '9:0', '9:2', '16:2')
Test-SmokeKeySequence Tab $false $false @('18:0', '9:0', '9:2', '18:2') -Alt $true
Test-SmokeKeySequence Tab $false $true @('16:0', '18:0', '9:0', '9:2', '18:2', '16:2') -Alt $true
$invalidKeyRejected = $false
try { $keyboardType::KeySequence([ConsoleKey]0, $true, $false, $false) | Out-Null } catch {
    if ($_.Exception.GetBaseException() -isnot [ArgumentOutOfRangeException]) { throw }
    $invalidKeyRejected = $true
}
Assert-SmokeEqual $invalidKeyRejected $true 'An invalid key must fail before constructing input events'

$schemaSelection = @($definitions['Test-SchemaFlows'].FindAll({
    param($node)
    $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Select-SchemaFirstLines'
}, $true))
Assert-SmokeEqual $schemaSelection.Count 1 'Schema line selection helper is missing or ambiguous'
& {
    $events = [System.Collections.Generic.List[string]]::new()
    function Send-FixtureKeys([ConsoleKey]$Key, [switch]$Control, [switch]$Shift) {
        $events.Add("${Key}:$($Control.IsPresent):$($Shift.IsPresent)")
    }
    function Wait-Control([string]$Name) { $events.Add("Wait:$Name") }
    . ([scriptblock]::Create($schemaSelection[0].Extent.Text))
    Select-SchemaFirstLines
    Assert-SmokeEqual @($events) @('Home:True:False', 'DownArrow:False:True', 'DownArrow:False:True', 'Home:False:True', 'Wait:Ln 3, Col 1') 'Schema line selection must end at an explicit line boundary before checking the caret'
}

$pointerTypeName = 'SmokePointerCheck' + [guid]::NewGuid().ToString('N')
$pointerType = Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
public static class $pointerTypeName {
    public static readonly List<string> Events = new List<string>();
    public static object FocusedElement;
    public static bool Compare(object left, object right) { return object.ReferenceEquals(left, right); }
    public static bool FailPressedFrame;
    public static uint ClipboardSequence;
    public static uint GetClipboardSequenceNumber() { return ClipboardSequence; }
    private static bool pressed;
    public static void Reset(bool fail) { Events.Clear(); pressed = false; FailPressedFrame = fail; }
    public static bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int width, int height, uint flags) {
        Events.Add(width > 0 && height > 0 ? "resize:" + width + "x" + height : "activate"); return true;
    }
    public static bool SetPhysicalCursorPos(int x, int y) {
        Events.Add("move:" + x + ":" + y); return true;
    }
    public static void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra) {
        if (flags != 2 && flags != 4 && flags != 8 && flags != 16) throw new InvalidOperationException("Unexpected pointer event");
        pressed = flags == 2 || flags == 8;
        Events.Add((flags == 8 || flags == 16 ? "right-" : "") + (pressed ? "down" : "up"));
    }
    public static int DwmFlush() {
        Events.Add("frame");
        if (pressed && FailPressedFrame) throw new InvalidOperationException("Synthetic compositor failure");
        return 0;
    }
}
"@ -PassThru
$pointerProbe = $definitions['Invoke-Control'].Extent.Text.
    Replace('function Invoke-Control(', 'function Invoke-SmokePointerProbe(').
    Replace('[RegressifySmokeNative]', "[$pointerTypeName]").
    Replace('[RegressifySmokePointer]', "[$pointerTypeName]")
$clickProbe = $definitions['Send-FixturePointerClick'].Extent.Text.Replace('[RegressifySmokeNative]', "[$pointerTypeName]")

function Test-SmokePointerClick([bool]$FailPressedFrame) {
    . ([scriptblock]::Create($pointerProbe))
    . ([scriptblock]::Create($clickProbe))
    function Wait-Control {
        [pscustomobject]@{ Current = [pscustomobject]@{
            BoundingRectangle = [pscustomobject]@{ Left = 10; Top = 20; Width = 100; Height = 40 }
        } }
    }
    function Focus-Fixture { $pointerType::Events.Add('focus') }
    $originalWindow = $script:window
    $script:window = [IntPtr]::Zero
    $pointerType::Reset($FailPressedFrame)
    $failure = $null
    try { Invoke-SmokePointerProbe 'Synthetic row' -Pointer } catch {
        $failure = $_.Exception.GetBaseException().Message
    } finally { $script:window = $originalWindow }
    $expected = @('activate', 'focus', 'move:60:40', 'frame', 'down', 'frame', 'up')
    if (-not $FailPressedFrame) { $expected += 'frame' }
    Assert-SmokeEqual @($pointerType::Events) $expected 'Native pointer input must settle between events and always release'
    Assert-SmokeEqual $failure $(if ($FailPressedFrame) { 'Synthetic compositor failure' } else { $null }) 'Pointer failures must propagate after release'
}
Test-SmokePointerClick $false
Test-SmokePointerClick $true

foreach ($failPressedFrame in @($false, $true)) {
    & {
        . ([scriptblock]::Create($clickProbe))
        $pointerType::Reset($failPressedFrame)
        $failure = $null
        try { Send-FixturePointerClick -Right } catch { $failure = $_.Exception.GetBaseException().Message }
        $expected = @('frame', 'right-down', 'frame', 'right-up')
        if (-not $failPressedFrame) { $expected += 'frame' }
        Assert-SmokeEqual @($pointerType::Events) $expected 'Right clicks must settle and always release'
        Assert-SmokeEqual $failure $(if ($failPressedFrame) { 'Synthetic compositor failure' } else { $null }) 'Right-click frame failures must propagate'
    }
}

$titlebarMatrix = @($definitions['Test-TitlebarFlows'].Body.EndBlock.Statements | Where-Object {
    $_ -is [System.Management.Automation.Language.ForEachStatementAst]
})[0]
$titlebarSizingProbe = $titlebarMatrix.Extent.Text.
    Replace('[RegressifySmokeNative]', "[$pointerTypeName]").
    Replace('[System.Windows.Automation.AutomationElement]', "[$pointerTypeName]").
    Replace('[System.Windows.Automation.Automation]', "[$pointerTypeName]")
foreach ($wideOnlyWorkaround in @($false, $true)) {
    & {
        $originalWindow = $script:window
        $script:window = [IntPtr]::Zero
        $screen = [pscustomobject]@{ Left = 0; Top = 0 }
        $pointerType::Reset($false)
        function Select-Theme([string]$Theme) { $pointerType::Events.Add("theme:$Theme") }
        function Test-TitlebarControlBounds([string]$Theme, [int[]]$Dimensions) { $pointerType::Events.Add("bounds:${Theme}:$($Dimensions -join 'x')") }
        function Capture-Window([string]$Name) { $pointerType::Events.Add("capture:$Name") }
        function Invoke-Control([string]$Name, [switch]$Pointer) {
            if ($Name -in @('Commands', 'Deploy')) {
                Assert-SmokeEqual $Pointer.IsPresent $true 'Header actions must be checked through native pointer input'
                $pointerType::Events.Add("action:$Name")
            }
        }
        function Wait-Control([string]$Name) {
            if ($Name.StartsWith('Appearance: ')) {
                return [pscustomobject]@{ Name = $Name }
            }
            if ($Name -in @('System', 'Light', 'Dark')) { $pointerType::Events.Add("menu:$Name") }
        }
        function Find-Control { }
        function Send-FixtureKeys([ConsoleKey]$Key) {
            $pointerType::Events.Add("key:$Key")
            if ($Key -eq [ConsoleKey]::Tab) { $pointerType::FocusedElement = $appearance }
        }
        function Wait-WindowState([scriptblock]$Match, [string]$Failure) { if (-not (& $Match)) { throw $Failure } }
        try {
            $probe = $titlebarSizingProbe
            if ($wideOnlyWorkaround) {
                $probe = $probe.Replace('Select-Theme $theme', "[$pointerTypeName]::SetWindowPos([IntPtr]::Zero, [IntPtr]::Zero, 0, 0, 1600, 1000, 0x0040) | Out-Null`n            Select-Theme `$theme")
            }
            & ([scriptblock]::Create($probe))
            $expected = foreach ($size in @(@(1000, 640), @(1600, 1000))) {
                'resize:' + ($size -join 'x')
                'theme:Dark'
                foreach ($theme in @('Light', 'Dark')) {
                    "theme:$theme"
                    "bounds:${theme}:$($size -join 'x')"
                    'key:Tab'
                    'key:Enter'
                    'menu:System'
                    'menu:Light'
                    'menu:Dark'
                    'key:Escape'
                    "capture:stock-sizing-final-$($theme.ToLowerInvariant())-$($size[0])"
                    'action:Commands'
                    "capture:stock-sizing-commands-$($theme.ToLowerInvariant())-$($size[0])"
                    'key:Escape'
                    'action:Deploy'
                    "capture:stock-sizing-deploy-$($theme.ToLowerInvariant())-$($size[0])"
                }
            }
            $rejected = $false
            try { Assert-SmokeEqual @($pointerType::Events) @($expected) 'Titlebar sizing order changed' } catch { $rejected = $true }
            Assert-SmokeEqual $rejected $wideOnlyWorkaround 'Titlebar sizing regression must reject a wide-only theme workaround'
        } finally { $script:window = $originalWindow }
    }
    $stockSizingChecks++
}

$clipboardProbe = $definitions['Invoke-FixtureClipboard'].Extent.Text.Replace('[RegressifySmokeNative]', "[$pointerTypeName]")
& {
    . ([scriptblock]::Create($clipboardProbe))
    $pointerType::Reset($false)
    $attempts = [System.Collections.Generic.List[int]]::new()
    $snapshot = [pscustomobject]@{ Fixture = 'opaque clipboard snapshot' }
    $actual = Invoke-FixtureClipboard {
        $attempts.Add(1)
        if ($attempts.Count -lt 3) { throw [System.Runtime.InteropServices.ExternalException]::new('Synthetic clipboard busy', -2147221040) }
        $snapshot
    }
    Assert-SmokeEqual $attempts.Count 3 'Busy clipboard access must retry before success'
    if (-not [object]::ReferenceEquals($actual, $snapshot)) { throw 'Clipboard retries must preserve the original data object.' }
    foreach ($busy in @($false, $true)) {
        $attempts.Clear()
        $failure = $null
        $timeout = if ($busy) { 0 } else { 2000 }
        try {
            Invoke-FixtureClipboard -TimeoutMilliseconds $timeout -Operation {
                $attempts.Add(1)
                if ($busy) { throw [System.Runtime.InteropServices.ExternalException]::new('Synthetic clipboard busy', -2147221040) }
                throw [System.InvalidOperationException]::new('Synthetic clipboard permanent failure')
            }
        } catch { $failure = $_.Exception.GetBaseException().Message }
        Assert-SmokeEqual $attempts.Count 1 'Expired or non-busy clipboard failures must not retry'
        Assert-SmokeEqual $failure $(if ($busy) { 'Synthetic clipboard busy' } else { 'Synthetic clipboard permanent failure' }) 'Clipboard failure details must propagate'
    }
}

$sequenceProbe = $definitions['Wait-FixtureClipboardText'].Extent.Text.Replace('[RegressifySmokeClipboard]', "[$pointerTypeName]")
$overviewClipboard = @($definitions['Test-DefaultFlows'].Body.EndBlock.Statements | Where-Object {
    $_ -is [System.Management.Automation.Language.TryStatementAst]
})[0]
$overviewCopyWaits = @($overviewClipboard.Body.FindAll({
    param($node)
    $node -is [System.Management.Automation.Language.CommandAst] -and
        $node.GetCommandName() -eq 'Wait-FixtureClipboardText'
}, $true))
Assert-SmokeEqual $overviewCopyWaits.Count 2 'Both Overview Details copy checks must wait for the clipboard sequence before reading exact text'
foreach ($mode in @('success', 'wrong-text', 'no-write')) {
    & {
        . ([scriptblock]::Create($sequenceProbe))
        $events = [System.Collections.Generic.List[string]]::new()
        $pointerType::ClipboardSequence = 10
        function Wait-WindowState([scriptblock]$Match, [string]$Failure) {
            Assert-SmokeEqual (& $Match) $false 'Clipboard read must wait while the sequence is unchanged'
            Assert-SmokeEqual $events.Count 0 'Waiting must not open the clipboard'
            if ($mode -eq 'no-write') { throw $Failure }
            $pointerType::ClipboardSequence = 11
            Assert-SmokeEqual (& $Match) $true 'A completed write must advance the clipboard sequence'
            $events.Add('changed')
        }
        function Invoke-FixtureClipboard([scriptblock]$Operation) {
            $events.Add('read')
            if ($mode -eq 'wrong-text') { return 'different synthetic text' }
            'exact synthetic text'
        }
        $failure = $null
        try { Wait-FixtureClipboardText 10 'exact synthetic text' 'Synthetic exact-copy failure' } catch {
            $failure = $_.Exception.GetBaseException().Message
        }
        Assert-SmokeEqual @($events) $(if ($mode -eq 'no-write') { @() } else { @('changed', 'read') }) 'Clipboard reads must follow completed writes only'
        Assert-SmokeEqual $failure $(if ($mode -eq 'success') { $null } else { 'Synthetic exact-copy failure' }) 'Wrong or missing clipboard writes must fail'
    }
}

$lifecycle = @($entry.EndBlock.Statements | Where-Object { $_ -is [System.Management.Automation.Language.TryStatementAst] })
Assert-SmokeEqual $lifecycle.Count 1 'The entry point must own the run/cleanup boundary'
$runnerTokens = $null
$runnerErrors = $null
$runner = [System.Management.Automation.Language.Parser]::ParseInput(
    $lifecycle[0].Extent.Text, $entry.Extent.File, [ref]$runnerTokens, [ref]$runnerErrors).GetScriptBlock()
$flows = [ordered]@{
    IconFlows = @('Test-WindowIcons', 'Test-BrandingIcons')
    ErrorImageFlows = @('Test-ErrorImageFlows')
    SchemaFlows = @('Test-SchemaFlows')
    GitFlows = @('Test-GitFlows')
    ComparisonFlows = @('Test-ComparisonFlows')
    TitlebarFlows = @('Test-TitlebarFlows')
    DeploymentFlows = @('Test-DeploymentFlows')
    PublishingFlows = @('Test-PublishingFlows')
    UpdateFlows = @('Test-UpdateFlows')
    InstanceFlows = @('Test-InstanceFlows')
    ArtifactFlows = @('Test-ArtifactFlows')
    ScenarioFlows = @('Test-ScenarioFlows')
}
$mockCommands = @('Write-JsonFixture', 'Start-Fixture', 'Test-ApplicationMenus', 'Test-DefaultFlows') + @($flows.Values | ForEach-Object { $_ })
foreach ($name in $mockCommands) {
    if (-not $definitions.ContainsKey($name)) { throw "Dispatch helper is missing: $name" }
}

function Test-SmokeDispatch([hashtable]$Flags, [string[]]$ExpectedFlow, [string]$FailurePoint = '', [switch]$SettingsAbsent) {
    foreach ($parameter in $parameters) { Set-Variable -Name $parameter -Value ([bool]$Flags[$parameter]) }
    $events = [System.Collections.Generic.List[string]]::new()
    $workspace = Join-Path ([System.IO.Path]::GetTempPath()) ('regressify-smoke-check-' + [guid]::NewGuid().ToString('N'))
    $settingsPath = Join-Path $workspace 'settings.json'
    $settingsBackup = Join-Path $workspace 'original-settings.json'
    $settingsExisted = -not $SettingsAbsent
    $script:process = $null
    $script:fixtureProcesses = [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
    $script:publishingBackups = @()
    $script:publishingCredentials = @()
    $script:publishingLandingIds = @()
    $script:cleanupSnapshot = $null
    foreach ($name in $mockCommands) {
        Set-Item "Function:$name" ([scriptblock]::Create("`$events.Add('$name'); if (`$FailurePoint -eq '$name') { throw 'Synthetic failure: $name' }"))
    }
    function Wait-Control { }
    function Copy-Item { $events.Add('Restore settings') }
    function Remove-Item { $events.Add('Remove settings') }
    function Get-FileHash { [pscustomobject]@{ Hash = 'unchanged' } }
    function Test-Path([string]$LiteralPath) { $SettingsAbsent -and $LiteralPath -eq $settingsPath }

    $failure = $null
    try { & $runner | Out-Null } catch { $failure = $_.Exception.Message }
    $expectedFailure = if ($FailurePoint) { "Synthetic failure: $FailurePoint" } else { $null }
    Assert-SmokeEqual $failure $expectedFailure 'Flow errors must propagate after cleanup'
    $expected = @('Write-JsonFixture', 'Start-Fixture')
    if ($ExpectedFlow[0] -notin @('Test-WindowIcons', 'Test-ErrorImageFlows')) { $expected += 'Test-ApplicationMenus' }
    $expected += $ExpectedFlow
    if ($FailurePoint) {
        $failureIndex = [array]::IndexOf($expected, $FailurePoint)
        if ($failureIndex -lt 0) { throw "Failure point is not in the expected flow: $FailurePoint" }
        $expected = $expected[0..$failureIndex]
    }
    $expected += $(if ($SettingsAbsent) { 'Remove settings' } else { 'Restore settings' })
    Assert-SmokeEqual @($events) $expected 'Incorrect flow order or missing cleanup'
    $script:dispatchChecks++
}

$script:dispatchChecks = 0
Test-SmokeDispatch @{} @('Test-DefaultFlows')
Test-SmokeDispatch @{ Capture = $true } @('Test-DefaultFlows')
Test-SmokeDispatch @{ SkipBuild = $true } @('Test-DefaultFlows')
Test-SmokeDispatch @{ KeepOpen = $true } @('Test-DefaultFlows')
Test-SmokeDispatch @{} @('Test-DefaultFlows') -SettingsAbsent
foreach ($failure in @('Write-JsonFixture', 'Start-Fixture', 'Test-ApplicationMenus', 'Test-DefaultFlows')) {
    Test-SmokeDispatch @{} @('Test-DefaultFlows') -FailurePoint $failure
}
foreach ($flow in $flows.GetEnumerator()) {
    $flags = @{ $flow.Key = $true }
    Test-SmokeDispatch $flags $flow.Value
    foreach ($failure in $flow.Value) { Test-SmokeDispatch $flags $flow.Value -FailurePoint $failure }
}
$flowNames = @($flows.Keys)
for ($index = 0; $index -lt $flowNames.Count - 1; $index++) {
    Test-SmokeDispatch @{ $flowNames[$index] = $true; $flowNames[$index + 1] = $true } $flows[$flowNames[$index]]
}

$draftTail = [scriptblock]::Create($definitions['Test-DefaultFlows'].Body.EndBlock.Statements[-1].Extent.Text)
function Test-SmokeDraftScope([bool]$KeepOpen) {
    $workspace = Join-Path ([System.IO.Path]::GetTempPath()) 'regressify-smoke-draft-check'
    $script:cleanupPath = $null
    $script:cleanupSnapshot = $null
    function Get-FileHash { [pscustomobject]@{ Hash = 'original-draft-hash' } }
    function Invoke-Control { }
    function Send-FixtureKeys { }
    function Send-FixtureText { }
    function Wait-SourceText { }
    & $draftTail
    $expectedPath = if ($KeepOpen) { $null } else { Join-Path $workspace 'visual_tests/smoke.tests.json' }
    $expectedHash = if ($KeepOpen) { $null } else { 'original-draft-hash' }
    Assert-SmokeEqual $script:cleanupPath $expectedPath 'Default flow lost the script-scoped cleanup path'
    Assert-SmokeEqual $script:cleanupSnapshot $expectedHash 'Default flow lost the script-scoped draft hash'
}
Test-SmokeDraftScope $false
Test-SmokeDraftScope $true

Write-Output "Smoke harness checks passed: $($loaded.Count) modules, $($definitions.Count) helpers, $dispatchChecks dispatch/cleanup cases, $stockSizingChecks stock-sizing cases, native keyboard/pointer events, and default-flow draft scope. No UI, app builds, or user settings were touched."