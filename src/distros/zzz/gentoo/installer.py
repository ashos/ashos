#!/usr/bin/python3

#https://wiki.gentoo.org/wiki/Handbook:AMD64/Installation/Base#Mounting_the_necessary_filesystems

import os
import subprocess
import sys ### REMOVE WHEN USING TRY CATCH

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
        print("Please be aware choosing option 1 and 2 will wipe root partition")
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
        print("Enter username (all lowercase, max 8 letters)")
        u = input("> ")
        if u:
            print("Happy with your username? (y/n)")
            reply = input("> ")
            if reply.casefold() == "y":
                break
            else:
                continue
    return u

def create_user(u, g):
    os.system(f"sudo chroot /mnt sudo useradd -m -G {g} -s /bin/bash {u}")
    os.system(f"echo '%{g} ALL=(ALL:ALL) ALL' | sudo tee -a /mnt/etc/sudoers")
    os.system(f"echo 'export XDG_RUNTIME_DIR=\"/run/user/1000\"' | sudo tee -a /mnt/home/{u}/.bashrc")

def set_password(u):
    clear()
    while True:
        print(f"Setting a password for '{u}':")
        os.system(f"sudo chroot /mnt passwd {u}")
        print("Was your password set properly (y/n)?")
        reply = input("> ")
        if reply.casefold() == "y":
            break
        else:
            continue

def main(args, distro):
    print("Welcome to the AshOS installer!\n\n\n\n\n")
    choice, distro_suffix = get_multiboot(distro)

#   Define variables
    packages = "base linux linux-firmware nano python3 python-anytree bash dhcpcd arch-install-scripts btrfs-progs networkmanager grub sudo tmux os-prober"
    ARCH = "amd64"
    astpart = to_uuid(args[1])
    btrdirs = [f"@{distro_suffix}", f"@.snapshots{distro_suffix}", f"@boot{distro_suffix}", f"@etc{distro_suffix}", f"@home{distro_suffix}", f"@var{distro_suffix}"]
    mntdirs = ["", ".snapshots", "boot", "etc", "home", "var"]
    tz = get_timezone()
#    hostname = get_hostname()
    hostname = subprocess.check_output(f"git rev-parse --short HEAD", shell=True).decode('utf-8').strip() # Just for debugging
    if os.path.exists("/sys/firmware/efi"):
        efi = True
    else:
        efi = False

#   Prep (format partition, etc.)
    if choice != "3":
        os.system(f"sudo mkfs.btrfs -L LINUX -f {args[1]}")
    os.system("pacman -Syy --noconfirm archlinux-keyring")

#   Mount and create necessary sub-volumes and directories
    if choice != "3":
        os.system(f"sudo mount -t btrfs {args[1]} /mnt")
    else:
        os.system(f"sudo mount -o subvolid=5 {args[1]} /mnt")
    for btrdir in btrdirs:
        os.system(f"sudo btrfs sub create /mnt/{btrdir}")
    os.system("sudo umount /mnt")
    for mntdir in mntdirs:
        os.system(f"sudo mkdir -p /mnt/{mntdir}") # -p to ignore /mnt exists complaint
        os.system(f"sudo mount {args[1]} -o subvol={btrdirs[mntdirs.index(mntdir)]},compress=zstd,noatime /mnt/{mntdir}")
    for i in ("tmp", "root"):
        os.system(f"mkdir -p /mnt/{i}")
    for i in ("ash", "boot", "etc", "root", "rootfs", "tmp"):
        os.system(f"mkdir -p /mnt/.snapshots/{i}")
    if efi:
        os.system("sudo mkdir -p /mnt/boot/efi")
        os.system(f"sudo mount {args[3]} /mnt/boot/efi")

#   Bootstrap then install anytree and necessary packages in chroot
    tmp1 = f"http://distfiles.gentoo.org/releases/{ARCH}/autobuilds/"
    tmp2 = subprocess.check_output(f'curl -L {tmp1}latest-stage3.txt | grep -i systemd | grep -Ev "multi|desktop" \
                                     | awk '"'{print $1}'"'', shell=True).decode('utf-8').strip()
    os.system(f"curl --output-dir /mnt -L -O {tmp1}{tmp2}")
    os.system(f"tar --numeric-owner --xattrs -xvJpf stage3-*.tar.xz -C /mnt") # add sudo
    os.system(f"sudo rm -v -f /mnt/stage3-{ARCH}-*")
    
    ### STEP 1 ENDS HERE
    
#    if efi:
#        excode = int(os.system("sudo pacstrap /mnt efibootmgr"))
#        if excode != 0:
#            print("Failed to download packages!")
#            sys.exit()

    os.system("sudo mount -o x-mount.mkdir --rbind --make-rslave /dev /mnt/dev")
    os.system("sudo mount -o x-mount.mkdir --types proc /proc /mnt/proc")
    os.system("sudo mount -o x-mount.mkdir --bind --make-slave /run /mnt/run")
    os.system("sudo mount -o x-mount.mkdir --rbind --make-rslave /sys /mnt/sys")
    if efi: # Bad idea to combine --make-[r]slave and --[r]bind ? https://unix.stackexchange.com/questions/120827/recursive-umount-after-rbind-mount
        os.system("sudo mount -o x-mount.mkdir,remount,rw --types efivarfs efivarfs /mnt/sys/firmware/efi/efivars")
    os.system("sudo cp --dereference /etc/resolv.conf /mnt/etc/")

    ### STEP 2 ENDS HERE

