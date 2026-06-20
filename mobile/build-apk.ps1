#!/usr/bin/env pwsh
# Build the Cortex Mobile debug APK from the command line on Windows.
#
# Why this script exists (two non-obvious gotchas it handles for you):
#   1. AGP 8.7.3 + Gradle 8.11.1 REJECT JDK 23+. This box's PATH `java` is JDK 25, so the
#      build must point JAVA_HOME at Android Studio's embedded JBR (21). The script does that.
#   2. This machine has NoDefaultCurrentDirectoryInExePath set, so cmd/Gradle can't resolve
#      `gradlew.bat` from the cwd. The script invokes it by FULL PATH with `-p <projectDir>`.
#
# The gradle-wrapper.jar is gitignored (binary). If it's missing, open the project in Android
# Studio once (it regenerates on Sync) or run `gradle wrapper --gradle-version 8.11.1`.
#
# First run downloads Gradle 8.11.1 (~130 MB) + SDK Platform 35 / Build-Tools — needs network
# and an already-accepted android-sdk-license (Android Studio accepts it when you install any SDK).
#
# Usage:  pwsh mobile\build-apk.ps1 [task]        (default task: assembleDebug)

param([string]$Task = "assembleDebug")

$ErrorActionPreference = "Stop"
$mobile = $PSScriptRoot

# --- JDK: prefer Android Studio's embedded JBR (17/21); never system JDK 23+ ---
$jbr = @(
  "C:\Program Files\Android\Android Studio\jbr",
  "$env:LOCALAPPDATA\Programs\Android Studio\jbr"
) | Where-Object { Test-Path (Join-Path $_ "bin\java.exe") } | Select-Object -First 1
if (-not $jbr) {
  throw "Android Studio JBR not found. Install Android Studio, or set JAVA_HOME to a JDK 17-21 (NOT 23+)."
}
$env:JAVA_HOME = $jbr

# --- Android SDK ---
if (-not $env:ANDROID_HOME) {
  $sdk = "$env:LOCALAPPDATA\Android\Sdk"
  if (Test-Path $sdk) { $env:ANDROID_HOME = $sdk }
}

$gradlew = Join-Path $mobile "gradlew.bat"
if (-not (Test-Path $gradlew)) { throw "gradlew.bat missing in $mobile" }
if (-not (Test-Path (Join-Path $mobile "gradle\wrapper\gradle-wrapper.jar"))) {
  Write-Warning "gradle-wrapper.jar missing (gitignored). Open the project in Android Studio once to regenerate it, or run: gradle wrapper --gradle-version 8.11.1"
}

Write-Host "JAVA_HOME    = $env:JAVA_HOME"
Write-Host "ANDROID_HOME = $env:ANDROID_HOME"
Write-Host "Task         = $Task"
Write-Host ""

& cmd.exe /c "`"$gradlew`" -p `"$mobile`" $Task --stacktrace"
$code = $LASTEXITCODE

if ($code -eq 0) {
  $apk = Join-Path $mobile "app\build\outputs\apk\debug\app-debug.apk"
  if (Test-Path $apk) {
    Write-Host ""
    Write-Host "BUILD OK -> $apk"
    Write-Host ("Size: {0:N1} MB" -f ((Get-Item $apk).Length / 1MB))
    Write-Host "Install on a connected Pixel:  adb install -r `"$apk`""
  }
} else {
  Write-Host "Build failed (exit $code)."
}
exit $code
