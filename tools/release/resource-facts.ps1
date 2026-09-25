[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$ExePath)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$version = [Diagnostics.FileVersionInfo]::GetVersionInfo($ExePath)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class StickyMdIconProbe {
    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    public static extern uint ExtractIconEx(string file, int index, IntPtr[] large, IntPtr[] small, uint count);
    [DllImport("user32.dll")]
    public static extern bool DestroyIcon(IntPtr icon);
}
'@
$large = [IntPtr[]]::new(1)
$small = [IntPtr[]]::new(1)
try {
    $icons = [StickyMdIconProbe]::ExtractIconEx($ExePath, 0, $large, $small, 1)
    [ordered]@{
        ProductName = $version.ProductName
        FileDescription = $version.FileDescription
        OriginalFilename = $version.OriginalFilename
        LegalCopyright = $version.LegalCopyright
        FileMajorPart = $version.FileMajorPart
        FileMinorPart = $version.FileMinorPart
        FileBuildPart = $version.FileBuildPart
        ProductMajorPart = $version.ProductMajorPart
        ProductMinorPart = $version.ProductMinorPart
        ProductBuildPart = $version.ProductBuildPart
        IconCount = $icons
    } | ConvertTo-Json -Compress
} finally {
    foreach ($handle in @($large[0], $small[0])) {
        if ($handle -ne [IntPtr]::Zero) { [void][StickyMdIconProbe]::DestroyIcon($handle) }
    }
}
