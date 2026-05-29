param(
  [string]$OutputDir = (Join-Path (Get-Location) 'Docs\github-screenshots')
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Username = $env:AI_CHAT_SCREENSHOT_USERNAME
$Password = $env:AI_CHAT_SCREENSHOT_PASSWORD
if ([string]::IsNullOrWhiteSpace($Username) -or [string]::IsNullOrWhiteSpace($Password)) {
  throw 'Set AI_CHAT_SCREENSHOT_USERNAME and AI_CHAT_SCREENSHOT_PASSWORD before running this script.'
}

Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Win32 {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT {
    public int Left;
    public int Top;
    public int Right;
    public int Bottom;
  }

  [DllImport("user32.dll")]
  public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

  [DllImport("user32.dll")]
  public static extern bool SetForegroundWindow(IntPtr hWnd);

  [DllImport("user32.dll")]
  public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

  [DllImport("user32.dll")]
  public static extern bool SetCursorPos(int X, int Y);

  [DllImport("user32.dll")]
  public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);

  [DllImport("user32.dll")]
  public static extern bool PrintWindow(IntPtr hwnd, IntPtr hDC, uint nFlags);

  public const int SW_RESTORE = 9;
  public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
  public const uint MOUSEEVENTF_LEFTUP = 0x0004;
}
'@

function Wait-ForHandle {
  param(
    [Parameter(Mandatory=$true)][System.Diagnostics.Process]$Process,
    [int]$TimeoutSeconds = 30
  )

  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while ([DateTime]::UtcNow -lt $deadline) {
    $Process.Refresh()
    if ($Process.MainWindowHandle -ne [IntPtr]::Zero) {
      return $Process.MainWindowHandle
    }
    Start-Sleep -Milliseconds 500
  }

  throw "Timed out waiting for window handle."
}

function Focus-Window {
  param([Parameter(Mandatory=$true)][IntPtr]$Handle)
  [Win32]::ShowWindow($Handle, [Win32]::SW_RESTORE) | Out-Null
  [Win32]::SetForegroundWindow($Handle) | Out-Null
  Start-Sleep -Milliseconds 350
}

function Get-WindowRect {
  param([Parameter(Mandatory=$true)][IntPtr]$Handle)
  $rect = New-Object Win32+RECT
  [Win32]::GetWindowRect($Handle, [ref]$rect) | Out-Null
  [System.Drawing.Rectangle]::FromLTRB($rect.Left, $rect.Top, $rect.Right, $rect.Bottom)
}

function Capture-Window {
  param(
    [Parameter(Mandatory=$true)][IntPtr]$Handle,
    [Parameter(Mandatory=$true)][string]$Path,
    [int]$CropTop = 0,
    [int]$CropBottom = 0
  )

  $rect = $null
  for ($attempt = 0; $attempt -lt 10; $attempt++) {
    $candidate = Get-WindowRect -Handle $Handle
    if ($candidate.Width -gt 0 -and $candidate.Height -gt 0) {
      $rect = $candidate
      break
    }
    Start-Sleep -Milliseconds 500
  }
  if (-not $rect) {
    throw "Window rectangle was not ready for capture."
  }

  $bmp = New-Object System.Drawing.Bitmap $rect.Width, $rect.Height
  $gfx = [System.Drawing.Graphics]::FromImage($bmp)
  try {
    $gfx.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size, [System.Drawing.CopyPixelOperation]::SourceCopy)
  } finally {
    $gfx.Dispose()
  }

  $target = $bmp
  if ($CropTop -gt 0 -or $CropBottom -gt 0) {
    $cropHeight = $rect.Height - $CropTop - $CropBottom
    $crop = New-Object System.Drawing.Bitmap $rect.Width, $cropHeight
    $cg = [System.Drawing.Graphics]::FromImage($crop)
    try {
      $src = New-Object System.Drawing.Rectangle 0, $CropTop, $rect.Width, $cropHeight
      $dst = New-Object System.Drawing.Rectangle 0, 0, $rect.Width, $cropHeight
      $cg.DrawImage($bmp, $dst, $src, [System.Drawing.GraphicsUnit]::Pixel)
    } finally {
      $cg.Dispose()
      $bmp.Dispose()
    }
    $target = $crop
  }

  $target.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  $target.Dispose()
}

