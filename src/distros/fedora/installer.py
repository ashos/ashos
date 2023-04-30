#!/usr/bin/env python3

import os
import subprocess as sp
import sys
from src.installer_core import * # NOQA
from setup import args, distro

def main():
    #   1. Define variables
    ARCH = "x86_64"
    RELEASE = "rawhide"
    packages = "kernel dnf passwd sudo btrfs-progs python-anytree sqlite-tools linux-firmware \
                glibc-langpack-en glibc-locale-source dhcpcd NetworkManager"
    if is_efi:
        packages += " efibootmgr"
        if "64" in ARCH: # REVIEW not good for AARCH64/ARM64
            packages += " shim-x64 grub2-efi-x64-modules"
    super_group = "wheel"
    v = "2" # GRUB version number in /boot/grubN

    #   Pre bootstrap
    pre_bootstrap()

    #   2. Bootstrap and install packages in chroot
    while True:
        try:
            strap(packages, ARCH, RELEASE)
        except sp.CalledProcessError as e:
            print(e)
            if not yes_no("F: Failed to strap package(s). Retry?"):
                unmounts("failed") # user declined
                sys.exit("F: Install failed!")
        else: # success
            break

    # Mount-points for chrooting
    ashos_mounts()
    cur_dir_code = chroot_in("/mnt")

    #   3. Package manager database and config files
    os.system('echo "kernel.printk=4" >> /etc/sysctl.d/10-kernel-printk.conf') # https://github.com/coreos/fedora-coreos-tracker/issues/220
    # If rpmdb is under /usr, move it to /var and create a symlink
    if os.path.islink("/var/lib/dnf") or os.path.isfile("/usr/lib/sysimage/dnf/history.sqlite"):
        os.system("rm -r /var/lib/dnf")
        os.system("mv /usr/lib/sysimage/dnf /var/lib/")
        os.system("ln -s /usr/lib/sysimage/dnf /var/lib/dnf")
    if os.path.islink("/var/lib/rpm") or os.path.isfile("/usr/lib/sysimage/rpm/rpmdb.sqlite"):
        os.system("rm -r /var/lib/rpm")
        os.system("mv /usr/lib/sysimage/rpm /var/lib/")
        os.system("ln -s /usr/lib/sysimage/rpm /var/lib/rpm")
    os.system("cp -a /var/lib/dnf /usr/share/ash/db/") # REVIEW mv and symlink if this is not working
    os.system("cp -a /var/lib/rpm /usr/share/ash/db/")
    os.system('echo persistdir="/usr/share/ash/db/dnf" >> /etc/dnf/dnf.conf') # REVIEW not sure!
    os.system(f"echo 'releasever={RELEASE}' > /etc/yum.conf") # REVIEW needed?

    #   4. Update hostname, hosts, locales and timezone, hosts
    os.system(f"echo {hostname} > /etc/hostname")
    os.system(f"echo 127.0.0.1 {hostname} {distro} >> /etc/hosts")
    os.system("localedef -v -c -i en_US -f UTF-8 en_US.UTF-8")
    #os.system("sudo sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/locale.gen")
    #os.system("sudo chroot /mnt sudo locale-gen")
    os.system("echo 'LANG=en_US.UTF-8' > /etc/locale.conf")
    os.system(f"ln -sf /usr/share/zoneinfo/{tz} /etc/localtime")
    os.system("hwclock --systohc")

    #   Post bootstrap
    post_bootstrap(super_group)

    #   5. Services (init, network, etc.)
    os.system("systemctl enable NetworkManager")
    os.system("systemctl disable rpmdb-migrate") # https://fedoraproject.org/wiki/Changes/RelocateRPMToUsr

    #   6. Boot and EFI
    initram_update()
    #	For now non-BLS format is used (Entries go in /boot/grub2/grub.cfg not in /boot/loader/entries/)
    os.system('grep -qxF GRUB_ENABLE_BLSCFG="false" /etc/default/grub || \
            echo GRUB_ENABLE_BLSCFG="false" >> /etc/default/grub')
    if is_efi: # This needs to go before grub_ash otherwise map.txt entry would be empty
        os.system(f"efibootmgr -c -d {args[2]} -p 1 -L 'Fedora' -l '\\EFI\\fedora\\shim.efi'")
    grub_ash(v)

    #   BTRFS snapshots
    deploy_base_snapshot()

    #   Copy boot and etc: deployed snapshot <---> common
    deploy_to_common()

    #   Unmount everything and finish
    chroot_out(cur_dir_code)
    if is_ash_bundle:
        bundler()
    unmounts()

    clear()
    print("Installation complete!")
    print("You can reboot now :)")

def initram_update():
    if is_luks:
        os.system("dd bs=512 count=4 if=/dev/random of=/etc/crypto_keyfile.bin iflag=fullblock")
        os.system("chmod 000 /etc/crypto_keyfile.bin") # Changed from 600 as even root doesn't need access
        os.system(f"cryptsetup luksAddKey {args[1]} /etc/crypto_keyfile.bin")
        os.system("sed -i -e '/^HOOKS/ s/filesystems/encrypt filesystems/' \
                          -e 's|^FILES=(|FILES=(/etc/crypto_keyfile.bin|' /etc/mkinitcpio.conf")
        #os.system(f"mkinitcpio -p linux{KERNEL}") # TODO

def strap(pkg, ARCH, RELEASE):
    sp.check_call(f"dnf -c {installer_dir}/src/distros/fedora/base.repo --installroot=/mnt install -y {pkg} --releasever={RELEASE} --forcearch={ARCH}", shell=True)

main()

