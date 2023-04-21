#!/usr/bin/env python3

import os
import subprocess
import sys
from src.installer_core import * # NOQA
#from src.installer_core import is_luks, ash_chroot, clear, deploy_base_snapshot, deploy_to_common, grub_ash, is_efi, post_bootstrap, pre_bootstrap, unmounts
from setup import args, distro

#   1. Define variables
is_format_btrfs = True ### REVIEW TEMPORARY
KERNEL = "" # options: https://wiki.archlinux.org/title/kernel e.g. "-xanmod"
packages = f"base linux{KERNEL} btrfs-progs sudo grub python3 python-anytree dhcpcd networkmanager nano linux-firmware" # os-prober bash tmux arch-install-scripts
if is_efi:
    packages += " efibootmgr"
if is_luks:
    packages += " cryptsetup" ### REVIEW_LATER
super_group = "wheel"
v = "" # GRUB version number in /boot/grubN

def initram_update():
    if is_luks:
        os.system(f"{SUDO} dd bs=512 count=4 if=/dev/random of=/mnt/etc/crypto_keyfile.bin iflag=fullblock")
        os.system(f"{SUDO} chmod 000 /mnt/etc/crypto_keyfile.bin") # Changed from 600 as even root doesn't need access
        os.system(f"{SUDO} cryptsetup luksAddKey {args[1]} /mnt/etc/crypto_keyfile.bin")
        os.system(f"{SUDO} sed -i -e '/^HOOKS/ s/filesystems/encrypt filesystems/' \
                        -e 's|^FILES=(|FILES=(/etc/crypto_keyfile.bin|' /mnt/etc/mkinitcpio.conf")
    if is_format_btrfs: ### REVIEW TEMPORARY
        os.system(f"{SUDO} sed -i 's|^MODULES=(|MODULES=(btrfs|' /mnt/etc/mkinitcpio.conf") ### TODO if array not empty, needs to be "btrfs "
    if is_luks or is_format_btrfs: ### REVIEW: does mkinitcpio need to be run without these conditions too?
        os.system(f"{SUDO} chroot /mnt {SUDO} mkinitcpio -p linux{KERNEL}")

def strap(pkg):
    while True:
        excode = os.system(f"{SUDO} pacstrap /mnt --needed {pkg}")
        if excode:
            if not yes_no("F: Failed to strap package(s). Retry?"): # User said no
                return 1
        else: # Success
            return 0

#   Pre bootstrap
pre_bootstrap()

#   2. Bootstrap and install packages in chroot
if KERNEL == "":
    excode = strap(packages)
else:
    if KERNEL not in ("-hardened", "-lts", "-zen"): # AUR needs to be enabled
        subprocess.call(f'./src/distros/{distro}/aur/aurutils.sh', shell=True)
        #subprocess.check_output(['./src/distros/arch/aur/aurutils.sh'])
    excode = os.system(f"pacman -Sqg base | sed 's/^linux$/&{KERNEL}/' | pacstrap /mnt --needed {packages}") ### TODO restructure code by appending to packages
if excode != 0:
    sys.exit("Failed to bootstrap!")

#   Mount-points for chrooting
ash_chroot()

#   3. Package manager database and config files
os.system(f"{SUDO} cp -r /mnt/var/lib/pacman/. /mnt/usr/share/ash/db/")
os.system("sed -i 's|[#?]DBPath.*$|DBPath       = /usr/share/ash/db/|g' /mnt/etc/pacman.conf")

#   4. Update hostname, hosts, locales and timezone, hosts
os.system(f"echo {hostname} | {SUDO} tee /mnt/etc/hostname")
os.system(f"echo 127.0.0.1 {hostname} {distro} | {SUDO} tee -a /mnt/etc/hosts")
#os.system(f"{SUDO} chroot /mnt {SUDO} localedef -v -c -i en_US -f UTF-8 en_US.UTF-8")
os.system(f"{SUDO} sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/locale.gen")
os.system(f"{SUDO} chroot /mnt {SUDO} locale-gen")
os.system(f"echo 'LANG=en_US.UTF-8' | {SUDO} tee /mnt/etc/locale.conf")
os.system(f"{SUDO} ln -srf /mnt/usr/share/zoneinfo/{tz} /mnt/etc/localtime")
os.system(f"{SUDO} chroot /mnt {SUDO} hwclock --systohc")

#   Post bootstrap
post_bootstrap(super_group)

#   5. Services (init, network, etc.)
os.system(f"{SUDO} chroot /mnt systemctl enable NetworkManager")

#   6. Boot and EFI
initram_update()
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

