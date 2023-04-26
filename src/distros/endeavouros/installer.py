#!/usr/bin/env python3

import os
import subprocess
import sys
from src.installer_core import * # NOQA
from setup import args, distro

def main():
    #   1. Define variables
    is_format_btrfs = True # REVIEW temporary
    KERNEL = "" # options: https://wiki.archlinux.org/title/kernel e.g. "-xanmod"
    packages = f"base linux{KERNEL} btrfs-progs sudo grub python3 python-anytree dhcpcd networkmanager nano \
                linux-firmware" # os-prober bash tmux arch-install-scripts
    if is_efi:
        packages += " efibootmgr"
    if is_luks:
        packages += " cryptsetup" # REVIEW
    super_group = "wheel"
    v = "" # GRUB version number in /boot/grubN
    EOS_mirrorlist = "https://gitlab.com/endeavouros-filemirror/PKGBUILDS/-/raw/master/endeavouros-mirrorlist/endeavouros-mirrorlist"
    EOS_pacman = "https://raw.githubusercontent.com/endeavouros-team/EndeavourOS-calamares/main/calamares/files/pacman.conf"

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
        sys.exit("F: Install failed!")

    #   Mount-points for chrooting
    ashos_mounts()
    # If using EOS iso (# before: in section 3.)
    os.system(f"sudo cp -a /etc/pacman.d/{distro}-mirrorlist /mnt/etc/pacman.d/")
    os.system("sudo cp -a /etc/pacman.conf /mnt/etc/pacman.conf")
    cur_dir_code = chroot_in("/mnt")

    #   3. Package manager database and config files
    os.system("cp -r /var/lib/pacman/. /usr/share/ash/db/") # removed /mnt/XYZ from both paths and below
    # If using either Arch or EOS iso
    #os.system(f"sudo curl -L {EOS_mirrorlist} -o /etc/pacman.d/{distro}-mirrorlist")
    #os.system(f"sudo curl -L {EOS_pacman} -o /etc/pacman.conf")
    os.system("sed -i 's|[#?]DBPath.*$|DBPath       = /usr/share/ash/db/|g' /etc/pacman.conf")

    #   4. Update hostname, hosts, locales and timezone, hosts
    os.system(f"echo {hostname} | tee /etc/hostname")
    os.system(f"echo 127.0.0.1 {hostname} {distro} | tee -a /etc/hosts")
    #os.system(f"{SUDO} chroot /mnt {SUDO} localedef -v -c -i en_US -f UTF-8 en_US.UTF-8")
    os.system("sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /etc/locale.gen")
    os.system("locale-gen")
    os.system("echo 'LANG=en_US.UTF-8' | tee /etc/locale.conf")
    os.system(f"ln -srf /usr/share/zoneinfo/{tz} /etc/localtime") # removed /mnt/XYZ from both paths (and from all lines above)
    os.system("hwclock --systohc")

    #   Post bootstrap
    post_bootstrap(super_group)

    #   5. Services (init, network, etc.)
    #os.system("/usr/lib/systemd/system-generators/systemd-fstab-generator /run/systemd/generator '' ''") # REVIEW recommended as fstab changed. "systemctl daemon-reload"
    os.system("systemctl enable NetworkManager")

    #   6. Boot and EFI
    initram_update(KERNEL)
    grub_ash(v)

    #   BTRFS snapshots
    deploy_base_snapshot()

    #   Copy boot and etc: deployed snapshot <---> common
    deploy_to_common()

    #   Unmount everything and finish
    chroot_out(cur_dir_code)
    unmounts()

clear()
print("Installation complete!")
print("You can reboot now :)")

def initram_update(KERNEL): # REVIEW removed "{SUDO}" from all lines below
    if is_luks:
        os.system("dd bs=512 count=4 if=/dev/random of=/etc/crypto_keyfile.bin iflag=fullblock") # removed /mnt/XYZ from output (and from lines below)
        os.system("chmod 000 /etc/crypto_keyfile.bin") # Changed from 600 as even root doesn't need access
        os.system(f"cryptsetup luksAddKey {args[1]} /etc/crypto_keyfile.bin")
        os.system("sed -i -e '/^HOOKS/ s/filesystems/encrypt filesystems/' \
                        -e 's|^FILES=(|FILES=(/etc/crypto_keyfile.bin|' /etc/mkinitcpio.conf")
    if is_format_btrfs: # REVIEW temporary
        os.system(f"sed -i 's|^MODULES=(|MODULES=(btrfs|' /etc/mkinitcpio.conf") # TODO if array not empty, needs to be "btrfs "
    if is_luks or is_format_btrfs: # REVIEW mkinitcpio needed to run without these conditions too?
        os.system(f"mkinitcpio -p linux{KERNEL}")

def strap(pkg):
    while True:
        excode = os.system(f"{SUDO} pacstrap /mnt --needed {pkg}")
        if excode:
            if not yes_no("F: Failed to strap package(s). Retry?"):
                unmounts(revert=True)
                return 1 # User declined
        else: # Success
            return 0

if __name__ == "__main__":
    main()

