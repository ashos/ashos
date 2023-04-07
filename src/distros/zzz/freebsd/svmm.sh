export DISTRIBUTIONS="kernel.txz base.txz localinstall.txz"
export INTERFACES="vmx0 vmx1"
export ZFSBOOT_VDEV_TYPE=stripe

# for esx vm
export ZFSBOOT_DISKS=da0

export ZFSBOOT_SWAP_SIZE=2g
export ZFSBOOT_CONFIRM_LAYOUT=0
export ZFSBOOT_SWAP_ENCRYPTION=YES
export ZFSBOOT_BOOT_TYPE="UEFI"
export nonInteractive="YES"

#!/bin/sh
#########################
# POSTINSTALL
#########################

MYHOST=paper
MYIP4=23

#########################
# start with resolving
#########################
# Not sure why we need to do it, but just do it
cat > /etc/resolv.conf << TMPRESOLVCONF
nameserver 10.0.0.232
nameserver 10.0.0.230
nameserver 10.0.0.231
TMPRESOLVCONF

cat >> /etc/hosts << TMPHOSTS
# don't rely on DNS for vital storage
10.0.100.3 nas-stor nas-stor.local.my.org
10.0.100.4 nas1-stor nas1-stor.local.my.org
TMPHOSTS

#########################
# freebsd-update
#########################
cat /etc/resolv.conf | tee -a /var/log/my-install.log
freebsd-update fetch --not-running-from-cron | tee -a /var/log/my-install.log
freebsd-update install | tee -a /var/log/my-install.log

#########################
# /etc/sysctl.conf
#########################
echo "Setting sysctl.conf" | tee -a /var/log/my-install.log

echo net.link.tap.up_on_open=1 >> /etc/sysctl.conf
echo security.jail.allow_raw_sockets=1 >> /etc/sysctl.conf

#########################
# prepare pkg system
#########################
mkdir -p /usr/local/etc/pkg/repos
cat >> /usr/local/etc/pkg/repos/FreeBSD.conf << PKGFREEBSDCONF
FreeBSD: {
  url: "pkg+http://pkg.FreeBSD.org/\${ABI}/latest"
}
PKGFREEBSDCONF
cat /usr/local/etc/pkg/repos/FreeBSD.conf | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg update -f | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg upgrade -q -y | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg audit -F | tee -a /var/log/my-install.log


#########################
# create network config.
#  Needs to dependant on macadres or IP or ...
#########################
sysrc hostname=${MYHOST}

sysrc defaultrouter="10.0.0.1"
sysrc ipv6_defaultrouter="2a06:2602:1d:40::1"

sysrc ifconfig_vmx0="inet 10.0.0.${MYIP4}/24"
sysrc ifconfig_vmx0_ipv6="inet6 2a06:2602:1d:40::${MYIP4}/64"
sysrc ifconfig_vmx1="inet 10.0.100.${MYIP4}/24"
sysrc ifconfig_vmx1_ipv6="inet6 2a06:2602:1d:100::${MYIP4}/64"

#########################
# SSH settings
#########################
sysrc sshd_enable=YES

cat >> /etc/ssh/sshd_config << SSHD_CONFIG
PermitRootLogin yes
SSHD_CONFIG

# copy host keys...
case "${MYHOST}" in
  paper)
    cp /localinstall/ssh/${MYHOST}/ssh_host* /etc/ssh/
    ;;
  *)
    echo "Unknown ssh host key config. Exiting..."
    exit 1
    ;;
esac

# create SSH environment
mkdir -m 700 -p /root/.ssh

cat > /root/.ssh/authorized_keys << ROOT_AUTHKEYS
XXX
ROOT_AUTHKEYS

#########################
# create time settings
#########################

cat > /etc/ntp.conf << NTPCONF
tos minclock 3 maxclock 6

server 10.0.0.1 iburst
server 10.0.0.7 iburst

restrict default limited kod nomodify notrap noquery nopeer
restrict source  limited kod nomodify notrap noquery
restrict 127.0.0.1
restrict ::1

leapfile "/var/db/ntpd.leap-seconds.list"
NTPCONF

sysrc ntpd_enable="YES"

# set timezone
ln -s /usr/share/zoneinfo/Europe/Amsterdam /etc/localtime

#########################
# create boot settings
#########################
# set loader.conf
cat >> /boot/loader.conf << LOADER_CONF
coretemp_load="YES"
aesni_load="YES"
vmm_load="YES"
if_bridge_load="YES"
#bridgestp_load="YES"
if_tap_load="YES"

cpu_microcode_load="YES"
cpu_microcode_name="/boot/firmware/intel-ucode.bin"
LOADER_CONF

#########################
# configure NFS
#########################
sysrc nfs_client_enable="YES"
sysrc rpc_lockd_enable="YES"
sysrc rpc_statd_enable="YES"

#########################
# configure fstab
#########################

echo "create zfs" | tee -a /var/log/my-install.log
zfs create -o mountpoint=/services zroot/services
zfs create zroot/services/X

zfs create -o mountpoint=/jails zroot/jails

