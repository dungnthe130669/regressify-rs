function Test-ErrorImageFlows {
    $process.CloseMainWindow() | Out-Null
    if (-not $process.WaitForExit(10000)) { throw 'Comparison fixture did not close before the failed-reference check.' }
    $failedOutput = Join-Path $workspace '.regressify/account'
    $failedRun = Join-Path $failedOutput 'bitmaps_test/failed-reference'
    $failedReference = Join-Path $failedOutput 'bitmaps_reference/failed.png'
    [System.IO.Directory]::CreateDirectory((Join-Path $failedOutput 'html_report')) | Out-Null
    Write-JsonFixture (Join-Path $failedRun 'report.json') @{
        id = 'account'
        tests = @(@{
            status = 'fail'
            pair = @{
                label = 'Failed reference'
                reference = '../bitmaps_reference/failed.png'
                test = '../bitmaps_test/failed-reference/failed.png'
                diffImage = '../bitmaps_test/failed-reference/failed_diff.png'
                engineErrorMsg = 'Capture failed: missing selector #pay'
            }
        })
    }
    $errorImage = Join-Path $repository 'src/test/data/error-icon.png'
    $legacyImage = Join-Path $failedRun 'failed.png'
    Write-TestImage $legacyImage ([System.Drawing.Color]::Red)
    Write-JsonFixture "$legacyImage.error.json" @{ error = $true; message = 'Capture failed: missing selector #pay' }
    $originalHash = (Get-FileHash -LiteralPath $legacyImage).Hash
    Copy-Item -LiteralPath $errorImage -Destination (Join-Path $failedRun 'failed_diff.png')
    $image = [System.Drawing.Bitmap]::FromFile($errorImage)
    try { $dimensions = "$($image.Width) x $($image.Height)" } finally { $image.Dispose() }
    Start-Fixture -Name 'failed-reference'
    Invoke-Control 'account.tests.json'
    Invoke-Control 'Results'
    foreach ($theme in @('Light', 'Dark')) {
        Select-Theme $theme
        foreach ($windowSize in @(@(1600, 1000), @(1080, 720))) {
            [RegressifySmokeNative]::SetWindowPos($script:window, [IntPtr]::Zero, $screen.Left + 20, $screen.Top + 20, $windowSize[0], $windowSize[1], 0x0040) | Out-Null
            Invoke-Control 'Side by side'
            Wait-Control "Reference $dimensions" | Out-Null
            Wait-Control "Test $dimensions" | Out-Null
            Wait-Control "Diff $dimensions" | Out-Null
            Wait-Control 'Capture failed: missing selector #pay' | Out-Null
            $name = 'comparison-failed-reference-' + $theme.ToLowerInvariant() + '-' + $windowSize[0]
            Capture-Window "$name-side-by-side"
            $native = [RegressifySmokeNative+Rect]::new()
            if (-not [RegressifySmokeNative]::GetWindowRect($script:window, [ref]$native)) { throw 'Cannot locate error-image window.' }
            $bounds = @('Reference', 'Test', 'Diff') | ForEach-Object { (Wait-Control "$_ image canvas").Current.BoundingRectangle }
            $bitmap = [System.Drawing.Bitmap]::FromFile((Join-Path $artifacts "$name-side-by-side.png"))
            try {
                $redSamples = 0
                foreach ($sample in @(@(0.5, 0.13), @(0.5, 0.28), @(0.5, 0.44), @(0.5, 0.58), @(0.5, 0.90), @(0.1, 0.15))) {
                    $colors = foreach ($pane in $bounds) {
                        $bitmap.GetPixel([int]($pane.Left - $native.Left + $pane.Width * $sample[0]), [int]($pane.Top - $native.Top + $pane.Height * $sample[1]))
                    }
                    if ($colors[0].R -gt 160 -and $colors[0].G -lt 80 -and $colors[0].B -lt 80) { $redSamples++ }
                    foreach ($color in $colors[1..2]) {
                        if ([Math]::Abs([int]$color.R - $colors[0].R) -gt 8 -or [Math]::Abs([int]$color.G - $colors[0].G) -gt 8 -or [Math]::Abs([int]$color.B - $colors[0].B) -gt 8) {
                            throw 'Reference/Test error artwork differs from Diff.'
                        }
                    }
                }
                if ($redSamples -eq 0) { throw 'Rounded error artwork did not render.' }
            } finally { $bitmap.Dispose() }
            Invoke-Control 'Reference'
            Wait-Control "Reference $dimensions" | Out-Null
            Capture-Window "$name-reference"
            Invoke-Control 'Test'
            Wait-Control "Test $dimensions" | Out-Null
            Capture-Window "$name-test"
            if (Test-Path -LiteralPath $failedReference) { throw 'Previewing a failed reference created a baseline file.' }
            if ((Get-FileHash -LiteralPath $legacyImage).Hash -ne $originalHash) { throw 'Previewing a failed capture rewrote its original image.' }
        }
    }
    Write-Output 'Reference, Test, and Diff display matching rounded error images; original artifacts and baselines remain unchanged.'
}
