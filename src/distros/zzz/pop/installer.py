#!/usr/bin/python3

####### Use if 'use_other_iso' != "": instead of all these if Debian/Ubuntu this and that

import os
import subprocess
import sys
from src.installer_core import * # NOQA
#from src.installer_core import is_luks, ash_chroot, clear, deploy_base_snapshot, deploy_to_common, get_hostname, get_item_from_path, grub_ash, is_efi, post_bootstrap, pre_bootstrap, unmounts
from setup import args, distro, use_other_iso

def initram_update_luks():
    if is_luks:
        os.system("sudo dd bs=512 count=4 if=/dev/random of=/mnt/etc/crypto_keyfile.bin iflag=fullblock")
        os.system("sudo chmod 000 /mnt/etc/crypto_keyfile.bin") # Changed from 600 as even root doesn't need access
        os.system(f"sudo cryptsetup luksAddKey {args[1]} /mnt/etc/crypto_keyfile.bin")
        os.system("sudo sed -i -e '/^HOOKS/ s/filesystems/encrypt filesystems/' \
                        -e 's|^FILES=(|FILES=(/etc/crypto_keyfile.bin|' /mnt/etc/mkinitcpio.conf")
        os.system(f"sudo chroot /mnt sudo mkinitcpio -p linux{KERNEL}")

#   1. Define variables
ARCH = "amd64"
RELEASE = "jammy"
KERNEL = ""
packages = f"linux-image-generic btrfs-progs sudo curl python3 python3-anytree dhcpcd5 network-manager locales nano" # firmware-linux-nonfree os-prober
super_group = "sudo"
v = "" # GRUB version number in /boot/grubN
tz = get_item_from_path("timezone", "/usr/share/zoneinfo")
hostname = get_hostname()
#hostname = subprocess.check_output("git rev-parse --short HEAD", shell=True).decode('utf-8').strip() # Just for debugging

#   Pre bootstrap
pre_bootstrap()

#   2. Bootstrap and install packages in chroot
#excl = subprocess.check_output("dpkg-query -f '${binary:Package} ${Priority}\n' -W | grep -v 'required\|important' | awk '{print $1}'", shell=True).decode('utf-8').strip().replace("\n",",")
excode = os.system(f"sudo debootstrap --arch {ARCH} --variant=minbase {RELEASE} /mnt http://apt.pop-os.org/ubuntu") ### http://archive.ubuntu.com/ubuntu --print-debs --include={packages} ? TODO: --exclude={excl} causes errors
if excode != 0:
    sys.exit("Failed to bootstrap!")

#   Mount-points for chrooting
ash_chroot()

# Install anytree and necessary packages in chroot
os.system("sudo systemctl start chronyd") # Sync time in the live iso
os.system("sudo chronyd -q 'server 0.us.pool.ntp.org iburst'") ### TODO: && sleep 30s
#os.system("sudo systemctl start ntp && sleep 30s && ntpq -p") # Sync time in the live iso (uncomment if using Debian/Ubuntu iso)
os.system(f"echo 'deb [trusted=yes] http://www.deb-multimedia.org stable main' | sudo tee -a /mnt/etc/apt/sources.list.d/multimedia.list{DEBUG}")
os.system(f"sudo chroot /mnt apt-get -y install --fix-broken software-properties-common") # needed for add-apt-repository ### REVIEW Reorganize code & combine with packages
os.system("sudo cp -afr /etc/apt/sources.list* /mnt/etc/apt/") ### REVIEW moved from down (package db section)
os.system("apt-key adv --keyserver keyserver.ubuntu.com --recv-keys 63C46DF0140D738961429F4E204DD8AEC33A7AFF") ### Needed when using PopOs iso?
os.system("sudo chroot /mnt add-apt-repository -y universe") ### Needed?
os.system("sudo chroot /mnt apt-get -y update")
os.system("sudo chroot /mnt apt-get -y install deb-multimedia-keyring --allow-unauthenticated")
os.system("sudo chroot /mnt apt-get -y update -oAcquire::AllowInsecureRepositories=true") ### REVIEW swapped place with line above
excode = os.system(f"sudo chroot /mnt apt-get -y install --fix-broken {packages}")
if excode != 0:
    sys.exit("Failed to download packages!")
if is_efi:
    excode = os.system("sudo chroot /mnt apt-get -y install grub-efi") # includes efibootmgr
    if excode != 0:
        sys.exit("Failed to install grub!")
else:
    excode = os.system("sudo chroot /mnt apt-get -y install grub-pc")
    if excode != 0:
        sys.exit("Failed to install grub!")
# auto-remove packages at the end or include ash auto-remove function in ashpk.py

#   3. Package manager database and config files
#ZZZZZZZ os.system("sudo cp -afr /etc/apt/sources.list* /mnt/etc/apt/")
#os.system(f"sed 's/RELEASE/{RELEASE}/g' ./src/distros/{distro}/sources.list | sudo tee /mnt/etc/apt/sources.list") ### REVIEW here or right before/after bootstrapping? ### REVIEW Needed?
#os.system("sudo sed -i '/cdrom/d' /mnt/etc/apt/sources.list")
os.system("sudo mv /mnt/var/lib/dpkg /mnt/usr/share/ash/db/") ### how about /var/lib/apt ?
os.system("sudo ln -srf /mnt/usr/share/ash/db/dpkg /mnt/var/lib/dpkg")
#os.system(f"echo 'RootDir=/usr/share/ash/db/' | sudo tee -a /mnt/etc/apt/apt.conf") ### REVIEW I don't think this works?!

#   4. Update hostname, hosts, locales and timezone, hosts
os.system(f"echo {hostname} | sudo tee /mnt/etc/hostname")
os.system(f"echo 127.0.0.1 {hostname} {distro} | sudo tee -a /mnt/etc/hosts") ### {distro} might not be needed
#os.system("sudo chroot /mnt sudo localedef -v -c -i en_US -f UTF-8 en_US.UTF-8")
os.system("sudo sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/locale.gen")
os.system("sudo chroot /mnt sudo locale-gen")
os.system("echo 'LANG=en_US.UTF-8' | sudo tee /mnt/etc/locale.conf")
os.system(f"sudo ln -srf /mnt/usr/share/zoneinfo/{tz} /mnt/etc/localtime")
os.system("sudo chroot /mnt sudo hwclock --systohc")

#   Post bootstrap
post_bootstrap(super_group)

#   5. Services (init, network, etc.)
os.system("sudo chroot /mnt systemctl enable NetworkManager")

#   6. Boot and EFI
initram_update_luks()
grub_ash(v)

#   BTRFS snapshots
deploy_base_snapshot()

#   Copy boot and etc: deployed snapshot <---> common
deploy_to_common()

#   Unmount everything and finish
unmounts()

clear()
print("Installation complete!")
print("You can reboot now :)")