echo "make mountpoints" | tee -a /var/log/my-install.log
# do this in /mnt since they are mounted here during install
# check bug 210804 for more info
mkdir -p \
  /mnt/services/X \
  | tee -a /var/log/my-install.log

ln -s /usr/home /home

cat >> /etc/fstab << FSTABENTRIES
fdescfs /dev/fd fdescfs rw 0 0

# nfs from flax, readonly
nas1-stor:/volume1/admin_shared /services/admin/shared nfs rw,nfsv3 0 0
...
FSTABENTRIES

#########################
# misc config settings
#########################
# screensaver
sysrc saver="green"
sysrc blanktime="300"

# powerd
sysrc powerd_enable="YES"

# misc misc
sysrc clear_tmp_enable="YES"

#########################
# create users and set rootpwd
#########################

...

#########################
# root mail in aliases
#########################
echo "root mail in aliases" | tee -a /var/log/my-install.log
echo "root: ...@my.org" >> /etc/aliases
newaliases | tee -a /var/log/my-install.log

#########################
# install packages
#########################
echo "install packages" | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg install -q -y devcpu-data | tee -a /var/log/my-install.log
# env ASSUME_ALWAYS_YES=YES pkg install -q -y py37-iocage | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg install -q -y bacula9-client | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg install -q -y sudo screen tmux | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg install -q -y ezjail | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg install -q -y gitup | tee -a /var/log/my-install.log
# env ASSUME_ALWAYS_YES=YES pkg install -q -y bhyve-firmware uefi-edk2-bhyve uefi-edk2-bhyve-csm | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg install -q -y lsof ca_root_nss | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg install -q -y bsnmp-ucd | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg autoremove | tee -a /var/log/my-install.log
env ASSUME_ALWAYS_YES=YES pkg clean -a | tee -a /var/log/my-install.log

#########################
# sudo configuration : wheel can sudo
#########################
echo "create allow_wheel for sudo" | tee -a /var/log/my-install.log
echo '%wheel ALL=(ALL) ALL' > /usr/local/etc/sudoers.d/allow_wheel

#########################
# bacula configuration
#########################
echo "configure bacula" | tee -a /var/log/my-install.log
sysrc bacula_fd_enable="YES"
# copy bacula config...
case "${MYHOST}" in
  paper)
    cp /localinstall/bacula/${MYHOST}/bacula-fd.conf /usr/local/etc/bacula/
    ;;
  *)
    echo "Unknown host for bacula config. Exiting..."
    exit 1
    ;;
esac

#########################
# devcpu-data configuration
#########################
echo "enable devcpu-data" | tee -a /var/log/my-install.log
sysrc microcode_update_enable="YES"

#########################
# ezjail configuration
#########################
echo "configure ezjail" | tee -a /var/log/my-install.log
sysrc ezjail_enable="YES"
# copy ezjail config
cat > /usr/local/etc/ezjail.conf << EZJAILCONF
# Note: If you have spread your jails to multiple locations, use softlinks
# to collect them in this directory
ezjail_jaildir=/jails

# Setting this to YES will start to manage the basejail and newjail in ZFS
ezjail_use_zfs="YES"

# Setting this to YES will manage ALL new jails in their own zfs
ezjail_use_zfs_for_jails="YES"

# The name of the ZFS ezjail should create jails on, it will be mounted at the ezjail_jaildir
ezjail_jailzfs="zroot/jails"

ezjail_default_flavour="salt"
EZJAILCONF

echo "Installing flavours" | tee -a /var/log/my-install.log
tar -C /localinstall/ezjail -cf - flavours | tar -C /mnt/jails -xvf - | tee -a /var/log/my-install.log

#########################
# bsnmpd configuration
#########################
echo "configure bsnmpd" | tee -a /var/log/my-install.log
sysrc bsnmpd_enable="YES"
# copy snmpd config...
case "${MYHOST}" in
  paper)
    cp /localinstall/bsnmpd/${MYHOST}/snmpd.config /etc/snmpd.config
    ;;
  *)
    echo "Unknown host for bsnmpd config. Exiting..."
    exit 1
    ;;
esac

#########################
# periodic.conf
#########################
echo "periodic.conf" | tee -a /var/log/my-install.log
touch /etc/periodic.conf
sysrc -f /etc/periodic.conf daily_clean_preserve_enable="NO"
sysrc -f /etc/periodic.conf daily_clean_msgs_enable="NO"
sysrc -f /etc/periodic.conf daily_clean_rwho_enable="NO"
sysrc -f /etc/periodic.conf daily_news_expire_enable="NO"
sysrc -f /etc/periodic.conf security_status_ipfwdenied_enable="NO"
sysrc -f /etc/periodic.conf security_status_ipfdenied_enable="NO"
sysrc -f /etc/periodic.conf security_status_pfdenied_enable="NO"
sysrc -f /etc/periodic.conf security_status_ipfwlimit_enable="NO"
sysrc -f /etc/periodic.conf security_status_ipf6denied_enable="NO"
sysrc -f /etc/periodic.conf security_status_tcpwrap_enable="NO"

#######
# TODO
#######
echo "log rotation?"
echo "fix /usr/local/etc/pkg/repos/FreeBSD.conf"
