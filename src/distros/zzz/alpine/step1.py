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
                btrfs-progs networkmanager tmux" #linux-firmware nano doas os-prober ###linux-{KERNEL}
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

#   Prep (format partition, etc.)
    if isLUKS:
        os.system("sudo modprobe dm-crypt")
        print("--- Create LUKS partition --- ")
        os.system(f"sudo cryptsetup -y -v -c aes-xts-plain64 -s 512 --hash sha512 --pbkdf pbkdf2 --type luks2 luksFormat {args[1]}")
        print("--- Open LUKS partition --- ")
        os.system(f"sudo cryptsetup --allow-discards --persistent --type luks2 open {args[1]} luks_root")
    if choice != "3":
        os.system(f"sudo mkfs.btrfs -L LINUX -f {btrfs_root}")
    os.system("pacman -Syy --noconfirm archlinux-keyring")

#   Mount and create necessary sub-volumes and directories
    if choice != "3":
        os.system(f"sudo mount -t btrfs {btrfs_root} /mnt")
    else:
        os.system(f"sudo mount -o subvolid=5 {btrfs_root} /mnt")
    for btrdir in btrdirs:
        os.system(f"sudo btrfs sub create /mnt/{btrdir}")
    os.system("sudo umount /mnt")
    for mntdir in mntdirs:
        os.system(f"sudo mkdir -p /mnt/{mntdir}") # -p to ignore /mnt exists complaint
        os.system(f"sudo mount {btrfs_root} -o subvol={btrdirs[mntdirs.index(mntdir)]},compress=zstd,noatime /mnt/{mntdir}")
    for i in ("tmp", "root"):
        os.system(f"mkdir -p /mnt/{i}")
    for i in ("ash", "boot", "etc", "root", "rootfs", "tmp"):
        os.system(f"mkdir -p /mnt/.snapshots/{i}")
    if efi:
        os.system("sudo mkdir -p /mnt/boot/efi")
        os.system(f"sudo mount {args[3]} /mnt/boot/efi")

### STEP 2 BEGINS

#   Bootstrap then install anytree and necessary packages in chroot
    os.system(f"curl -LO https://dl-cdn.alpinelinux.org/alpine/{RELEASE}/main/{ARCH}/apk-tools-static-{APK}.apk")
    os.system("tar zxf apk-tools-static-*.apk")
###    os.system("sudo cp ./src/distros/alpine/repositories /mnt/etc/apk/")
###    excode = int(os.system(f"sudo ./sbin/apk.static --arch {ARCH} -X http://dl-cdn.alpinelinux.org/alpine/{RELEASE}/main/ \
###                             -U --allow-untrusted --root /mnt --initdb add --no-cache {packages}"))
    excode = int(os.system(f"sudo ./sbin/apk.static --arch {ARCH} -X http://dl-cdn.alpinelinux.org/alpine/{RELEASE}/main/ \
                             -U --allow-untrusted --root /mnt --initdb add --no-cache alpine-base"))    
    if excode != 0:
        print("Failed to bootstrap!")
        sys.exit()
    # Mount-points needed for chrooting
    os.system("sudo mount -o x-mount.mkdir --rbind --make-rslave /dev /mnt/dev")
    os.system("sudo mount -o x-mount.mkdir --types proc /proc /mnt/proc")
    os.system("sudo mount -o x-mount.mkdir --bind --make-slave /run /mnt/run")
    os.system("sudo mount -o x-mount.mkdir --rbind --make-rslave /sys /mnt/sys")
    if efi:
        os.system("sudo mount -o x-mount.mkdir --rbind --make-rslave /sys/firmware/efi/efivars /mnt/sys/firmware/efi/efivars")
    os.system("sudo cp --dereference /etc/resolv.conf /mnt/etc/")
    os.system("sudo cp ./src/distros/alpine/repositories /mnt/etc/apk/")

### STEP 1 ENDS

args = list(sys.argv)
distro="alpine"
main(args, distro)
