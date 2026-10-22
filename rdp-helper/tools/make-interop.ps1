# Generates MSTSCLib.dll (typed .NET bindings for the Windows Remote Desktop control, mstscax.dll).
# Run with Windows PowerShell 5.1: TypeLibConverter only exists in the .NET Framework.
param([string]$Out = (Join-Path $PSScriptRoot "..\lib"))

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force $Out | Out-Null

Add-Type -TypeDefinition @"
using System;
using System.Reflection;
using System.Runtime.InteropServices;
public class TlbSink : ITypeLibImporterNotifySink {
    public void ReportEvent(ImporterEventKind k, int c, string m) { Console.WriteLine(m); }
    public Assembly ResolveRef(object t) { return null; }
}
public static class Tlb {
    [DllImport("oleaut32.dll", CharSet = CharSet.Unicode, PreserveSig = false)]
    public static extern void LoadTypeLibEx(string file, int kind, [MarshalAs(UnmanagedType.Interface)] out object lib);
}
"@

$lib = $null
[Tlb]::LoadTypeLibEx("$env:windir\System32\mstscax.dll", 2, [ref]$lib)
$conv = New-Object System.Runtime.InteropServices.TypeLibConverter
$asm = $conv.ConvertTypeLibToAssembly($lib, (Join-Path $Out "MSTSCLib.dll"), [System.Runtime.InteropServices.TypeLibImporterFlags]::None, (New-Object TlbSink), $null, $null, "MSTSCLib", $null)
$asm.Save("MSTSCLib.dll")
Move-Item -Force (Join-Path (Get-Location) "MSTSCLib.dll") (Join-Path $Out "MSTSCLib.dll") -ErrorAction SilentlyContinue
Get-Item (Join-Path $Out "MSTSCLib.dll")
