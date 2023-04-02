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
        os.system("sudo chroot /mnt sudo mkinitcpio -p linux")

#   Define variables
ARCH = "x86_64" # Options: x86_64, i386, arm64
RELEASE = "" # empty for Glibc, "-musl" for MUSL ### define glibc and musl vars
DATE = "20210930"
#URL = "https://repo-default.voidlinux.org/live/current/"
packages = f"linux-mainline curl python3 python3-anytree \
             btrfs-progs NetworkManager sudo nano tmux" # os-prober firmware-linux-nonfree linux-image-{ARCH} dhcpcd
super_group = "wheel"
v = "" # GRUB version number in /boot/grubN
tz = get_timezone()
hostname = get_hostname()
#hostname = subprocess.check_output("git rev-parse --short HEAD", shell=True).decode('utf-8').strip() # Just for debugging

#   Pre bootstrap
pre_bootstrap()

#   Bootstrap and install packages in chroot
#subprocess.check_output("", shell=True)
excode = os.system(f"curl -o VoidLinux_Rootfs.tar.xz -LO https://repo-default.voidlinux.org/live/current/void-{ARCH}{RELEASE}-ROOTFS-{DATE}.tar.xz")
os.system("sudo tar xvf VoidLinux_Rootfs.tar.xz -C /mnt")
if excode != 0:
    sys.exit("Failed to bootstrap!")

#   Mount-points for chrooting
ash_chroot()

# Install anytree and necessary packages in chroot
os.system("sudo chroot /mnt sudo xbps-install -y -Su xbps") ### is sudo needed inside chroot, test. if not remove all below (update: NOT needed)
os.system("sudo chroot /mnt sudo xbps-install -y -u")
os.system("sudo chroot /mnt sudo xbps-install -y -Su")
os.system("sudo chroot /mnt sudo xbps-install -y base-system")
os.system("sudo chroot /mnt sudo xbps-remove -y base-voidstrap")
excode = os.system(f"sudo chroot /mnt sudo xbps-install -y {packages}")
if excode != 0:
    sys.exit("Failed to download packages!")
if is_efi:
    excode = os.system(f"sudo chroot /mnt xbps-install -y grub-{ARCH}-efi") ### efibootmgr does get installed. Does this do it?
    if excode != 0:
        sys.exit("Failed to install grub!")
else:
    excode = os.system("sudo chroot /mnt xbps-install -y grub")
    if excode != 0:
        sys.exit("Failed to install grub!")

#   Package manager database and config files
os.system("sudo mv /mnt/var/db/xbps /mnt/usr/share/ash/db/")
os.system("sudo ln -srf /mnt/usr/share/ash/db/xbps /mnt/var/db/xbps")

#   Update hostname, hosts, locales and timezone, hosts
os.system(f"echo {hostname} | sudo tee /mnt/etc/hostname")
os.system(f"echo 127.0.0.1 {hostname} {distro} | sudo tee -a /mnt/etc/hosts")
#os.system("sudo chroot /mnt sudo localedef -v -c -i en_US -f UTF-8 en_US.UTF-8")
if RELEASE == "": # For glibc variant ### what about musl?
    os.system("sudo sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/default/libc-locales")
    os.system("sudo chroot /mnt sudo xbps-reconfigure -f glibc-locales")
elif RELEASE == "-musl":
    print("TODO")
os.system("echo 'LANG=en_US.UTF-8' | sudo tee /mnt/etc/locale.conf")
os.system(f"sudo ln -srf /mnt{tz} /mnt/etc/localtime")
os.system("sudo chroot /mnt sudo hwclock --systohc")

#   Post bootstrap
post_bootstrap(distro, super_group)

#   Services (init, network, etc.)
os.system("sudo chroot /mnt ln -s /etc/sv/NetworkManager /etc/runit/runsvdir/default/")

#   Boot and EFI
initram_update_luks()
grub_ash(distro, v)

#   BTRFS snapshots
deploy_base_snapshot()

#   Copy boot and etc: deployed snapshot <---> common
deploy_to_common()

#   Unmount everything and finish
os.system("sudo chroot /mnt sudo xbps-reconfigure -fa")
unmounts()

clear()
print("Installation complete!")
print("You can reboot now :)")

