#!/usr/bin/python3

import os
import subprocess
import sys
from src.installer_core import * # NOQA
#from src.installer_core import is_luks, ash_chroot, clear, deploy_base_snapshot, deploy_to_common, get_hostname, get_timezone, grub_ash, is_efi, post_bootstrap, pre_bootstrap, unmounts
from setup import args, distro

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
RELEASE = "sid"
KERNEL = ""
packages = f"linux-image-{ARCH} btrfs-progs sudo curl python3 python3-anytree dhcpcd5 network-manager locales nano firmware-linux" # os-prober
super_group = "sudo"
v = "" # GRUB version number in /boot/grubN
tz = get_timezone()
hostname = get_hostname()
#hostname = subprocess.check_output("git rev-parse --short HEAD", shell=True).decode('utf-8').strip() # Just for debugging

#   Pre bootstrap
pre_bootstrap()

#   2. Bootstrap and install packages in chroot
excl = subprocess.check_output("dpkg-query -f '${binary:Package} ${Priority}\n' -W | grep -v 'required\|important' | awk '{print $1}'", shell=True).decode('utf-8').strip().replace("\n",",")
excode = os.system(f"sudo debootstrap --arch {ARCH} --exclude={excl} {RELEASE} /mnt http://ftp.debian.org/debian") ### --include={packages} ? --variant=minbase ?
if excode != 0:
    sys.exit("Failed to bootstrap!")

#   Mount-points for chrooting
ash_chroot()

# Install anytree and necessary packages in chroot
os.system("sudo systemctl start ntp && sleep 30s && ntpq -p") # Sync time in the live iso
os.system(f"echo 'deb [trusted=yes] http://www.deb-multimedia.org {RELEASE} main' | sudo tee -a /mnt/etc/apt/sources.list.d/multimedia.list{DEBUG}")
os.system("sudo chroot /mnt apt-get -y update -oAcquire::AllowInsecureRepositories=true")
os.system("sudo chroot /mnt apt-get -y install deb-multimedia-keyring --allow-unauthenticated")
excode = os.system(f"sudo chroot /mnt apt-get -y install --fix-broken {packages}")
if excode != 0:
    sys.exit("Failed to download packages!")
if is_efi:
    excode = os.system("sudo chroot /mnt apt-get -y install grub-efi") ### efibootmgr does get installed. Does this do it?
    if excode != 0:
        sys.exit("Failed to install grub!")
else:
    excode = os.system("sudo chroot /mnt apt-get -y install grub-pc")
    if excode != 0:
        sys.exit("Failed to install grub!")

#   3. Package manager database and config files
os.system("sudo mv /mnt/var/lib/dpkg /mnt/usr/share/ash/db/")
os.system("sudo ln -srf /mnt/usr/share/ash/db/dpkg /mnt/var/lib/dpkg")

#   4. Update hostname, hosts, locales and timezone, hosts
os.system(f"echo {hostname} | sudo tee /mnt/etc/hostname")
os.system(f"echo 127.0.0.1 {hostname} {distro} | sudo tee -a /mnt/etc/hosts")
#os.system("sudo chroot /mnt sudo localedef -v -c -i en_US -f UTF-8 en_US.UTF-8")
os.system("sudo sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/locale.gen")
os.system("sudo chroot /mnt sudo locale-gen")
os.system("echo 'LANG=en_US.UTF-8' | sudo tee /mnt/etc/locale.conf")
os.system(f"sudo ln -srf /mnt{tz} /mnt/etc/localtime")
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

