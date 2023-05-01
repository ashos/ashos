#!/usr/bin/env python3

import os
import subprocess as sp
import sys
from src.installer_core import * # NOQA
from setup import args, distro

#   1. Define variables
ARCH = "amd64"
RELEASE = "sid"
KERNEL = ""
packages = f"linux-image-{ARCH} btrfs-progs sudo curl dhcpcd5 locales nano \
            network-manager" # console-setup firmware-linux firmware-linux-nonfree os-prober
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

def main():
    #   Pre bootstrap
    pre_bootstrap()

    #   Mount-points for chrooting
    ashos_mounts()

    #   2. Bootstrap and install packages in chroot
    os.system("systemctl start ntp && sleep 30s && ntpq -p") # Sync time in the live iso
    while True:
        try:
            strap()
        except sp.CalledProcessError as e:
            print(e)
            if not yes_no("F: Failed to strap package(s). Retry?"):
                unmounts("failed") # user declined
                sys.exit("F: Install failed!")
        else: # success
            break

    #   Go inside chroot
    cur_dir_code = chroot_in("/mnt")

    # Install anytree and necessary packages in chroot
    try:
        open("/etc/apt/sources.list.d/multimedia.list", "a").write(f"deb [trusted=yes] https://www.deb-multimedia.org {RELEASE} main")
        os.chmod("/tmp", 0o1777)
        # REVIEW /tmp Otherwise error "Couldn't create temporary file /tmp/apt.conf.XYZ" # REVIEW necessary after switching to chroot_in and chroot_out? third line below necessary?
        commands = f'''
        apt-get -y update -oAcquire::AllowInsecureRepositories=true
        apt-get -y -f install deb-multimedia-keyring --allow-unauthenticated
        apt-get -y full-upgrade --allow-unauthenticated
        apt-get -y install --no-install-recommends --fix-broken {packages}
        '''
        sp.check_call(commands, shell=True)
    except (Exception, sp.CalledProcessError, FileNotFoundError):
        sys.exit("Failed to download packages!")

    #   3. Package manager database and config files
    os.system("mv /var/lib/dpkg /usr/share/ash/db/")
    os.system("ln -sf /usr/share/ash/db/dpkg /var/lib/dpkg")

    #   4. Update hostname, hosts, locales and timezone, hosts
    os.system(f"echo {hostname} > /etc/hostname")
    os.system(f"echo 127.0.0.1 {hostname} {distro} >> /etc/hosts")
    #os.system("sudo chroot /mnt sudo localedef -v -c -i en_US -f UTF-8 en_US.UTF-8")
    os.system("sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /etc/locale.gen")
    os.system("locale-gen")
    os.system("echo 'LANG=en_US.UTF-8' > /etc/locale.conf")
    os.system(f"ln -sf /usr/share/zoneinfo/{tz} /etc/localtime")
    os.system("/sbin/hwclock --systohc")

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
        os.system("sed -i -e 's|^#KEYFILE_PATTERN=|KEYFILE_PATTERN='/etc/crypto_keyfile.bin'|' /etc/cryptsetup-initramfs/conf-hook")
        os.system("echo UMASK=0077 >> /etc/initramfs-tools/initramfs.conf")
        os.system(f"echo 'luks_root '{args[1]}' /etc/crypto_keyfile.bin luks' >> /etc/crypttab")
        os.system(f"update-initramfs -u") # REVIEW: What about kernel variants?

def strap():
    excl = sp.check_output("dpkg-query -f '${binary:Package} ${Priority}\n' -W | grep -v 'required\\|important' | awk '{print $1}'", shell=True).decode('utf-8').strip().replace("\n",",")
    sp.check_call(f"debootstrap --arch {ARCH} --exclude={excl} {RELEASE} /mnt http://ftp.debian.org/debian", shell=True) # REVIEW --include={packages} ? --variant=minbase ?

main()

