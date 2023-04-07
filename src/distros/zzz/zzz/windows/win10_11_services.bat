GOTO EndComment
<!--*************************************************
Created using Windows AFG found at:
;http://www.windowsafg.com

Installation Notes
Location: 
Notes: Enter your comments here...

Option 	Description
Auto 	Specifies a service that automatically starts each time the computer is restarted and runs even if no one logs on to the computer.
Delayed-auto 	Specifies a service that starts automatically a short time after other auto services are started.
Demand 	Specifies a service that must be started manually.
Disabled 	Specifies a service that cannot be started.
**************************************************-->
:EndComment

echo Now configuring services.
REM ***** Disable Edge Update Service
sc config MicrosoftEdgeElevationService start= disabled
sc config edgeupdate start= disabled
sc config edgeupdatem start= disabled
REM *****
REM ***** Disable Windows Defender (should be in safe mode. Hold Shift click Power>Restart)
REM execute a bunch of reg files
REM ToDo boot to linux and rename all WD folders
REM *****
REM ***** Disable updates through lgpo (Computer Configurations > Admin Templates > Windows Components)
REM Always automatically restart at the scheduled time : Disabled
REM Configure Automatic Updates : Disabled
REM Allow updates over metered connections : Disabled
REM Do not connect to any Windows Update Internet locations : Disabled
REM Do not include drivers with Windows Updates : Enabled
REM *****

REM todo https://mirror.math.princeton.edu/pub/redcorelinux/amd64/iso/Redcore.Linux.Hardened.2201.Rastaban.KDE.amd64.iso
REM todo enable and show hibernate in power menu

