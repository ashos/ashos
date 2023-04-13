#!/usr/bin/python3

import os
from shutil import copy
import subprocess
import sys ### REMOVE WHEN TRY EXCEPT ELSE IS IMPLEMENTED
from src.installer_core import * # NOQA
#from src.installer_core import is_luks, ash_chroot, clear, deploy_base_snapshot, deploy_to_common, grub_ash, is_efi, post_bootstrap, pre_bootstrap, unmounts
from setup import args, distro

#   1. Define variables
APK = "2.12.11-r0" # https://git.alpinelinux.org/aports/plain/main/apk-tools/APKBUILD
ARCH = "x86_64"
RELEASE = "edge"
KERNEL = "edge" ### lts
packages = f"linux-{KERNEL} curl coreutils sudo tzdata mount mkinitfs umount tmux python3 py3-anytree bash"
            #linux-firmware-none networkmanager linux-firmware nano doas os-prober musl-locales musl-locales-lang dbus #### default mount from busybox gives errors. Do I also need umount?!
if is_efi:
    packages += " efibootmgr"
    packages_no_trigger = "grub-efi" # https://gitlab.alpinelinux.org/alpine/aports/-/issues/11673
#    if is_mutable: ### TODO still errors
#        packages += " dosfstools" # Optional for fsck.vfat checks at boot up
else:
    packages_no_trigger = "grub-bios"
if is_format_btrfs:
    packages += " btrfs-progs"
if is_luks:
    packages += " cryptsetup" ### REVIEW_LATER
super_group = "wheel"
v = "" # GRUB version number in /boot/grubN
URL = f"https://dl-cdn.alpinelinux.org/alpine/{RELEASE}/main"

def initram_update():
    if is_luks:
        os.system("sudo dd bs=512 count=4 if=/dev/random of=/mnt/etc/crypto_keyfile.bin iflag=fullblock")
        os.system("sudo chmod 000 /mnt/etc/crypto_keyfile.bin") # Changed from 600 as even root doesn't need access
        os.system(f"sudo cryptsetup luksAddKey {args[1]} /mnt/etc/crypto_keyfile.bin")
        os.system("sudo sed -i -e '/^HOOKS/ s/filesystems/encrypt filesystems/' \
                        -e 's|^FILES=(|FILES=(/etc/crypto_keyfile.bin|' /mnt/etc/mkinitcpio.conf") ### IMPORTANT TODO
    if is_format_btrfs: ### REVIEW TEMPORARY
        os.system("sudo sed -i 's|ext4|ext4 btrfs|' /mnt/etc/mkinitfs/mkinitfs.conf") ### TODO if array not empty, needs to be "btrfs "
    if is_luks or is_format_btrfs: ### REVIEW: does mkinitcpio need to be run without these conditions too?
        try: # work with default kernel modules first
            subprocess.check_output("sudo chroot /mnt sudo mkinitfs -b / -f /etc/fstab", shell=True) ### REVIEW <kernelvers>
        except subprocess.CalledProcessError: # and if errors
            kv = os.listdir('/mnt/lib/modules')
            try:
                if len(kv) == 1:
                    subprocess.check_output(f"sudo chroot /mnt sudo mkinitfs -b / -f /etc/fstab -k {''.join(kv)}", shell=True)
            except:
                print(f"F: Creating initfs with either live default or {kv} kernels failed!")
                print("Next, type just folder name from /mnt/lib/modules i.e. 5.15.104-0-lts")
                while True:
                    try:
                        kv = get_item_from_path("kernel version", "/mnt/lib/modules")
                        subprocess.check_output(f"sudo chroot /mnt sudo mkinitfs -b / -f /etc/fstab -k {kv}", shell=True)
                        break # Success
                    except subprocess.CalledProcessError:
                        print(f"F: Creating initfs with kernel {kv} failed!")
                        continue

#   Pre bootstrap
pre_bootstrap()

