#!/usr/bin/python3

import os
import subprocess
import sys ### REMOVE WHEN TRY EXCEPT ELSE IS IMPLEMENTED

def clear():
    os.system("#clear")

def to_uuid(part):
    return subprocess.check_output(f"sudo blkid -s UUID -o value {part}", shell=True).decode('utf-8').strip()

#   This function returns a tuple: (1. choice whether partitioning and formatting should happen
#   2. Underscore plus name of distro if it should be appended to sub-volume names
def get_multiboot(dist):
    clear()
    while True:
        print("Please choose one of the following:\n1. Single OS installation\n2. Initiate a multi-boot ashos setup\n3. Adding to an already installed ashos")
        print("Please be aware choosing option 1 and 2 will wipe root partition.")
        i = input("> ")
        if i == "1":
            return i, ""
            break
        elif i == "2":
            return i, f"_{dist}"
            break
        elif i == "3":
            return i, f"_{dist}"
            break
        else:
            print("Invalid choice!")
            continue

def get_hostname():
    clear()
    while True:
        print("Enter hostname:")
        h = input("> ")
        if h:
            print("Happy with your hostname? (y/n)")
            reply = input("> ")
            if reply.casefold() == "y":
                break
            else:
                continue
    return h

def get_timezone():
    clear()
    while True:
        print("Select a timezone (type list to list):")
        z = input("> ")
        if z == "list":
            os.system("ls /usr/share/zoneinfo | less")
        elif os.path.isfile(f"/usr/share/zoneinfo/{z}"):
            return str(f"/usr/share/zoneinfo/{z}")
        else:
            print("Invalid timezone!")
            continue

def get_username():
    clear()
    while True:
        print("Enter username (all lowercase):")
        u = input("> ")
        if u:
            print("Happy with your username? (y/n)")
            reply = input("> ")
            if reply.casefold() == "y":
                break
            else:
                continue
    return u

def get_luks():
    clear()
    while True:
        print("Would you like to use LUKS? (y/n)")
        reply = input("> ")
        if reply.casefold() == "y":
            e = True
            break
        elif reply.casefold() == "n":
            e = False
            break
        else:
            continue
    return e

def create_user(u, g):
    os.system(f"sudo chroot /mnt sudo /usr/sbin/adduser -h /home/{u} -G {g} -s /bin/bash {u}")
    os.system(f"echo '%{g} ALL=(ALL:ALL) ALL' | sudo tee -a /mnt/etc/sudoers")
    os.system(f"echo 'export XDG_RUNTIME_DIR=\"/run/user/1000\"' | sudo tee -a /mnt/home/{u}/.bashrc")

###def create_user(u, g):
###    os.system(f"sudo chroot /mnt sudo useradd -m -G {g} -s /bin/bash {u}")
###    os.system(f"echo '%{g} ALL=(ALL:ALL) ALL' | sudo tee -a /mnt/etc/sudoers")
###    os.system(f"echo 'export XDG_RUNTIME_DIR=\"/run/user/1000\"' | sudo tee -a /mnt/home/{u}/.bashrc")

def set_password(u, s):
    clear()
    while True:
        print(f"Setting a password for '{u}':")
        os.system(f"sudo chroot /mnt {s} passwd {u}")
        print("Was your password set properly? (y/n)")
        reply = input("> ")
        if reply.casefold() == "y":
            break
        else:
            continue

def main(args, distro):
    print("Welcome to the AshOS installer!\n\n\n\n\n")

#   Define variables
    ARCH = "x86_64"
    RELEASE = "edge"
    APK = "2.12.9-r5"
    KERNEL = "lts" ### edge
###    packages = f"alpine-base linux-lts tzdata sudo python3 py3-anytree bash \
###                btrfs-progs networkmanager tmux" #linux-firmware nano doas os-prober ###linux-{KERNEL}
    packages = f"linux-{KERNEL} tzdata sudo python3 py3-anytree bash \
                btrfs-progs tmux" #linux-firmware nano doas os-prober ###linux-{KERNEL}
    #packages = "base linux linux-firmware nano python3 python-anytree bash dhcpcd \
    #            arch-install-scripts btrfs-progs networkmanager grub sudo tmux os-prober"
    choice, distro_suffix = get_multiboot(distro)
    btrdirs = [f"@{distro_suffix}", f"@.snapshots{distro_suffix}", f"@boot{distro_suffix}", f"@etc{distro_suffix}", f"@home{distro_suffix}", f"@var{distro_suffix}"]
    mntdirs = ["", ".snapshots", "boot", "etc", "home", "var"]
    isLUKS = get_luks()
    tz = get_timezone()
