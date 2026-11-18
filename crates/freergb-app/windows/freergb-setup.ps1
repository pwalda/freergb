# SPDX-License-Identifier: GPL-2.0-or-later
# Run elevated by the freeRGB installer (windows/hooks.nsh).
param(
    [Parameter(Mandatory)]
    [ValidateSet('install-service', 'remove-service', 'find-openrgb', 'remove-openrgb', 'find-pawnio', 'install-pawnio', 'install-sensors', 'remove-sensors')]
    [string]$Action,
    # install-pawnio: uninstall the present PawnIO first.
    [switch]$Reinstall
)
$ErrorActionPreference = 'Stop'
$service = 'freeRGB-OpenRGB'
$openrgb = Join-Path $PSScriptRoot 'openrgb\OpenRGB.exe'
# Reads the CPU temperature through PawnIO as SYSTEM, for the app (crates/freergb-sensord).
$sensors = 'freeRGB-Sensors'
$sensord = Join-Path $PSScriptRoot 'freergb-sensord.exe'

function Get-Installed([string]$pattern) {
    # Both registry views: a 32-bit installer files its entry under WOW6432Node.
    Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
        Where-Object { $_.DisplayName -like $pattern } | Select-Object -First 1
}

# The NSIS hooks read this from stdout; no trailing newline.
function Out-Version([string]$version) {
    if ($version) { [Console]::Out.Write($version) } else { [Console]::Out.Write('unknown') }
}