#   2. Bootstrap and install packages in chroot
os.system(f"curl -LO {URL}/{ARCH}/apk-tools-static-{APK}.apk")
os.system("tar zxf apk-tools-static-*.apk")
excode1 = os.system(f"sudo ./sbin/apk.static --arch {ARCH} -X {URL} -U --allow-untrusted --root /mnt --initdb --no-cache add alpine-base") ### REVIEW Is "/" needed after {URL} ?
copy("./src/distros/alpine/repositories", "/mnt/etc/apk/") ### REVIEW MOVED from down at section 3 to here as installing 'bash' was giving error
os.system("sudo cp --dereference /etc/resolv.conf /mnt/etc/") # --remove-destination ### not writing through dangling symlink! (TODO: try except)

while True:
    try:
        subprocess.check_output(f"chroot /mnt /bin/sh -c '/sbin/apk update && /sbin/apk add {packages}'", shell=True)
        subprocess.check_output(f"chroot /mnt /bin/sh -c '/sbin/apk update && /sbin/apk add --no-scripts {packages_no_trigger}'", shell=True)
    except subprocess.CalledProcessError:
        print("F: Bootstrap failed!")
        if yes_no("Would you like to try again?"):
            continue
        else:
            break
    else:
        print("Bootstrap finished!")

#   Mount-points for chrooting
ash_chroot()

#   3. Package manager database and config files
#os.system("sudo cp -r /mnt/var/lib/apk/. /mnt/usr/share/ash/db") ### REVIEW seems always empty?
# /var/cache/apk/ , /var/lib/apk/ , /etc/apk/cache/
os.system("sudo mv /mnt/lib/apk /mnt/usr/share/ash/db/")
os.system("sudo ln -srf /mnt/usr/share/ash/db/apk /mnt/lib/apk")

#   4. Update hostname, hosts, locales and timezone, hosts
os.system(f"echo {hostname} | sudo tee /mnt/etc/hostname")
os.system(f"echo 127.0.0.1 {hostname} {distro} | sudo tee -a /mnt/etc/hosts")
#os.system("sudo sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/locale.gen")
#os.system("sudo chroot /mnt sudo locale-gen")
#os.system("echo 'LANG=en_US.UTF-8' | sudo tee /mnt/etc/locale.conf")
os.system(f"sudo ln -srf /mnt/usr/share/zoneinfo/{tz} /mnt/etc/localtime")
os.system("sudo chroot /mnt /sbin/hwclock --systohc")

#   Post bootstrap
post_bootstrap(super_group)
if yes_no("Replace Busybox's ash with Ash? (NOT recommended yet!)"):
    os.system(f"sudo mv /mnt/bin/ash /mnt/bin/busyash")
    #os.system(f"sudo mv /mnt/bin/ash /mnt/usr/bin/ash")
    #os.system(f"sudo mv /mnt/usr/bin/ash /mnt/bin/ash")
    print("Ash replaced Busybox's ash (which is now busyash)!")
else:
    os.system(f"sudo mv /mnt/usr/bin/ash /mnt/usr/bin/asd")
    print("Use asd instead of ash!")

#   5. Services (init, network, etc.)
os.system("sudo chroot /mnt /bin/bash -c '/sbin/setup-interfaces'")
os.system(f"sudo chroot /mnt /bin/bash -c '/usr/sbin/adduser {username} plugdev'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add devfs sysinit'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add dmesg sysinit'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add mdev sysinit'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add hwdrivers sysinit'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add cgroups sysinit'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add hwclock boot'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add modules boot'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add sysctl boot'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add hostname boot'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add bootmisc boot'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add syslog boot'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add swap boot'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add networking boot'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add seedrng boot'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add mount-ro shutdown'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add killprocs shutdown'")
os.system("sudo chroot /mnt /bin/bash -c 'sudo /sbin/rc-update add savecache shutdown'")
#os.system("sudo chroot /mnt /bin/bash -c '/sbin/rc-service networkmanager start'")

#   6. Boot and EFI
os.system('echo GRUB_CMDLINE_LINUX_DEFAULT=\\"modules=sd-mod,usb-storage,btrfs quiet rootfstype=btrfs\\" | sudo tee -a /mnt/etc/default/grub') # should be before initram create otherwise canonical error in grub-probe
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