#   Chroot into it
    os.system("sudo mkdir -p /mnt/var/db/repos/gentoo")
    os.system("sudo chroot /mnt /bin/env -i TERM=$TERM /bin/bash")
    #source /etc/profile
    #env-update
    #export PS1="(chroot) $PS1"
    
    ### emerge-webrsync -x -v           ### Needed?

#   Update fstab part 1
    os.system(f"echo 'UUID=\"{to_uuid(args[1])}\" / btrfs subvol=@{distro_suffix},compress=zstd,noatime,ro 0 0' | sudo tee -a /mnt/etc/fstab")
    for mntdir in mntdirs:
        os.system(f"echo 'UUID=\"{to_uuid(args[1])}\" /{mntdir} btrfs subvol=@{mntdir}{distro_suffix},compress=zstd,noatime 0 0' | sudo tee -a /mnt/etc/fstab")
    if efi:
        os.system(f"echo 'UUID=\"{to_uuid(args[3])}\" /boot/efi vfat umask=0077 0 2' | sudo tee -a /mnt/etc/fstab")
    os.system("echo '/.snapshots/ash/root /root none bind 0 0' | sudo tee -a /mnt/etc/fstab")
    os.system("echo '/.snapshots/ash/tmp /tmp none bind 0 0' | sudo tee -a /mnt/etc/fstab")
    os.system(f"sudo sed -i '0,/@{distro_suffix}/ s|@{distro_suffix}|@.snapshots{distro_suffix}/rootfs/snapshot-tmp|' /mnt/etc/fstab")
    os.system(f"sudo sed -i '0,/@boot{distro_suffix}/ s|@boot{distro_suffix}|@.snapshots{distro_suffix}/boot/boot-tmp|' /mnt/etc/fstab")
    os.system(f"sudo sed -i '0,/@etc{distro_suffix}/ s|@etc{distro_suffix}|@.snapshots{distro_suffix}/etc/etc-tmp|' /mnt/etc/fstab")
    os.system(f"sudo sed -i '/\@{distro_suffix}/d' /mnt/etc/fstab") # Delete @_distro entry

    ### STEP 3 ENDS HERE

    ### CONTINUE debootstrapping (will put it up above eventually)
    os.system("sudo chroot /mnt emerge sys-boot/efibootmgr") # Removed '--ask'

#   Database and config files
    os.system("sudo mkdir -p /mnt/usr/share/ash/db")
    os.system("echo '0' | sudo tee /mnt/usr/share/ash/snap")
    #os.system("sudo cp -r /mnt/var/lib/portage/. /mnt/usr/share/ash/db")
    os.system("sudo cp -r /mnt/var/db/repos/gentoo/. /mnt/usr/share/ash/db")
    #   os.system("sudo mkdir --parents /mnt/etc/portage/repos.conf")
    #os.system("sudo cp /mnt/usr/share/portage/config/repos.conf /mnt/etc/portage/repos.conf/gentoo.conf")
    os.system(f"sed -i 's|[#?]DBPath.*$|DBPath       = /usr/share/ash/db/|g' /mnt/usr/share/portage/config/repos.conf")
    os.system(f"sudo sed -i '/^ID/ s|{distro}|{distro}_ashos|' /mnt/etc/os-release") # Modify OS release information (optional)

#   Update hostname, hosts, locales and timezone, hosts
    os.system(f"echo {hostname} | sudo tee /mnt/etc/hostname")
    os.system(f"echo 127.0.0.1 {hostname} | sudo tee -a /mnt/etc/hosts")
    os.system("sudo sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/locale.gen")
    os.system("sudo chroot /mnt sudo locale-gen")
    #os.system("echo 'LANG=en_US.UTF-8' | sudo tee /mnt/etc/locale.conf") ### REVIEW It already has C.UTF-8 should I add en_US.UTF-8 ?
    os.system(f"sudo ln -srf /mnt{tz} /mnt/etc/localtime")
    os.system("sudo chroot /mnt /sbin/hwclock --systohc")

#   Copy and symlink ashpk and detect_os.sh
    os.system("sudo mkdir -p /mnt/.snapshots/ash/snapshots")
    os.system(f"echo '{to_uuid(args[1])}' | sudo tee /mnt/.snapshots/ash/part")
    os.system(f"sudo cp -a ./src/distros/{distro}/ashpk_core.py /mnt/.snapshots/ash/ash")
    os.system("sudo cp -a ./src/detect_os.sh /mnt/.snapshots/ash/detect_os.sh")
    os.system("sudo ln -srf /mnt/.snapshots/ash/ash /mnt/usr/bin/ash")
    os.system("sudo ln -srf /mnt/.snapshots/ash/detect_os.sh /mnt/usr/bin/detect_os.sh")
    os.system("sudo ln -srf /mnt/.snapshots/ash /mnt/var/lib/ash")
    os.system("echo {\\'name\\': \\'root\\', \\'children\\': [{\\'name\\': \\'0\\'}]} | sudo tee /mnt/.snapshots/ash/fstree") # Initialize fstree

