# Installs (or updates) the HawaOS Companion on Windows from the latest
# GitHub release. In PowerShell:
#
#   irm https://raw.githubusercontent.com/TheCodeHeist/hawaos-lilygo/main/install.ps1 | iex
#
# A version can be given: $env:HAWAOS_VERSION = "0.9.0" first, or
#   & ([scriptblock]::Create((irm <url above>))) 0.9.0
#
# It downloads the release's setup .exe, checks it against the release's
# SHA256SUMS-windows, and runs it silently for your user only; no
# administrator rights needed. Run it again to update.
#
# Testing aid: $env:HAWAOS_SETUP = "<path to a setup .exe>" installs that file
# instead of downloading one, and $env:HAWAOS_NO_LAUNCH = "1" doesn't start
# the companion afterwards (the Windows workflow uses both).

& {
    param([string]$Version = $env:HAWAOS_VERSION)

    $ErrorActionPreference = "Stop"
    $ProgressPreference = "SilentlyContinue" # Windows PowerShell downloads crawl with the progress bar
    $Repo = "TheCodeHeist/hawaos-lilygo"
    # The installer's AppId (packaging/windows/hawaos-companion.iss).
    $UninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{56F6AE17-FD41-4877-9265-22F68EC5CBEA}_is1"

    function Say($msg) { Write-Host "==> " -ForegroundColor Cyan -NoNewline; Write-Host $msg }

    try {
        if (-not [Environment]::Is64BitOperatingSystem) {
            throw "Releases are built for 64-bit Windows only."
        }
        # Windows PowerShell 5.1 may not offer TLS 1.2, which GitHub requires.
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

        $tmp = Join-Path ([IO.Path]::GetTempPath()) ("hawaos-" + [Guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory -Path $tmp | Out-Null

        if ($env:HAWAOS_SETUP) {
            $setup = (Resolve-Path $env:HAWAOS_SETUP).Path
            Say "Using $setup"
        } else {
            if ($Version) {
                $api = "https://api.github.com/repos/$Repo/releases/tags/v$($Version.TrimStart('v'))"
            } else {
                $api = "https://api.github.com/repos/$Repo/releases/latest"
            }
            Say "Finding the release"
            try {
                $release = Invoke-RestMethod -Uri $api -Headers @{ "User-Agent" = "hawaos-install" }
            } catch {
                throw "Couldn't reach GitHub ($api)."
            }
            $asset = $release.assets | Where-Object { $_.name -like "hawaos-companion-*-windows-x64-setup.exe" } | Select-Object -First 1
            $sums = $release.assets | Where-Object { $_.name -eq "SHA256SUMS-windows" } | Select-Object -First 1
            if (-not $asset) { throw "Release $($release.tag_name) has no Windows installer." }

            $setup = Join-Path $tmp $asset.name
            Say "Downloading $($asset.name)"
            Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $setup -UseBasicParsing
            if ($sums) {
                $sumsText = (Invoke-WebRequest -Uri $sums.browser_download_url -UseBasicParsing).Content
                if ($sumsText -is [byte[]]) { $sumsText = [Text.Encoding]::ASCII.GetString($sumsText) }
                $line = $sumsText -split "`n" | Where-Object { $_ -match "\s\*?$([regex]::Escape($asset.name))\s*$" } | Select-Object -First 1
                if (-not $line) { throw "SHA256SUMS-windows doesn't list $($asset.name)." }
                $want = ($line -split "\s+")[0].ToLower()
                $got = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLower()
                if ($want -ne $got) { throw "The download doesn't match the release's SHA256SUMS-windows." }
                Say "Checksum OK"
            }
        }

        # The installer replaces the running companion; start it again after.
        $wasRunning = [bool](Get-Process hawaos-companion -ErrorAction SilentlyContinue)
        if ($wasRunning) {
            Say "Closing the running companion"
            Stop-Process -Name hawaos-companion -Force
            Start-Sleep -Milliseconds 500
        }

        Say "Installing"
        $log = Join-Path $tmp "install.log"
        $p = Start-Process -FilePath $setup -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-", "/LOG=`"$log`"" -Wait -PassThru
        if ($p.ExitCode -ne 0) {
            $keep = Join-Path ([IO.Path]::GetTempPath()) "hawaos-install.log"
            Copy-Item $log $keep -ErrorAction SilentlyContinue
            throw "The installer failed (exit code $($p.ExitCode)); its log is $keep."
        }

        $dir = (Get-ItemProperty $UninstallKey -ErrorAction SilentlyContinue).InstallLocation
        if (-not $dir) { $dir = Join-Path $env:LOCALAPPDATA "Programs\HawaOS Companion" }
        $exe = Join-Path $dir "hawaos-companion.exe"
        if (-not (Test-Path $exe)) { throw "The installer finished, but $exe isn't there." }
        $installed = (Get-ItemProperty $UninstallKey -ErrorAction SilentlyContinue).DisplayVersion

        if (-not $env:HAWAOS_NO_LAUNCH) {
            if ($wasRunning) {
                Start-Process -FilePath $exe -ArgumentList "--minimized"
            } else {
                Start-Process -FilePath $exe
            }
        }

        Write-Host ""
        Say "Done: HawaOS Companion $installed is installed, and opens from the Start menu."
        if (-not $wasRunning) {
            Say "On first launch, allow it through Windows Firewall on private networks (the device talks to it over Wi-Fi)."
        }
        Say "To update later, run this command again."
    } catch {
        Write-Host "Error: " -ForegroundColor Red -NoNewline
        Write-Host $_.Exception.Message
    } finally {
        if ($tmp -and (Test-Path $tmp)) { Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue }
    }
} @args