#    hostname = get_hostname()
    hostname = subprocess.check_output("git rev-parse --short HEAD", shell=True).decode('utf-8').strip() # Just for debugging
    if os.path.exists("/sys/firmware/efi"):
        efi = True
    else:
        efi = False
    if isLUKS:
        btrfs_root = "/dev/mapper/luks_root"
        if efi:
            luks_grub_args = "luks2 btrfs part_gpt cryptodisk pbkdf2 gcry_rijndael gcry_sha512"
        else:
            luks_grub_args = "luks2 btrfs biosdisk part_msdos cryptodisk pbkdf2 gcry_rijndael gcry_sha512"
    else:
        btrfs_root = args[1]
        luks_grub_args = ""

### STEP 2 startS

#   Update fstab
    os.system(f"echo 'UUID=\"{to_uuid(btrfs_root)}\" / btrfs subvol=@{distro_suffix},compress=zstd,noatime,ro 0 0' | sudo tee -a /mnt/etc/fstab")
    for mntdir in mntdirs:
        os.system(f"echo 'UUID=\"{to_uuid(btrfs_root)}\" /{mntdir} btrfs subvol=@{mntdir}{distro_suffix},compress=zstd,noatime 0 0' | sudo tee -a /mnt/etc/fstab")
    if efi:
        os.system(f"echo 'UUID=\"{to_uuid(args[3])}\" /boot/efi vfat umask=0077 0 2' | sudo tee -a /mnt/etc/fstab")
    os.system("echo '/.snapshots/ash/root /root none bind 0 0' | sudo tee -a /mnt/etc/fstab")
    os.system("echo '/.snapshots/ash/tmp /tmp none bind 0 0' | sudo tee -a /mnt/etc/fstab")
    os.system(f"sudo sed -i '0,/@{distro_suffix}/ s|@{distro_suffix}|@.snapshots{distro_suffix}/rootfs/snapshot-deploy|' /mnt/etc/fstab")
    os.system(f"sudo sed -i '0,/@boot{distro_suffix}/ s|@boot{distro_suffix}|@.snapshots{distro_suffix}/boot/boot-deploy|' /mnt/etc/fstab")
    os.system(f"sudo sed -i '0,/@etc{distro_suffix}/ s|@etc{distro_suffix}|@.snapshots{distro_suffix}/etc/etc-deploy|' /mnt/etc/fstab")
    os.system(f"sudo sed -i '/\@{distro_suffix}/d' /mnt/etc/fstab") # Delete @_distro entry

#   Database and config files
    os.system("sudo mkdir -p /mnt/usr/share/ash/db")
    os.system("echo '0' | sudo tee /mnt/usr/share/ash/snap")
    os.system("sudo cp -r /mnt/var/lib/apk/. /mnt/usr/share/ash/db")
    #os.system("sed -i 's|[#?]DBPath.*$|DBPath       = /usr/share/ash/db/|g' /mnt/etc/pacman.conf")
    os.system(f"sudo sed -i '/^ID/ s|{distro}|{distro}_ashos|' /mnt/etc/os-release") # Modify OS release information (optional)

#   Update hostname, hosts, locales and timezone, hosts
    os.system(f"echo {hostname} | sudo tee /mnt/etc/hostname")
    os.system(f"echo 127.0.0.1 {hostname} | sudo tee -a /mnt/etc/hosts")
    #os.system("sudo sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/locale.gen")
    #os.system("sudo chroot /mnt sudo locale-gen")
    #os.system("echo 'LANG=en_US.UTF-8' | sudo tee /mnt/etc/locale.conf")
    os.system(f"sudo ln -srf /mnt{tz} /mnt/etc/localtime")
    os.system("sudo chroot /mnt /sbin/hwclock --systohc")

#   Copy and symlink ashpk and detect_os.sh
    os.system("sudo mkdir -p /mnt/.snapshots/ash/snapshots")
    os.system(f"echo '{to_uuid(btrfs_root)}' | sudo tee /mnt/.snapshots/ash/part")
    os.system(f"sudo cp -a ./src/distros/{distro}/ashpk_core.py /mnt/.snapshots/ash/ash")
    #os.system(f"sudo cat ./src/distros/ashpk_core.py ./src/distros/{distro}/ashpk.py > /mnt/.snapshots/ash/ash") ### SOON
    os.system("sudo cp -a ./src/detect_os.sh /mnt/.snapshots/ash/detect_os.sh")
    os.system("sudo ln -srf /mnt/.snapshots/ash/ash /mnt/usr/bin/ash")
    os.system("sudo ln -srf /mnt/.snapshots/ash/detect_os.sh /mnt/usr/bin/detect_os.sh")
    os.system("sudo ln -srf /mnt/.snapshots/ash /mnt/var/lib/ash")
    os.system("echo {\\'name\\': \\'root\\', \\'children\\': [{\\'name\\': \\'0\\'}]} | sudo tee /mnt/.snapshots/ash/fstree") # Initialize fstree


### STEP 2 ENDS

args = list(sys.argv)
distro="alpine"
main(args, distro)