#   Create user and set password
    set_password("root")
    username = get_username()
    create_user(username, "wheel")
    set_password(username)

#   Services (init, network, etc.)
    os.system("sudo chroot /mnt systemctl enable NetworkManager")

#   GRUB and EFI
    os.system(f'sudo chroot /mnt sudo grub-install {args[2]}')
    os.system(f"sudo chroot /mnt sudo grub-mkconfig {args[2]} -o /boot/grub/grub.cfg")
    os.system("sudo mkdir -p /mnt/boot/grub/BAK") # Folder for backing up grub configs created by ashpk
###    os.system(f"sudo sed -i '0,/subvol=@{distro_suffix}/ s,subvol=@{distro_suffix},subvol=@.snapshots{distro_suffix}/rootfs/snapshot-tmp,g' /mnt/boot/grub/grub.cfg") ### This was not replacing mount points in Advanced section
    os.system(f"sudo sed -i 's|subvol=@{distro_suffix}|subvol=@.snapshots{distro_suffix}/rootfs/snapshot-tmp|g' /mnt/boot/grub/grub.cfg")
    # Create a mapping of "distro" <=> "BootOrder number". Ash reads from this file to switch between distros.
    if efi:
        if not os.path.exists("/mnt/boot/efi/EFI/map.txt"):
            os.system("echo DISTRO,BootOrder | sudo tee /mnt/boot/efi/EFI/map.txt")
        os.system(f"echo '{distro},' $(efibootmgr -v | grep -i {distro} | awk '"'{print $1}'"' | sed '"'s|[^0-9]*||g'"') | sudo tee -a /mnt/boot/efi/EFI/map.txt")

#   BTRFS snapshots
    os.system("sudo btrfs sub snap -r /mnt /mnt/.snapshots/rootfs/snapshot-0")
    os.system("sudo btrfs sub create /mnt/.snapshots/boot/boot-tmp")
    os.system("sudo btrfs sub create /mnt/.snapshots/etc/etc-tmp")
    os.system("sudo cp -r --reflink=auto /mnt/boot/. /mnt/.snapshots/boot/boot-tmp")
    os.system("sudo cp -r --reflink=auto /mnt/etc/. /mnt/.snapshots/etc/etc-tmp")
    os.system("sudo btrfs sub snap -r /mnt/.snapshots/boot/boot-tmp /mnt/.snapshots/boot/boot-0")
    os.system("sudo btrfs sub snap -r /mnt/.snapshots/etc/etc-tmp /mnt/.snapshots/etc/etc-0")
    os.system("sudo btrfs sub snap /mnt/.snapshots/rootfs/snapshot-0 /mnt/.snapshots/rootfs/snapshot-tmp")
    os.system("sudo chroot /mnt sudo btrfs sub set-default /.snapshots/rootfs/snapshot-tmp")
    os.system("sudo cp -r /mnt/root/. /mnt/.snapshots/root/")
    os.system("sudo cp -r /mnt/tmp/. /mnt/.snapshots/tmp/")
    os.system("sudo rm -rf /mnt/root/*")
    os.system("sudo rm -rf /mnt/tmp/*")

#   Copy boot and etc from snapshot's tmp to common
    if efi:
        os.system("sudo umount /mnt/boot/efi")
    os.system("sudo umount /mnt/boot")
    os.system(f"sudo mount {args[1]} -o subvol=@boot{distro_suffix},compress=zstd,noatime /mnt/.snapshots/boot/boot-tmp")
    os.system("sudo cp --reflink=auto -r /mnt/.snapshots/boot/boot-tmp/. /mnt/boot/")
    os.system("sudo umount /mnt/etc")
    os.system(f"sudo mount {args[1]} -o subvol=@etc{distro_suffix},compress=zstd,noatime /mnt/.snapshots/etc/etc-tmp")
    os.system("sudo cp --reflink=auto -r /mnt/.snapshots/etc/etc-tmp/. /mnt/etc/")
    os.system("sudo cp --reflink=auto -r /mnt/.snapshots/boot/boot-0/. /mnt/.snapshots/rootfs/snapshot-tmp/boot/")
    os.system("sudo cp --reflink=auto -r /mnt/.snapshots/etc/etc-0/. /mnt/.snapshots/rootfs/snapshot-tmp/etc/")

#   Unmount everything and finish
    os.system("sudo umount --recursive /mnt")
    os.system(f"sudo mount {args[1]} -o subvolid=0 /mnt")
    os.system(f"sudo btrfs sub del /mnt/@{distro_suffix}")
    os.system("sudo umount --recursive /mnt")
    clear()
    print("Installation complete")
    print("You can reboot now :)")

