param([string]$Root = (Split-Path -Parent $PSScriptRoot))
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName PresentationCore, WindowsBase
$names = @('previous', 'next', 'expand', 'collapse', 'close', 'smile', 'shield', 'add')
foreach ($name in $names) {
    [xml]$svg = Get-Content -Raw -LiteralPath (Join-Path $Root "assets/ui/$name.svg")
    $view = $svg.DocumentElement.GetAttribute('viewBox').Split(' ')
    $vw = [double]::Parse($view[2], [Globalization.CultureInfo]::InvariantCulture)
    $vh = [double]::Parse($view[3], [Globalization.CultureInfo]::InvariantCulture)
    $stream = [IO.MemoryStream]::new()
    $writer = [IO.BinaryWriter]::new($stream)
    $sizes = @(20,25,30,40,50,60,80)
    $writer.Write([byte]$sizes.Count)
    foreach ($size in $sizes) {
        $visual = [Windows.Media.DrawingVisual]::new()
        $dc = $visual.RenderOpen()
        # Figma's Smile vector is 18.6667 square in a centered 20px slot.
        $fit = if ($name -eq 'smile') { $size / 20 } else { $size / $vw }
        $offset = if ($name -eq 'smile') { ($size - $vw * $fit) / 2 } else { 0 }
        $transform = [Windows.Media.TransformGroup]::new()
        $transform.Children.Add([Windows.Media.ScaleTransform]::new($fit,$fit))
        $transform.Children.Add([Windows.Media.TranslateTransform]::new($offset,$offset))
        $dc.PushTransform($transform)
        foreach ($path in $svg.SelectNodes("//*[local-name()='path']")) {
            $geometry = [Windows.Media.Geometry]::Parse($path.GetAttribute('d'))
            $stroke = [double]::Parse($path.GetAttribute('stroke-width'),[Globalization.CultureInfo]::InvariantCulture)
            $pen = [Windows.Media.Pen]::new([Windows.Media.Brushes]::White,$stroke)
            $pen.StartLineCap = [Windows.Media.PenLineCap]::Round
            $pen.EndLineCap = [Windows.Media.PenLineCap]::Round
            $pen.LineJoin = [Windows.Media.PenLineJoin]::Round
            $dc.DrawGeometry($null,$pen,$geometry)
        }
        $dc.Pop()
        $dc.Close()
        $bitmap = [Windows.Media.Imaging.RenderTargetBitmap]::new($size,$size,96,96,[Windows.Media.PixelFormats]::Pbgra32)
        $bitmap.Render($visual)
        $pixels = [byte[]]::new($size*$size*4)
        $bitmap.CopyPixels($pixels,$size*4,0)
        $writer.Write([uint16]$size)
        $writer.Write([uint32]($size*$size))
        for ($i=3; $i -lt $pixels.Length; $i+=4) { $writer.Write($pixels[$i]) }
    }
    $writer.Flush()
    [IO.File]::WriteAllBytes((Join-Path $Root "assets/ui/$name.mask"),$stream.ToArray())
    $writer.Dispose()
}
