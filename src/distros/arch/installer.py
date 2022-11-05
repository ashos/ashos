#!/usr/bin/python3

import os
import subprocess
import sys
from src.installer_core import * # NOQA
#from src.installer_core import is_luks, ash_chroot, clear, deploy_base_snapshot, deploy_to_common, get_hostname, get_timezone, grub_ash, is_efi, post_bootstrap, pre_bootstrap, unmounts
from setup import args, distro, use_other_iso

def initram_update():
    if is_luks:
        os.system("sudo dd bs=512 count=4 if=/dev/random of=/mnt/etc/crypto_keyfile.bin iflag=fullblock")
        os.system("sudo chmod 000 /mnt/etc/crypto_keyfile.bin") # Changed from 600 as even root doesn't need access
        os.system(f"sudo cryptsetup luksAddKey {args[1]} /mnt/etc/crypto_keyfile.bin")
        os.system("sudo sed -i -e '/^HOOKS/ s/filesystems/encrypt filesystems/' \
                        -e 's|^FILES=(|FILES=(/etc/crypto_keyfile.bin|' /mnt/etc/mkinitcpio.conf")
    if is_format_btrfs: ### REVIEW TEMPORARY
        os.system(f"sudo sed -i 's|^MODULES=(|MODULES=(btrfs|' /mnt/etc/mkinitcpio.conf") ### TODO if array not empty, needs to be "btrfs "
    if is_luks or is_format_btrfs: ### REVIEW: does mkinitcpio need to be run without these conditions too?
        os.system(f"sudo chroot /mnt sudo mkinitcpio -p linux{KERNEL}")

#   1. Define variables
is_format_btrfs = True ### REVIEW TEMPORARY
KERNEL = "" # options: https://wiki.archlinux.org/title/kernel
packages = f"base linux{KERNEL} btrfs-progs sudo grub python3 python-anytree dhcpcd networkmanager nano linux-firmware" # os-prober bash tmux arch-install-scripts
super_group = "wheel"
v = "" # GRUB version number in /boot/grubN
tz = get_timezone()
hostname = get_hostname()
#hostname = subprocess.check_output("git rev-parse --short HEAD", shell=True).decode('utf-8').strip() # Just for debugging

#   Pre bootstrap
pre_bootstrap()

#   2. Bootstrap and install packages in chroot
if KERNEL == "":
    excode = os.system(f"sudo pacstrap /mnt --needed {packages}")
else:
    if KERNEL not in ("-hardened", "-lts", "-zen"): # AUR needs to be enabled
        subprocess.call(f'./src/distros/{distro}/aur/aurutils.sh', shell=True)
        #subprocess.check_output(['./src/distros/arch/aur/aurutils.sh'])
    excode = os.system(f"pacman -Sqg base | sed 's/^linux$/&{KERNEL}/' | pacstrap /mnt --needed {packages}")
if excode != 0:
    sys.exit("Failed to bootstrap!")
if is_efi:
    excode = os.system("sudo pacstrap /mnt --needed efibootmgr")
    if excode != 0:
        sys.exit("Failed to download packages!")

#   Mount-points for chrooting
ash_chroot()

#   3. Package manager database and config files
os.system("sudo cp -r /mnt/var/lib/pacman/. /mnt/usr/share/ash/db/")
os.system("sed -i 's|[#?]DBPath.*$|DBPath       = /usr/share/ash/db/|g' /mnt/etc/pacman.conf")

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

