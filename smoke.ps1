Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class Win32Probe {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindowExW(IntPtr parent, IntPtr after, string className, string windowName);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindowW(string className, string windowName);
}
"@

Get-Process cap-ime-switch -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 800

$a = Start-Process -FilePath "target\release\cap-ime-switch.exe" -PassThru
Start-Sleep -Seconds 1
$aliveA = -not $a.HasExited
$hwndMsg = [IntPtr]::new(-3)
$h1 = [Win32Probe]::FindWindowExW($hwndMsg, [IntPtr]::Zero, "CapIMESwitchTrayWindow", "CapIMESwitch")

$b = Start-Process -FilePath "target\release\cap-ime-switch.exe" -PassThru
Start-Sleep -Seconds 2

$aliveA2 = -not $a.HasExited
$aliveB = -not $b.HasExited
$msgBox = [Win32Probe]::FindWindowW("#32770", "CapIMESwitch")
$h2 = [Win32Probe]::FindWindowExW($hwndMsg, [IntPtr]::Zero, "CapIMESwitchTrayWindow", "CapIMESwitch")

Write-Output "first_alive=$aliveA2 hidewin1=$h1 hidewin2=$h2"
Write-Output "second_alive=$aliveB prompt_shown=$($msgBox -ne [IntPtr]::Zero)"

# 提权模式(以管理员身份启动)涉及 UAC 弹窗,不在自动化冒烟范围内;
# 此处仅只读查询计划任务存在性,供人工核对提权功能状态
$taskExists = (schtasks.exe /Query /TN "CapIMESwitchElevated" 2>$null) -match "CapIMESwitchElevated"
Write-Output "elevated_task_exists=$taskExists"

$b.CloseMainWindow() | Out-Null
Start-Sleep -Milliseconds 500
if (-not $b.HasExited) { Stop-Process -Id $b.Id -Force }
if ($aliveA2) { Stop-Process -Id $a.Id -Force }
Write-Output "cleaned"