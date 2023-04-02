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
#        os.system(f"sudo chroot /mnt sudo mkinitcpio -p linux{KERNEL}") ### TODO

#   1. Define variables
ARCH = "x86_64"
RELEASE = "rawhide"
packages = "kernel dnf passwd sudo btrfs-progs python-anytree sqlite-tools linux-firmware \
            glibc-langpack-en glibc-locale-source dhcpcd NetworkManager"
if is_efi:
    packages += " efibootmgr"
    if "64" in ARCH: ### Not good for AARCH64/ARM64
        packages += " shim-x64 grub2-efi-x64-modules"
super_group = "wheel"
v = "2" # GRUB version number in /boot/grubN
tz = get_timezone()
hostname = get_hostname()
#hostname = subprocess.check_output("git rev-parse --short HEAD", shell=True).decode('utf-8').strip() # Just for debugging

#   Pre bootstrap
pre_bootstrap()

# Mount-points for chrooting
ash_chroot()

#   2. Bootstrap and install packages in chroot
excode = os.system(f"sudo dnf -c ./src/distros/fedora/base.repo --installroot=/mnt install -y {packages} --releasever={RELEASE} --forcearch={ARCH}")
if excode != 0:
    sys.exit("Failed to bootstrap!")

#   3. Package manager database and config files
os.system('echo "kernel.printk=4" | sudo tee -a /mnt/etc/sysctl.d/10-kernel-printk.conf') ### https://github.com/coreos/fedora-coreos-tracker/issues/220
# If rpmdb is under /usr, move it to /var and create a symlink
if os.path.islink("/var/lib/dnf") or os.path.isfile("/usr/lib/sysimage/dnf/history.sqlite"):
    os.system("sudo rm -r /var/lib/dnf")
    os.system("sudo mv /usr/lib/sysimage/dnf /var/lib/")
    os.system("sudo ln -s /usr/lib/sysimage/dnf /var/lib/dnf")
if os.path.islink("/var/lib/rpm") or os.path.isfile("/usr/lib/sysimage/rpm/rpmdb.sqlite"):
    os.system("sudo rm -r /var/lib/rpm")
    os.system("sudo mv /usr/lib/sysimage/rpm /var/lib/")
    os.system("sudo ln -s /usr/lib/sysimage/rpm /var/lib/rpm")
os.system("sudo cp -a /mnt/var/lib/dnf /mnt/usr/share/ash/db/") ### mv and symlink if this is not working
os.system("sudo cp -a /mnt/var/lib/rpm /mnt/usr/share/ash/db/")
os.system('echo persistdir="/usr/share/ash/db/dnf" | sudo tee -a /mnt/etc/dnf/dnf.conf') ### REVIEW I'm not sure if this works?!
os.system(f"echo 'releasever={RELEASE}' | tee /mnt/etc/yum.conf") ### REVIEW Needed?

#   4. Update hostname, hosts, locales and timezone, hosts
os.system(f"echo {hostname} | sudo tee /mnt/etc/hostname")
os.system(f"echo 127.0.0.1 {hostname} {distro} | sudo tee -a /mnt/etc/hosts")
os.system("sudo chroot /mnt sudo localedef -v -c -i en_US -f UTF-8 en_US.UTF-8")
#os.system("sudo sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/locale.gen")
#os.system("sudo chroot /mnt sudo locale-gen")
os.system("echo 'LANG=en_US.UTF-8' | sudo tee /mnt/etc/locale.conf")
os.system(f"sudo ln -srf /mnt{tz} /mnt/etc/localtime")
os.system("sudo chroot /mnt sudo hwclock --systohc")

#   Post bootstrap
post_bootstrap(super_group)

#   5. Services (init, network, etc.)
os.system("sudo chroot /mnt systemctl enable NetworkManager")
os.system("sudo chroot /mnt systemctl disable rpmdb-migrate") # https://fedoraproject.org/wiki/Changes/RelocateRPMToUsr

#   6. Boot and EFI
initram_update_luks()
#	For now I use non-BLS format. Entries go in /boot/grub2/grub.cfg not in /boot/loader/entries/)
os.system('grep -qxF GRUB_ENABLE_BLSCFG="false" /mnt/etc/default/grub || \
           echo GRUB_ENABLE_BLSCFG="false" | sudo tee -a /mnt/etc/default/grub')
if is_efi: # This needs to go before grub_ash otherwise map.txt entry would be empty
    os.system(f"efibootmgr -c -d {args[2]} -p 1 -L 'Fedora' -l '\\EFI\\fedora\\shim.efi'")
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