REM sc config AxInstSV start= demand
REM sc config AJRouter start= demand
REM sc config AppReadiness start= demand
REM sc config AppIDSvc start= demand
REM sc config Appinfo start= demand
REM sc config ALG start= demand
REM sc config AppMgmt start= demand
REM sc config AppXSvc start= demand
REM sc config BITS start= delayed-auto
REM sc config BrokerInfrastructure start= auto
REM sc config BFE start= auto
REM sc config BDESVC start= demand
REM sc config wbengine start= demand
REM sc config BthHFSrv start= demand
REM sc config bthserv start= demand
REM sc config PeerDistSvc start= demand
REM sc config CDPSvc start= demand
REM sc config CertPropSvc start= demand
REM sc config ClipSVC start= demand
REM sc config KeyIso start= demand
REM sc config EventSystem start= auto
REM sc config COMSysApp start= demand
REM sc config Browser start= demand
REM sc config CoreMessagingRegistrar start= auto
REM sc config VaultSvc start= demand
REM sc config CryptSvc start= auto
REM sc config DsSvc start= demand
REM sc config DcpSvc start= demand
REM sc config DcomLaunch start= auto
REM sc config DoSvc start= delayed-auto
REM sc config DeviceAssociationService start= demand
REM sc config DeviceInstall start= demand
REM sc config DmEnrollmentSvc start= demand
REM sc config DsmSvc start= demand
REM sc config DevQueryBroker start= demand
REM sc config Dhcp start= auto
REM sc config DPS start= auto
REM sc config WdiServiceHost start= demand
REM sc config WdiSystemHost start= demand
REM Telemetry disable
sc config DiagTrack start= disabled REM auto
sc config dmwappushservice start= disabled REM REVIEW Not sure if really required to disable telemetry?!
schtasks.exe /change /tn "Microsoft\Windows\Customer Experience Improvement Program\Consolidator" /disable
REM *****
REM sc config TrkWks start= auto
REM sc config MSDTC start= demand
REM sc config Dnscache start= auto
REM sc config MapsBroker start= delayed-auto
REM sc config embeddedmode start= demand
REM sc config EFS start= demand
REM sc config EntAppSvc start= demand
REM sc config EapHost start= demand
REM sc config Fax start= demand
REM sc config fhsvc start= demand
REM sc config fdPHost start= demand
REM sc config FDResPub start= demand
REM sc config lfsvc start= demand
REM sc config gpsvc start= auto
REM sc config HomeGroupListener start= demand
REM sc config HomeGroupProvider start= demand
REM sc config hidserv start= demand
REM sc config vmickvpexchange start= demand
REM sc config vmicguestinterface start= demand
REM sc config vmicshutdown start= demand
REM sc config vmicheartbeat start= demand
REM sc config vmicrdv start= demand
REM sc config vmictimesync start= demand
REM sc config vmicvmsession start= demand
REM sc config vmicvss start= demand
REM sc config IKEEXT start= demand
REM sc config UI0Detect start= demand
REM sc config SharedAccess start= demand
REM sc config IEEtwCollectorService start= demand
REM sc config iphlpsvc start= auto
REM sc config PolicyAgent start= demand
REM sc config KtmRm start= demand
REM sc config lltdsvc start= demand
REM sc config LSM start= auto
REM sc config diagnosticshub.standardcollector.service start= demand
sc config wlidsvc start= disabled REM demand
REM sc config MSiSCSI start= demand
REM sc config NgcSvc start= demand
REM sc config NgcCtnrSvc start= demand
REM sc config swprv start= demand
REM sc config smphost start= demand
REM sc config SmsRouter start= demand
REM sc config NetTcpPortSharing start= disabled
REM sc config Netlogon start= demand
REM sc config NcdAutoSetup start= demand
REM sc config NcbService start= demand
REM sc config Netman start= demand
REM sc config NcaSvc start= demand
REM sc config netprofm start= demand
REM sc config NlaSvc start= auto
REM sc config NetSetupSvc start= demand
REM sc config nsi start= auto
REM sc config CscService start= demand
REM sc config defragsvc start= demand
REM sc config PNRPsvc start= demand
REM sc config p2psvc start= demand
REM sc config p2pimsvc start= demand
REM sc config pla start= demand
REM sc config PlugPlay start= demand
REM sc config PNRPAutoReg start= demand
REM sc config WPDBusEnum start= demand
REM sc config Power start= auto
REM sc config Spooler start= auto
REM sc config PrintNotify start= demand
REM sc config wercplsupport start= demand
REM sc config PcaSvc start= demand
REM sc config QWAVE start= demand
REM sc config RasAuto start= demand
REM sc config RasMan start= demand
REM sc config SessionEnv start= demand
REM sc config TermService start= demand
REM sc config UmRdpService start= demand
REM sc config RpcSs start= auto
REM sc config RpcLocator start= demand
REM sc config RemoteRegistry start= disabled
REM sc config RetailDemo start= demand
REM sc config RemoteAccess start= disabled
REM sc config RpcEptMapper start= auto
REM sc config seclogon start= demand
REM sc config SstpSvc start= demand
REM sc config SamSs start= auto
REM sc config wscsvc start= delayed-auto
REM sc config SensorDataService start= demand
REM sc config SensrSvc start= demand
REM sc config SensorService start= demand
REM sc config LanmanServer start= auto
REM sc config ShellHWDetection start= auto
REM sc config SCardSvr start= disabled
REM sc config ScDeviceEnum start= demand
REM sc config SCPolicySvc start= demand
REM sc config SNMPTRAP start= demand
REM sc config sppsvc start= delayed-auto
REM sc config svsvc start= demand
REM sc config SSDPSRV start= demand
REM sc config StateRepository start= demand
REM sc config WiaRpc start= demand
REM sc config StorSvc start= demand
REM sc config SysMain start= auto
REM sc config SENS start= auto
REM sc config SystemEventsBroker start= auto
REM sc config Schedule start= auto
REM sc config lmhosts start= demand
REM sc config TapiSrv start= demand
REM sc config Themes start= auto
REM sc config tiledatamodelsvc start= auto
REM sc config TimeBroker start= demand
REM sc config TabletInputService start= demand
sc config UsoSvc start= disabled REM demand
REM sc config upnphost start= demand
REM sc config UserManager start= auto
REM sc config ProfSvc start= auto
REM sc config vds start= demand
REM sc config VSS start= demand
REM sc config WalletService start= demand
REM sc config WebClient start= demand
REM sc config AudioSrv start= auto
REM sc config AudioEndpointBuilder start= auto
REM sc config SDRSVC start= demand
REM sc config WbioSrvc start= demand
REM sc config WcsPlugInService start= demand
REM sc config wcncsvc start= demand
REM sc config Wcmsvc start= auto
REM sc config WdNisSvc start= demand
REM sc config WinDefend start= auto
REM sc config wudfsvc start= demand
REM sc config WEPHOSTSVC start= demand
REM sc config WerSvc start= demand
REM sc config Wecsvc start= demand
REM sc config EventLog start= auto
REM sc config MpsSvc start= auto
REM sc config FontCache start= auto
REM sc config StiSvc start= demand
REM sc config msiserver start= demand
REM sc config LicenseManager start= demand
REM sc config Winmgmt start= disabled
REM sc config WMPNetworkSvc start= demand
REM sc config icssvc start= demand
REM sc config TrustedInstaller start= demand
REM sc config WpnService start= demand
REM sc config WinRM start= demand
sc config WSearch start= disabled REM delayed-auto
REM sc config WSService start= demand
REM sc config W32Time start= demand
sc config wuauserv start= disabled REM demand
REM sc config WinHttpAutoProxySvc start= demand
REM sc config dot3svc start= demand
REM sc config Wlansvc start= demand
REM sc config wmiApSrv start= demand
REM sc config workfolderssvc start= demand
REM sc config LanmanWorkstation start= auto
REM sc config WwanSvc start= demand
REM sc config XblAuthManager start= demand
REM sc config XblGameSave start= demand
REM sc config XboxNetApiSvc start= demand

REM Mozilla Maintenance Service, demand
sc config MozillaMaintenance start= disabled


@echo off
SET /P QUESTION=Reboot computer now? (Y/N):
If /I %QUESTION%==Y goto reboot
echo Will not reboot. Now exiting command prompt.
timeout /t 5
exit
:reboot
shutdown -r -t 5

