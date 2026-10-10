# Smoke test for the Windows (Raw Input) backend. Run from the repository root
# after `cargo build -p manykey`. Starts `manykey watch --include-injected`,
# injects an "A" key press with SendInput, and checks it comes out the other end.
$ErrorActionPreference = "Stop"
$exe = Join-Path (Get-Location) "target\debug\manykey.exe"

Write-Host "== manykey list"
& $exe list
if ($LASTEXITCODE -ne 0) { throw "manykey list exited with $LASTEXITCODE" }

Add-Type @"
using System; using System.Runtime.InteropServices;
public class K {
  [StructLayout(LayoutKind.Sequential)] public struct KI { public ushort vk, scan; public uint flags, time; public IntPtr extra; }
  [StructLayout(LayoutKind.Explicit, Size=40)] public struct IN { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KI ki; }
  [DllImport("user32.dll")] static extern uint SendInput(uint n, IN[] i, int size);
  public static uint Send(ushort scan, bool up) {
    IN[] i = new IN[1]; i[0].type = 1; i[0].ki.scan = scan; i[0].ki.flags = (uint)(0x8 | (up ? 0x2 : 0));
    return SendInput(1, i, Marshal.SizeOf(typeof(IN)));
  }
}
"@

$out = Join-Path (Get-Location) "manykey-watch.out"
$p = Start-Process -FilePath $exe -ArgumentList "watch", "--include-injected" `
    -RedirectStandardOutput $out -PassThru -NoNewWindow
Start-Sleep -Seconds 3
$sent = [K]::Send(0x1E, $false) + [K]::Send(0x1E, $true)   # scancode 0x1E = the A key
Write-Host "SendInput accepted $sent of 2 events"
Start-Sleep -Seconds 2
Stop-Process -Id $p.Id -Force

$text = Get-Content $out -Raw
Write-Host "== manykey watch output"
Write-Host $text
if ($text -notmatch "A\s+down" -or $text -notmatch "A\s+up") {
    throw "expected an A key press and release in the output"
}
Write-Host "OK: injected key press was reported"