function Click-Rel {
  param(
    [Parameter(Mandatory=$true)][IntPtr]$Handle,
    [Parameter(Mandatory=$true)][int]$X,
    [Parameter(Mandatory=$true)][int]$Y
  )

  $rect = Get-WindowRect -Handle $Handle
  [Win32]::SetCursorPos($rect.Left + $X, $rect.Top + $Y) | Out-Null
  Start-Sleep -Milliseconds 80
  [Win32]::mouse_event([Win32]::MOUSEEVENTF_LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
  [Win32]::mouse_event([Win32]::MOUSEEVENTF_LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 250
}

function Type-Text {
  param([Parameter(Mandatory=$true)][string]$Text)
  [System.Windows.Forms.SendKeys]::SendWait($Text)
  Start-Sleep -Milliseconds 120
}

function Type-Keys {
  param([Parameter(Mandatory=$true)][string]$Keys)
  [System.Windows.Forms.SendKeys]::SendWait($Keys)
  Start-Sleep -Milliseconds 180
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
Get-ChildItem -LiteralPath $OutputDir -Filter '*.png' -File -ErrorAction SilentlyContinue | Remove-Item -Force

$releaseRoot = (Resolve-Path 'src-tauri\target\release').Path
$captureRoot = Join-Path $OutputDir '.capture-run'
if (Test-Path $captureRoot) {
  Remove-Item -LiteralPath $captureRoot -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $captureRoot | Out-Null
Get-ChildItem -LiteralPath $releaseRoot -File | Copy-Item -Destination $captureRoot -Force

$process = $null
try {
  $exe = Resolve-Path (Join-Path $captureRoot 'ai-chat.exe')
  $process = Start-Process -FilePath $exe -PassThru
  $handle = Wait-ForHandle -Process $process
  Focus-Window -Handle $handle
  Start-Sleep -Seconds 4

  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '01-setup.png')

  # Create the local account using direct clicks to keep focus stable.
  Click-Rel -Handle $handle -X 870 -Y 550
  Type-Keys '^{A}{BACKSPACE}'
  Type-Text $Username

  Click-Rel -Handle $handle -X 870 -Y 638
  Type-Keys '^{A}{BACKSPACE}'
  Type-Text $Password

  Click-Rel -Handle $handle -X 870 -Y 721
  Type-Keys '^{A}{BACKSPACE}'
  Type-Text $Password

  Click-Rel -Handle $handle -X 892 -Y 825
  Start-Sleep -Seconds 5

  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '02-lock.png')

  Focus-Window -Handle $handle

  Click-Rel -Handle $handle -X 870 -Y 550
  Type-Keys '^{A}{BACKSPACE}'
  Type-Text $Username

  Click-Rel -Handle $handle -X 870 -Y 638
  Type-Keys '^{A}{BACKSPACE}'
  Type-Text $Password

  Click-Rel -Handle $handle -X 892 -Y 825
  Start-Sleep -Seconds 4

  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '03-chat-home.png')

  # New chat dialog
  Click-Rel -Handle $handle -X 224 -Y 95
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '04-new-chat.png')
  Click-Rel -Handle $handle -X 1080 -Y 450
  Start-Sleep -Milliseconds 300

  # Chat drawer
  Click-Rel -Handle $handle -X 1260 -Y 24
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '05-chat-drawer.png')
  Click-Rel -Handle $handle -X 130 -Y 24
  Start-Sleep -Milliseconds 300

  # Models / Gallery / Backups / Diagnostics / Settings
  Click-Rel -Handle $handle -X 100 -Y 140
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '06-models.png')

  Click-Rel -Handle $handle -X 100 -Y 176
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '07-gallery.png')

  Click-Rel -Handle $handle -X 100 -Y 252
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '08-backups.png')

  Click-Rel -Handle $handle -X 100 -Y 288
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '09-diagnostics.png') -CropBottom 210

  Click-Rel -Handle $handle -X 100 -Y 214
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '10-settings.png')

  # Theme previews in settings
  Click-Rel -Handle $handle -X 474 -Y 323
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '11-theme-paper.png')

  Click-Rel -Handle $handle -X 590 -Y 323
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '12-theme-alpine.png')

  Click-Rel -Handle $handle -X 706 -Y 323
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '13-theme-clay.png')

  Click-Rel -Handle $handle -X 822 -Y 323
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '14-theme-mono.png')

  # Open more settings sections
  Click-Rel -Handle $handle -X 700 -Y 410
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '15-generation.png')

  Click-Rel -Handle $handle -X 700 -Y 468
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '16-paths.png')

  Click-Rel -Handle $handle -X 700 -Y 526
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '17-security.png')

  # Return to chat and capture additional states
  Click-Rel -Handle $handle -X 100 -Y 102
  Start-Sleep -Seconds 1
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '18-chat-night.png')

  # Lock screen
  Click-Rel -Handle $handle -X 1388 -Y 24
  Start-Sleep -Seconds 2
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '19-lock-screen.png')

  # Unlock once more for a final state
  Focus-Window -Handle $handle
  Type-Text $Username
  Type-Keys '{TAB}'
  Type-Text $Password
  Type-Keys '{ENTER}'
  Start-Sleep -Seconds 3
  Capture-Window -Handle $handle -Path (Join-Path $OutputDir '20-chat-final.png')
}
finally {
  if ($process -and -not $process.HasExited) {
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
  }
  if (Test-Path $captureRoot) {
    Remove-Item -LiteralPath $captureRoot -Recurse -Force -ErrorAction SilentlyContinue
  }
}
