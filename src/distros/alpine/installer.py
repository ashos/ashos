#!/usr/bin/env python3

import os
import subprocess as sp
import sys
import tarfile
from re import search
from setup import args, distro
from shutil import copy
from src.installer_core import * # NOQA
from setup import args, distro
from urllib.error import URLError, HTTPError
from urllib.request import urlopen

#   1. Define variables
ARCH = "x86_64"
RELEASE = "edge"
KERNEL = "edge" # options: lts
packages = f"linux-{KERNEL} curl coreutils sudo tzdata mount mkinitfs umount \
            tmux bash" # \
            #linux-firmware-none networkmanager linux-firmware nano doas os-prober musl-locales musl-locales-lang dbus"
            # default mount from busybox gives errors. # umount still required?!
if not is_ash_bundle:
    packages +=  " python3 py3-anytree"
if is_efi:
    packages += " efibootmgr"
    packages_no_trigger = "grub-efi" # https://gitlab.alpinelinux.org/alpine/aports/-/issues/11673
#if is_mutable: # TODO still errors
#    packages += " dosfstools" # Optional for fsck.vfat checks at boot up
else:
    packages_no_trigger = "grub-bios"
if is_format_btrfs:
    packages += " btrfs-progs"
if is_luks:
    packages += " cryptsetup" # REVIEW
super_group = "wheel"
URL = f"https://dl-cdn.alpinelinux.org/alpine/{RELEASE}/main"
v = "" # GRUB version number in /boot/grubN

def main():
    #   Pre bootstrap
    pre_bootstrap()

    #   2. Bootstrap and install packages in chroot
    while True:
        try:
            strap(packages)
        except (sp.CalledProcessError, HTTPError, URLError, FileNotFoundError, tarfile.ExtractError) as e:
            print(e)
            if not yes_no("F: Failed to strap package(s). Retry?"):
                unmounts("failed") # user declined
                sys.exit("F: Install failed!")
        else: # success
            break

    #   Mount-points for chrooting
    ashos_mounts()
    cur_dir_code = chroot_in("/mnt")

    #   3. Package manager database and config files
    #os.system(f"{SUDO} cp -r /mnt/var/lib/apk/. /mnt/usr/share/ash/db") # REVIEW seems always empty?
    # /var/cache/apk/ , /var/lib/apk/ , /etc/apk/cache/
    os.system("mv /lib/apk /usr/share/ash/db/")
    os.system("ln -sf /usr/share/ash/db/apk /lib/apk")

    #   4. Update hostname, hosts, locales and timezone, hosts
    os.system(f"echo {hostname} > /etc/hostname")
    os.system(f"echo 127.0.0.1 {hostname} {distro} >> /etc/hosts")
    #os.system(f"{SUDO} sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/locale.gen")
    #os.system(f"{SUDO} chroot /mnt {SUDO} locale-gen")
    #os.system(f"echo 'LANG=en_US.UTF-8' | {SUDO} tee /mnt/etc/locale.conf")
    os.system(f"ln -sf /usr/share/zoneinfo/{tz} /etc/localtime") # removed /mnt/XYZ from both paths (and from all lines above)
    os.system("/sbin/hwclock --systohc")

    #   Post bootstrap
    post_bootstrap(super_group)
    if yes_no("Replace Busybox's ash with Ash? (NOT recommended yet!)"): # REVIEW removed "{SUDO}" from all lines below (and all {SUDO}'s)
        os.system("mv /bin/ash /bin/busyash")
        #os.system(f"{SUDO} mv /mnt/bin/ash /mnt/usr/bin/ash")
        #os.system(f"{SUDO} mv /mnt/usr/bin/ash /mnt/bin/ash")
        print("Ash replaced Busybox's ash (which is now busyash)!")
    else:
        os.system("mv /usr/bin/ash /usr/bin/asd")
        print("Use asd instead of ash!")

    #   5. Services (init, network, etc.) # REVIEW removed {SUDO} chroot /mnt /bin/bash -c '{SUDO} from all lines
    os.system("/sbin/setup-interfaces")
    os.system(f"/usr/sbin/adduser {username} plugdev")
    os.system("/sbin/rc-update add devfs sysinit")
    os.system("/sbin/rc-update add dmesg sysinit")
    os.system("/sbin/rc-update add mdev sysinit")
    os.system("/sbin/rc-update add hwdrivers sysinit")
    os.system("/sbin/rc-update add cgroups sysinit")
    os.system("/sbin/rc-update add hwclock boot")
    os.system("/sbin/rc-update add modules boot")
    os.system("/sbin/rc-update add sysctl boot")
    os.system("/sbin/rc-update add hostname boot")
    os.system("/sbin/rc-update add bootmisc boot")
    os.system("/sbin/rc-update add syslog boot")
    os.system("/sbin/rc-update add swap boot")
    os.system("/sbin/rc-update add networking boot")
    os.system("/sbin/rc-update add seedrng boot")
    os.system("/sbin/rc-update add mount-ro shutdown")
    os.system("/sbin/rc-update add killprocs shutdown")
    os.system("/sbin/rc-update add savecache shutdown")
    #os.system(f"{SUDO} chroot /mnt /bin/bash -c '/sbin/rc-service networkmanager start'")

    #   6. Boot and EFI
    os.system('echo GRUB_CMDLINE_LINUX_DEFAULT=\\"modules=sd-mod,usb-storage,btrfs quiet rootfstype=btrfs\\" >> /etc/default/grub') # should be before initram create otherwise canonical error in grub-probe
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

