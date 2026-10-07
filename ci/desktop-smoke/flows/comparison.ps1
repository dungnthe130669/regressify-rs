function Test-ComparisonFlows {
    if (-not ('RegressifySmokeZoomKeys' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RegressifySmokeZoomKeys {
    [DllImport("user32.dll")] public static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
}
'@
    }
    function Write-ComparisonImage([string]$Path, [int]$Width, [int]$Height) {
        $bitmap = [System.Drawing.Bitmap]::new($Width, $Height)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            for ($row = 0; $row * 80 -lt $Height; $row++) {
                for ($column = 0; $column * 80 -lt $Width; $column++) {
                    $color = [System.Drawing.Color]::FromArgb(35 + ($column * 37) % 180, 35 + ($row * 43) % 180, 40 + (($row + $column) * 59) % 150)
                    $brush = [System.Drawing.SolidBrush]::new($color)
                    try { $graphics.FillRectangle($brush, $column * 80, $row * 80, 80, 80) } finally { $brush.Dispose() }
                }
            }
            $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
    }
    function Get-ComparisonBounds([string]$Pane) {
        $preview = (Wait-Control "$Pane image preview").Current.BoundingRectangle
        $canvas = (Wait-Control "$Pane image canvas").Current.BoundingRectangle
        return [System.Drawing.RectangleF]::Intersect(
            [System.Drawing.RectangleF]::new($preview.Left, $preview.Top, $preview.Width, $preview.Height),
            [System.Drawing.RectangleF]::new($canvas.Left, $canvas.Top, $canvas.Width, $canvas.Height))
    }
    function Move-ComparisonPointer([string]$Pane, [double]$HorizontalFraction = 0.43, [double]$VerticalFraction = 0.41) {
        $bounds = Get-ComparisonBounds $Pane
        Focus-Fixture
        $horizontal = [int]($bounds.Left + $bounds.Width * $HorizontalFraction)
        $vertical = [int]($bounds.Top + $bounds.Height * $VerticalFraction)
        if (-not [RegressifySmokePointer]::SetPhysicalCursorPos($horizontal, $vertical)) { throw 'Cannot position the comparison pointer.' }
        Wait-Control "$Pane image preview" | Out-Null
        return @(($horizontal - $bounds.Left), ($vertical - $bounds.Top))
    }
    function Test-GuideLine($Bitmap, $Bounds, [double]$Horizontal, [double]$Vertical, [bool]$Across) {
        for ($shift = -3; $shift -le 3; $shift++) {
            $white = 0
            $samples = 0
            $length = if ($Across) { $Bounds.Width } else { $Bounds.Height }
            for ($distance = 24; $distance -lt $length - 24; $distance += 23) {
                $column = [int]($Bounds.Left + $(if ($Across) { $distance } else { $Horizontal + $shift }))
                $row = [int]($Bounds.Top + $(if ($Across) { $Vertical + $shift } else { $distance }))
                $color = $Bitmap.GetPixel($column, $row)
                if ($color.R -ge 235 -and $color.G -ge 235 -and $color.B -ge 235) { $white++ }
                $samples++
            }
            if ($samples -gt 0 -and $white / $samples -ge 0.8) { return $true }
        }
        return $false
    }
    function Read-StableComparisonSample($Bitmap, $Bounds, [int]$Horizontal, [int]$Vertical) {
        for ($verticalShift = 0; $verticalShift -le 8; $verticalShift++) {
            for ($horizontalShift = 0; $horizontalShift -le 8; $horizontalShift++) {
                $colors = @()
                $stable = $true
                foreach ($preview in $Bounds) {
                    $column = [int]($preview.Left + $Horizontal + $horizontalShift)
                    $row = [int]($preview.Top + $Vertical + $verticalShift)
                    $color = $Bitmap.GetPixel($column, $row)
                    foreach ($neighbor in @(@(-2, 0), @(2, 0), @(0, -2), @(0, 2))) {
                        if ($Bitmap.GetPixel($column + $neighbor[0], $row + $neighbor[1]).ToArgb() -ne $color.ToArgb()) { $stable = $false }
                    }
                    $colors += $color
                }
                if ($stable) { return $colors }
            }
        }
        throw 'Comparison pixel probe did not find a stable tile interior.'
    }
    function Assert-ComparisonPixels([string]$Name, [string]$Pane = 'Reference', [switch]$HideGuides, [double]$HorizontalFraction = 0.43, [double]$VerticalFraction = 0.41) {
        $pointer = Move-ComparisonPointer $Pane $HorizontalFraction $VerticalFraction
        $native = [RegressifySmokeNative+Rect]::new()
        if (-not [RegressifySmokeNative]::GetWindowRect($script:window, [ref]$native)) { throw 'Cannot read comparison window bounds.' }
        $bounds = @('Reference', 'Test', 'Diff') | ForEach-Object {
            $preview = Get-ComparisonBounds $_
            [System.Drawing.RectangleF]::new($preview.Left - $native.Left, $preview.Top - $native.Top, $preview.Width, $preview.Height)
        }
        if ($HideGuides) {
            $header = (Wait-Control 'Results').Current.BoundingRectangle
            [RegressifySmokePointer]::SetPhysicalCursorPos([int]($header.Left + $header.Width / 2), [int]($header.Top + $header.Height / 2)) | Out-Null
            Wait-Control 'Results' | Out-Null
        }
        Capture-Window $Name
        $bitmap = [System.Drawing.Bitmap]::FromFile((Join-Path $artifacts "$Name.png"))
        try {
            foreach ($preview in $bounds) {
                foreach ($across in @($false, $true)) {
                    $visible = Test-GuideLine $bitmap $preview $pointer[0] $pointer[1] $across
                    if ($visible -eq $HideGuides.IsPresent) { throw "Pointer guide visibility is incorrect in '$Name' (horizontal: $across)." }
                }
            }
            $matches = 0
            $signature = @()
            foreach ($verticalFraction in @(0.15, 0.35, 0.55)) {
                foreach ($horizontalFraction in @(0.15, 0.35, 0.55)) {
                    $vertical = [int]($bounds[0].Height * $verticalFraction)
                    $horizontal = [int]($bounds[0].Width * $horizontalFraction)
                    $sample = Read-StableComparisonSample $bitmap $bounds $horizontal $vertical
                    $reference = $sample[0]
                    foreach ($compared in $sample[1..($sample.Count - 1)]) {
                        if ([Math]::Abs([int]$reference.R - $compared.R) -le 8 -and [Math]::Abs([int]$reference.G - $compared.G) -le 8 -and [Math]::Abs([int]$reference.B - $compared.B) -le 8) { $matches++ }
                    }
                    $signature += $reference.ToArgb()
                }
            }
            $expected = 9 * ($bounds.Count - 1)
            if ($matches -ne $expected) { throw "Comparison offsets differ in '$Name' ($matches/$expected matching pixel samples)." }
            $script:comparisonSignature = $signature -join ','
        } finally { $bitmap.Dispose() }
    }
    function Send-ComparisonWheel([string]$Pane, [bool]$Horizontal) {
        Move-ComparisonPointer $Pane | Out-Null
        $flags = if ($Horizontal) { 0x1000 } else { 0x0800 }
        $amount = if ($Horizontal) { 120 } else { -120 }
        $data = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$amount), 0)
        [RegressifySmokeNative]::mouse_event($flags, 0, 0, $data, [UIntPtr]::Zero)
        Wait-Control "$Pane image preview" | Out-Null
    }
    function Test-ComparisonGestures([string]$Name, [string]$Pane) {
        foreach ($amount in @(120, -120)) {
            $before = (Wait-Control "$Pane image canvas").Current.BoundingRectangle
            $visible = Get-ComparisonBounds $Pane
            $pointer = Move-ComparisonPointer $Pane
            $horizontal = $visible.Left + $pointer[0]
            $vertical = $visible.Top + $pointer[1]
            $data = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$amount), 0)
            [RegressifySmokeZoomKeys]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero)
            try {
                [RegressifySmokeNative]::mouse_event(0x0800, 0, 0, $data, [UIntPtr]::Zero)
                $after = (Wait-Control "$Pane image canvas").Current.BoundingRectangle
            } finally { [RegressifySmokeZoomKeys]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero) }
            if (($amount -gt 0 -and $after.Width -le $before.Width) -or ($amount -lt 0 -and $after.Width -ge $before.Width)) {
                throw "Ctrl+wheel did not zoom $Pane in '$Name'."
            }
            $anchoredX = $after.Left + ($horizontal - $before.Left) * ($after.Width / $before.Width)
            $anchoredY = $after.Top + ($vertical - $before.Top) * ($after.Height / $before.Height)
            if ([Math]::Abs($anchoredX - $horizontal) -gt 2 -or [Math]::Abs($anchoredY - $vertical) -gt 2) {
                throw "Ctrl+wheel moved the pointer's image coordinate in '$Name'."
            }
        }
        $before = (Wait-Control "$Pane image canvas").Current.BoundingRectangle
        $visible = Get-ComparisonBounds $Pane
        $pointer = Move-ComparisonPointer $Pane
        [RegressifySmokeNative]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
        try {
            [RegressifySmokePointer]::SetPhysicalCursorPos([int]($visible.Left + $pointer[0] - 42), [int]($visible.Top + $pointer[1] - 54)) | Out-Null
            $after = (Wait-Control "$Pane image canvas").Current.BoundingRectangle
            if ([Math]::Abs($after.Left - $before.Left + 42) -gt 2 -or [Math]::Abs($after.Top - $before.Top + 54) -gt 2) {
                throw "Left-drag did not pan the image in '$Name'."
            }
        } finally { [RegressifySmokeNative]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero) }
        Assert-ComparisonPixels "$Name-zoom-pan" $Pane
    }
    Write-ComparisonImage (Join-Path $output 'bitmaps_reference/checkout.png') 1600 2200
    Write-ComparisonImage (Join-Path $output 'bitmaps_test/smoke-run/checkout.png') 1280 1800
    Write-ComparisonImage (Join-Path $output 'bitmaps_test/smoke-run/checkout_diff.png') 1600 2400
    $reportPath = Join-Path $output 'bitmaps_test/smoke-run/report.json'
    $report = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
    $report.tests[0].pair.diff.dimensionDifference.width = -320
    $report.tests[0].pair.diff.dimensionDifference.height = -400
    Write-JsonFixture $reportPath $report
    Invoke-Control 'smoke.tests.json'
    Invoke-Control 'Results'
    Wait-Control 'Test 1280 x 1800' | Out-Null
    Invoke-Control 'Side by side'
    Wait-Control 'Reference 1600 x 2200' | Out-Null
    Wait-Control 'Diff 1600 x 2400' | Out-Null
    foreach ($theme in @('Light', 'Dark')) {
        Select-Theme $theme
        foreach ($dimensions in @(@(1600, 1000), @(1080, 720))) {
            [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, $dimensions[0], $dimensions[1], 0x0040) | Out-Null
            Invoke-Control 'Error Account desktop'
            Wait-Control 'Missing selector #account' | Out-Null
            Test-CaptureInformation ('information-same-url-' + $theme.ToLowerInvariant() + '-' + $dimensions[0]) -SameUrl
            Invoke-Control 'Mismatch Checkout'
            Wait-Control 'Reference 1600 x 2200' | Out-Null
            Wait-Control 'Test 1280 x 1800' | Out-Null
            Test-CaptureInformation ('information-distinct-urls-' + $theme.ToLowerInvariant() + '-' + $dimensions[0]) -DiffX -320 -DiffY -400
            Test-ImageContextMenus
            foreach ($removed in @('Next capture', 'Previous capture', 'Next captures', 'Previous captures')) {
                if ($null -ne (Find-Control $removed)) { throw "Removed pagination control '$removed' is still present." }
            }
            $tab = (Wait-Control 'Side by side').Current.BoundingRectangle
            $fit = (Wait-Control 'Fit area').Current.BoundingRectangle
            if ($fit.Left -lt $tab.Right -or [Math]::Abs(($fit.Top + $fit.Height / 2) - ($tab.Top + $tab.Height / 2)) -gt 3) {
                throw 'Image controls are not aligned to the right of the tabs.'
            }
            Invoke-Control 'Actual size (100%)'
            $actual = (Wait-Control 'Reference image canvas').Current.BoundingRectangle
            $name = 'comparison-' + $theme.ToLowerInvariant() + '-' + $dimensions[0]
            Assert-ComparisonPixels "$name-initial"
            Assert-ComparisonPixels "$name-moved-guides" -HorizontalFraction 0.61 -VerticalFraction 0.54
            foreach ($pane in @('Reference', 'Test', 'Diff')) {
                foreach ($horizontal in @($false, $true)) {
                    $before = $script:comparisonSignature
                    Send-ComparisonWheel $pane $horizontal
                    Assert-ComparisonPixels ("$name-" + $pane.ToLowerInvariant() + $(if ($horizontal) { '-horizontal' } else { '-vertical' })) $pane
                    if ($before -eq $script:comparisonSignature) { throw "The $pane wheel did not move comparison pixels (horizontal: $horizontal)." }
                }
            }
            foreach ($pane in @('Reference', 'Test', 'Diff')) {
                Test-ComparisonGestures ("$name-" + $pane.ToLowerInvariant()) $pane
            }
            Invoke-Control 'Fit area'
            foreach ($pane in @('Reference', 'Test', 'Diff')) {
                $preview = (Wait-Control "$pane image preview").Current.BoundingRectangle
                $canvas = (Wait-Control "$pane image canvas").Current.BoundingRectangle
                $scale = [Math]::Min(1.0, [Math]::Min($preview.Width / $actual.Width, $preview.Height / $actual.Height))
                if ($canvas.Left -lt $preview.Left - 1 -or $canvas.Top -lt $preview.Top - 1 -or $canvas.Right -gt $preview.Right + 1 -or $canvas.Bottom -gt $preview.Bottom + 1) {
                    throw "Fit area clipped the $pane image in '$name'."
                }
                if ([Math]::Abs($canvas.Width - $actual.Width * $scale) -gt 2 -or [Math]::Abs($canvas.Height - $actual.Height * $scale) -gt 2) {
                    throw "Fit area used an incorrect scale for $pane in '$name': canvas=$canvas, actual=$actual, viewport=$preview, scale=$scale."
                }
                if ([Math]::Abs(($canvas.Left + $canvas.Width / 2) - ($preview.Left + $preview.Width / 2)) -gt 1) {
                    throw "Fit area did not center the $pane image in '$name'."
                }
            }
            Assert-ComparisonPixels "$name-fit" 'Test'
            Assert-ComparisonPixels "$name-no-guides" -HideGuides
            Invoke-Control 'Fit width'
            foreach ($pane in @('Reference', 'Test', 'Diff')) {
                $preview = (Wait-Control "$pane image preview").Current.BoundingRectangle
                $canvas = (Wait-Control "$pane image canvas").Current.BoundingRectangle
                $scale = [Math]::Min(1.0, $preview.Width / $actual.Width)
                if ([Math]::Abs($canvas.Width - $actual.Width * $scale) -gt 2 -or [Math]::Abs($canvas.Height - $actual.Height * $scale) -gt 2) {
                    throw "Fit width used an incorrect scale for $pane in '$name'."
                }
            }
            Assert-ComparisonPixels "$name-fit-width" 'Diff'
            Invoke-Control 'Fit area'
            foreach ($pane in @('Reference', 'Test', 'Diff')) {
                Move-ComparisonPointer $pane | Out-Null
                Send-FixturePointerClick
                Wait-Control "$pane image preview" | Out-Null
                foreach ($other in @('Reference', 'Test', 'Diff') | Where-Object { $_ -ne $pane }) {
                    if ($null -ne (Find-Control "$other image preview")) { throw "Clicking $pane did not open its tab." }
                }
                Invoke-Control 'Fit width'
                Capture-Window ("$name-" + $pane.ToLowerInvariant() + '-tab')
                Invoke-Control 'Side by side'
                Wait-Control 'Diff 1600 x 2400' | Out-Null
            }
        }
    }
    Invoke-Control 'account.tests.json'
    Wait-Control 'No captures for this suite' | Out-Null
    if ($null -ne (Find-Control 'Test image preview') -or $null -ne (Find-Control 'Mismatch Checkout')) { throw 'An empty suite displayed another suite''s captures.' }
    Capture-Window 'comparison-empty-suite'
    Invoke-Control 'smoke.tests.json'
    Wait-Control 'Test 1280 x 1800' | Out-Null
    Test-ReportApproval
    Test-ErrorImageFlows
    Write-Output 'Three-pane Results, fit modes, image-tab navigation, pointer-centered zoom, drag panning, linked scrolling, and crosshairs passed native checks in light/dark desktop and compact windows.'
}
