Add-Type -AssemblyName System.Drawing

function New-RoundedRectanglePath([float]$x, [float]$y, [float]$width, [float]$height, [float]$radius) {
    $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $diameter = $radius * 2
    $path.AddArc($x, $y, $diameter, $diameter, 180, 90)
    $path.AddArc($x + $width - $diameter, $y, $diameter, $diameter, 270, 90)
    $path.AddArc($x + $width - $diameter, $y + $height - $diameter, $diameter, $diameter, 0, 90)
    $path.AddArc($x, $y + $height - $diameter, $diameter, $diameter, 90, 90)
    $path.CloseFigure()
    return $path
}

$canvas = [System.Drawing.Bitmap]::new(512, 512)
$graphics = [System.Drawing.Graphics]::FromImage($canvas)
$graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
$graphics.Clear([System.Drawing.Color]::Transparent)

$backgroundPath = New-RoundedRectanglePath 20 20 472 472 112
$backgroundBrush = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
    [System.Drawing.Rectangle]::new(20, 20, 472, 472),
    [System.Drawing.Color]::FromArgb(24, 52, 61),
    [System.Drawing.Color]::FromArgb(40, 91, 98),
    45
)
$graphics.FillPath($backgroundBrush, $backgroundPath)
$borderPen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(140, 138, 177, 167), 6)
$graphics.DrawPath($borderPen, $backgroundPath)

$playPath = New-RoundedRectanglePath 131 126 250 154 54
$playBrush = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
    [System.Drawing.Rectangle]::new(131, 126, 250, 154),
    [System.Drawing.Color]::FromArgb(255, 98, 93),
    [System.Drawing.Color]::FromArgb(217, 56, 66),
    90
)
$graphics.FillPath($playBrush, $playPath)
$triangle = [System.Drawing.Point[]]@(
    [System.Drawing.Point]::new(226, 169),
    [System.Drawing.Point]::new(305, 203),
    [System.Drawing.Point]::new(226, 237)
)
$graphics.FillPolygon([System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(255, 248, 233)), $triangle)

$arrowPen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(255, 239, 199, 116), 30)
$arrowPen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
$arrowPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
$arrowPen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
$graphics.DrawLine($arrowPen, 256, 285, 256, 379)
$graphics.DrawLine($arrowPen, 190, 322, 256, 388)
$graphics.DrawLine($arrowPen, 256, 388, 322, 322)
$trayPen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(145, 196, 174), 23)
$trayPen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
$trayPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
$graphics.DrawLine($trayPen, 157, 417, 355, 417)

$iconPng = Join-Path $PSScriptRoot '..\resources\yt-dlp-ui.png'
$canvas.Save([System.IO.Path]::GetFullPath($iconPng), [System.Drawing.Imaging.ImageFormat]::Png)

$sizes = @(16, 24, 32, 48, 64, 128, 256)
$pngImages = [System.Collections.Generic.List[byte[]]]::new()
foreach ($size in $sizes) {
    $bitmap = [System.Drawing.Bitmap]::new($size, $size)
    $draw = [System.Drawing.Graphics]::FromImage($bitmap)
    $draw.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $draw.DrawImage($canvas, [System.Drawing.Rectangle]::new(0, 0, $size, $size))
    $stream = [System.IO.MemoryStream]::new()
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    $pngImages.Add($stream.ToArray())
    $stream.Dispose()
    $draw.Dispose()
    $bitmap.Dispose()
}

$icoStream = [System.IO.MemoryStream]::new()
$writer = [System.IO.BinaryWriter]::new($icoStream)
$writer.Write([UInt16]0)
$writer.Write([UInt16]1)
$writer.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($index = 0; $index -lt $sizes.Count; $index++) {
    $size = $sizes[$index]
    $writer.Write([byte]$(if ($size -eq 256) { 0 } else { $size }))
    $writer.Write([byte]$(if ($size -eq 256) { 0 } else { $size }))
    $writer.Write([byte]0)
    $writer.Write([byte]0)
    $writer.Write([UInt16]1)
    $writer.Write([UInt16]32)
    $writer.Write([UInt32]$pngImages[$index].Length)
    $writer.Write([UInt32]$offset)
    $offset += $pngImages[$index].Length
}
foreach ($png in $pngImages) { $writer.Write($png) }
$writer.Flush()
$iconPath = Join-Path $PSScriptRoot '..\resources\yt-dlp-ui.ico'
[System.IO.File]::WriteAllBytes([System.IO.Path]::GetFullPath($iconPath), $icoStream.ToArray())

$writer.Dispose()
$icoStream.Dispose()
$graphics.Dispose()
$canvas.Dispose()
