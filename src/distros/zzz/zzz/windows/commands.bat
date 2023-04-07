REM Windows delete search index
del C:\ProgramData\Microsoft\Search\Data\Applications\Windows\Windows.db
del C:\ProgramData\Microsoft\Search\Data\Applications\Windows\Windows.edb

REM Enable WSL (https://learn.microsoft.com/en-us/windows/wsl/install-on-server)
echo Y | powershell Enable-WindowsOptionalFeature -FeatureName Microsoft-Windows-Subsystem-Linux  -Online -NoRestart
REM dism /online /enable-feature /featurename:Microsoft-Windows-Subsystem-Linux /all /norestart
REM wsl --install REM newer versions of Windows 10/11 (disadvantage: auto installs ubuntu WSL)

REM Enable Windows Sandbox
echo Y | powershell Enable-WindowsOptionalFeature -FeatureName Containers-DisposableClientVM -All -Online -NoRestart

REM Install Scoop
powershell Set-ExecutionPolicy RemoteSigned -Scope CurrentUser REM Optional: Needed to run a remote script the first time
powershell irm get.scoop.sh | iex

REM Install Oracle VirtualBox (WinGet)
winget install -e --id Oracle.VirtualBox

REM Install Oracle VirtualBox (Scoop)
scoop bucket add nonportable
scoop install virtualbox-np

REM Install Hyper-V
echo Y | powershell Enable-WindowsOptionalFeature -FeatureName Microsoft-Hyper-V -All -Online -NoRestart
REM DISM /Online /Enable-Feature /All /FeatureName:Microsoft-Hyper-V
REM bcdedit /set hypervisorlaunchtype off|on REM to temporarily disable/enable hyper-v at boot time

REM ----- After connecting to Wifi ----
REM Most likely not necessary at all
if X86_64:
    https://wslstorestorage.blob.core.windows.net/wslblob/wsl_update_x64.msi
elif ARM64:
    https://wslstorestorage.blob.core.windows.net/wslblob/wsl_update_arm64.msi

scoop install vncviewer
scoop install anydesk
scoop install openshot
scoop install yt-dlp
scoop install mpv
scoop install which
scoop install crystaldiskinfo


REM chezmoi add these:
REM Linux: $HOME/.anydesk/user.conf
REM Windows: C:\Users\Me\AppData\Roaming\AnyDesk\user.conf


REM Enable network discovery
netsh advfirewall firewall set rule group="Network Discovery" new enable=Yes
REM Get-Service
Start-Service fdPHost (not needed)
REM Start-Service FDResPub (not needed)
REM Run -> ms-settings:network

REM Enable network file and sharing https://go.microsoft.com/fwlink/?linkid=121488
netsh advfirewall firewall set rule group="File and Printer Sharing" new enable=Yes
REM netsh firewall set service type=fileandprint mode=enable profile=all (deprecated)

REM Added network printer through Microsoft Edge GUI (which actually just opens Windows Settings)

REM Add Persian keyboard layout with command TODO









REM what are these?
REM Get-Command-Module GroupPolicy
REM Add-WindowsCapability -Online -Name Rsat.GroupPolicy.Management.Tools~~~~0.0.1.0
REM scoop bucket add versions