switch ($Action) {
    'install-service' {
        # The official OpenRGB installer's service already serves the SDK port; freeRGB uses that.
        if (Get-Service -Name 'OpenRGB' -ErrorAction SilentlyContinue) { exit 0 }
        $bin = '"' + $openrgb + '"'
        if (Get-Service -Name $service -ErrorAction SilentlyContinue) {
            # PowerShell 5.1 has no Set-Service -BinaryPathName and mangles quotes passed to sc.exe.
            Set-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Services\$service" -Name ImagePath -Value $bin
            Set-Service -Name $service -StartupType Automatic
        } else {
            New-Service -Name $service -BinaryPathName $bin -DisplayName 'OpenRGB for freeRGB' `
                -Description 'The OpenRGB SDK server that freeRGB talks to.' -StartupType Automatic | Out-Null
        }
        # Come back after a crash.
        & sc.exe failure $service reset= 86400 actions= restart/5000/restart/5000/restart/60000 | Out-Null
        Start-Service -Name $service
    }
    'remove-service' {
        if (Get-Service -Name $service -ErrorAction SilentlyContinue) {
            Stop-Service -Name $service -Force -ErrorAction SilentlyContinue
            & sc.exe delete $service | Out-Null
        }
        # OpenRGB keeps a service's settings and logs beside its exe.
        Remove-Item (Join-Path $PSScriptRoot 'openrgb\service_config') -Recurse -Force -ErrorAction SilentlyContinue
    }
    'find-openrgb' {
        # Another OpenRGB installed for the whole PC: an installer entry or its service.
        $entry = Get-Installed 'OpenRGB*'
        if ($entry) { Out-Version $entry.DisplayVersion; exit 0 }
        if (Get-Service -Name 'OpenRGB' -ErrorAction SilentlyContinue) { Out-Version ''; exit 0 }
        exit 1
    }
    'remove-openrgb' {
        Stop-Service -Name 'OpenRGB' -Force -ErrorAction SilentlyContinue
        # A running OpenRGB window would hold its files and leave the uninstall half done.
        Get-Process OpenRGB -ErrorAction SilentlyContinue | Where-Object { $_.Path -ne $openrgb } |
            Stop-Process -Force -ErrorAction SilentlyContinue
        $entry = Get-Installed 'OpenRGB*'
        if ($entry) {
            if ($entry.PSChildName -match '^\{[0-9A-Fa-f-]{36}\}$') {
                $p = Start-Process msiexec.exe -ArgumentList '/x', $entry.PSChildName, '/qn', '/norestart' -Wait -PassThru
            } else {
                $cmd = if ($entry.QuietUninstallString) { $entry.QuietUninstallString } else { $entry.UninstallString }
                $p = Start-Process cmd.exe -ArgumentList '/c', $cmd -Wait -PassThru
            }
            # 3010: removed, reboot suggested.
            if ($p.ExitCode -notin 0, 3010) { exit $p.ExitCode }
        }
        # A service set up by hand has no uninstaller.
        if (Get-Service -Name 'OpenRGB' -ErrorAction SilentlyContinue) { & sc.exe delete 'OpenRGB' | Out-Null }
    }
    'find-pawnio' {
        $entry = Get-Installed 'PawnIO*'
        if ($entry) { Out-Version $entry.DisplayVersion; exit 0 }
        if (Get-Service -Name 'PawnIO' -ErrorAction SilentlyContinue) { Out-Version ''; exit 0 }
        exit 1
    }
    'install-pawnio' {
        # PawnIO is not ours to redistribute: fetch the official setup, pinned by checksum.
        $url = 'https://github.com/namazso/PawnIO.Setup/releases/download/2.2.0/PawnIO_setup.exe'
        $sha256 = '1F519A22E47187F70A1379A48CA604981C4FCF694F4E65B734AAA74A9FBA3032'
        $file = Join-Path $env:TEMP 'freeRGB-PawnIO_setup.exe'
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -Uri $url -OutFile $file -UseBasicParsing
        if ((Get-FileHash $file -Algorithm SHA256).Hash -ne $sha256) {
            Remove-Item $file -ErrorAction SilentlyContinue
            throw 'The PawnIO download does not match its checksum.'
        }
        if ($Reinstall) {
            # OpenRGB and the sensor service hold the driver open.
            Stop-Service -Name $service -Force -ErrorAction SilentlyContinue
            Stop-Service -Name $sensors -Force -ErrorAction SilentlyContinue
            $p = Start-Process $file -ArgumentList '-uninstall', '-silent' -Wait -PassThru
            if ($p.ExitCode -notin 0, 3010) { Remove-Item $file -ErrorAction SilentlyContinue; exit $p.ExitCode }
        }
        $p = Start-Process $file -ArgumentList '-install', '-silent' -Wait -PassThru
        Remove-Item $file -ErrorAction SilentlyContinue
        # 3010: installed, reboot suggested. 183 (ERROR_ALREADY_EXISTS): it was already there.
        if ($p.ExitCode -notin 0, 3010, 183) { exit $p.ExitCode }
        # OpenRGB looks for PawnIO devices only when it starts.
        if (Get-Service -Name $service -ErrorAction SilentlyContinue) { Restart-Service -Name $service -Force }
        elseif (Get-Service -Name 'OpenRGB' -ErrorAction SilentlyContinue) { Restart-Service -Name 'OpenRGB' -Force }
        # It would find PawnIO by itself within half a minute; now is sooner.
        if (Get-Service -Name $sensors -ErrorAction SilentlyContinue) { Restart-Service -Name $sensors -Force }
    }
    'install-sensors' {
        $bin = '"' + $sensord + '"'
        if (Get-Service -Name $sensors -ErrorAction SilentlyContinue) {
            Set-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Services\$sensors" -Name ImagePath -Value $bin
            Set-Service -Name $sensors -StartupType Automatic
        } else {
            New-Service -Name $sensors -BinaryPathName $bin -DisplayName 'freeRGB sensors' `
                -Description 'Reads the CPU temperature through PawnIO for freeRGB. It reads nothing else and takes no input.' `
                -StartupType Automatic | Out-Null
        }
        & sc.exe failure $sensors reset= 86400 actions= restart/5000/restart/5000/restart/60000 | Out-Null
        Start-Service -Name $sensors
    }
    'remove-sensors' {
        if (Get-Service -Name $sensors -ErrorAction SilentlyContinue) {
            Stop-Service -Name $sensors -Force -ErrorAction SilentlyContinue
            & sc.exe delete $sensors | Out-Null
        }
    }
}
exit 0
