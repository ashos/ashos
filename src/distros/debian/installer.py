#!/usr/bin/env python3

import os
import subprocess as sp
import sys
from src.installer_core import * # NOQA
from setup import args, distro

def main():
    #   1. Define variables
    ARCH = "amd64"
    RELEASE = "sid"
    KERNEL = ""
    packages = f"linux-image-{ARCH} network-manager btrfs-progs sudo curl dhcpcd5 locales nano" # console-setup firmware-linux firmware-linux-nonfree os-prober
    if not is_ash_bundle:
        packages +=  " python3 python3-anytree"
    if is_efi:
        packages += " grub-efi"  # includes efibootmgr
    else:
        packages += " grub-pc"
    if is_luks:
        packages += " cryptsetup cryptsetup-initramfs cryptsetup-run"
    super_group = "sudo"
    v = "" # GRUB version number in /boot/grubN

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

    #   Mount-points for chrooting
    ashos_mounts()
    os.system("sudo systemctl start ntp && sleep 30s && ntpq -p") # Sync time in the live iso
    cur_dir_code = chroot_in("/mnt")

    # Install anytree and necessary packages in chroot
    os.system(f"echo 'deb [trusted=yes] https://www.deb-multimedia.org {RELEASE} main' >> /etc/apt/sources.list.d/multimedia.list{DEBUG}")
    os.system("chmod 1777 /tmp") # Otherwise error "Couldn't create temporary file /tmp/apt.conf.XYZ" # REVIEW necessary after switching to chroot_in and chroot_out ?
    os.system("apt-get -y update -oAcquire::AllowInsecureRepositories=true")
    os.system("apt-get -y -f install deb-multimedia-keyring --allow-unauthenticated")
    os.system("apt-get -y full-upgrade --allow-unauthenticated") # REVIEW necessary?
    excode = os.system(f"apt-get -y install --no-install-recommends --fix-broken {packages}")
    if excode != 0:
        sys.exit("Failed to download packages!")

    #   3. Package manager database and config files
    os.system("sudo mv /var/lib/dpkg /usr/share/ash/db/") # removed /mnt/XYZ from both paths and below
    os.system("sudo ln -sf /usr/share/ash/db/dpkg /var/lib/dpkg")

    #   4. Update hostname, hosts, locales and timezone, hosts
    os.system(f"echo {hostname} | sudo tee /etc/hostname")
    os.system(f"echo 127.0.0.1 {hostname} {distro} | sudo tee -a /etc/hosts")
    #os.system("sudo chroot /mnt sudo localedef -v -c -i en_US -f UTF-8 en_US.UTF-8")
    os.system("sudo sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /etc/locale.gen")
    os.system("locale-gen")
    os.system("echo 'LANG=en_US.UTF-8' | sudo tee /etc/locale.conf")
    os.system(f"sudo ln -sf /usr/share/zoneinfo/{tz} /etc/localtime")
    os.system("hwclock --systohc")

    #   Post bootstrap
    post_bootstrap(super_group)

    #   5. Services (init, network, etc.)
    os.system("systemctl enable NetworkManager")

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

def initram_update(): # REVIEW removed "{SUDO}" from all lines below
    if is_luks:
        os.system("dd bs=512 count=4 if=/dev/random of=/etc/crypto_keyfile.bin iflag=fullblock") # removed /mnt/XYZ from output (and from lines below)
        os.system("chmod 000 /etc/crypto_keyfile.bin") # Changed from 600 as even root doesn't need access
        os.system(f"cryptsetup luksAddKey {args[1]} /etc/crypto_keyfile.bin")
        os.system("sed -i -e 's|^#KEYFILE_PATTERN=|KEYFILE_PATTERN='/etc/crypto_keyfile.bin'|' /etc/cryptsetup-initramfs/conf-hook")
        os.system("echo UMASK=0077 >> /etc/initramfs-tools/initramfs.conf")
        os.system(f"echo 'luks_root '{args[1]}'  /etc/crypto_keyfile.bin luks' | sudo tee -a /etc/crypttab")
        os.system(f"update-initramfs -u") # REVIEW: Need sudo inside? What about kernel variants?

def strap(pkg, ARCH, RELEASE):
    excl = sp.check_output("dpkg-query -f '${binary:Package} ${Priority}\n' -W | grep -v 'required\\|important' | awk '{print $1}'", shell=True).decode('utf-8').strip().replace("\n",",")
    sp.check_call(f"sudo debootstrap --arch {ARCH} --exclude={excl} {RELEASE} /mnt http://ftp.debian.org/debian", shell=True) # REVIEW --include={packages} ? --variant=minbase ?

main()