def get_apk_ver():
    temp = urlopen("https://git.alpinelinux.org/aports/plain/main/apk-tools/APKBUILD").read().decode('utf-8')
    major = search('pkgver=(.+?)\n', temp).group(1)
    minor = search('pkgrel=(.+?)\n', temp).group(1)
    return f"{major}-r{minor}"

def initram_update(): # REVIEW removed "{SUDO}" from all lines below
    if is_luks:
        os.system("dd bs=512 count=4 if=/dev/random of=/etc/crypto_keyfile.bin iflag=fullblock")
        os.system("chmod 000 /etc/crypto_keyfile.bin") # Changed from 600 as even root doesn't need access
        os.system(f"cryptsetup luksAddKey {args[1]} /etc/crypto_keyfile.bin")
        os.system("sed -i -e '/^HOOKS/ s/filesystems/encrypt filesystems/' -e \
                  's|^FILES=(|FILES=(/etc/crypto_keyfile.bin|' /etc/mkinitcpio.conf") # TODO important
    if is_format_btrfs: # REVIEW temporary
        os.system("sed -i 's|ext4|ext4 btrfs|' /etc/mkinitfs/mkinitfs.conf") # TODO if array not empty, needs to be "btrfs "
    if is_luks or is_format_btrfs: # REVIEW: mkinitcpio need to be run without these conditions too?
        try: # default kernel modules first
            sp.check_call("mkinitfs -b / -f /etc/fstab", shell=True) # REVIEW <kernelvers>
        except sp.CalledProcessError: # and if errors
            kv = os.listdir('/lib/modules')
            try:
                if len(kv) == 1: # only one folder exists
                    sp.check_call(f"mkinitfs -b / -f /etc/fstab -k {''.join(kv)}", shell=True)
            except:
                print(f"F: Failed to create initfs with either live default or {kv} kernels!")
                print("Next, type just folder name from /lib/modules i.e. 5.15.104-0-lts")
                while True:
                    try:
                        kv = get_item_from_path("kernel version", "/lib/modules")
                        sp.check_call(f"mkinitfs -b / -f /etc/fstab -k {kv}", shell=True)
                        break # Success
                    except sp.CalledProcessError:
                        print(f"F: Creating initfs with kernel {kv} failed!")
                        continue

def strap(packages):
    APK = get_apk_ver() # e.g. 2.14.0_rc1-r0
    with TemporaryDirectory(dir="/tmp", prefix="ash.") as tmpdir:
        apk_file = urlopen(f"{URL}/{ARCH}/apk-tools-static-{APK}.apk").read() # curl -LO
        open(f"{tmpdir}/apk-tools-static-{APK}.apk", "wb").write(apk_file)
        tarfile.open(f"{tmpdir}/apk-tools-static-{APK}.apk").extractall(path=tmpdir) # tar zxf
        # REVIEW close() needed?
        sp.check_call(f"{SUDO} {tmpdir}/sbin/apk.static --arch {ARCH} -X {URL} -U --allow-untrusted --root /mnt --initdb --no-cache add alpine-base", shell=True) # REVIEW "/" needed after {URL} ?
    copy(f"{installer_dir}/src/distros/alpine/repositories", "/mnt/etc/apk/") # REVIEW moved here from section 3 as error in installing 'bash'
    commands = f'''
    {SUDO} cp --dereference /etc/resolv.conf /mnt/etc/
    chroot /mnt /bin/sh -c "/sbin/apk update && /sbin/apk add {packages}"
    chroot /mnt /bin/sh -c "/sbin/apk update && /sbin/apk add --no-scripts {packages_no_trigger}"
    '''
    sp.check_call(commands, shell=True)

main()

