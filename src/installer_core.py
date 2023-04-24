#!/usr/bin/python3

import os
import subprocess
from re import search
from setup import args, distro, distro_name
from shutil import copy, which

SUDO = "sudo" ### Test and emove if sudo not needed in any distro

# ------------------------------ CORE FUNCTIONS ------------------------------ #

#   Mount-points needed for chrooting
def ash_chroot():
    os.system(f"{SUDO} mount -o x-mount.mkdir --rbind --make-rslave /dev /mnt/dev")
    os.system(f"{SUDO} mount -o x-mount.mkdir --types proc /proc /mnt/proc")
    os.system(f"{SUDO} mount -o x-mount.mkdir --bind --make-slave /run /mnt/run")
    os.system(f"{SUDO} mount -o x-mount.mkdir --rbind --make-rslave /sys /mnt/sys")
    if is_efi:
        os.system(f"{SUDO} mount -o x-mount.mkdir --rbind --make-rslave /sys/firmware/efi/efivars /mnt/sys/firmware/efi/efivars")
    os.system(f"{SUDO} cp --dereference /etc/resolv.conf /mnt/etc/") # --remove-destination ### not writing through dangling symlink! (TODO: try except)

#   Clear screen
def clear():
    os.system("#clear")

#   Users
def create_user(u, g):
    if distro == "alpine": ### REVIEW 2023 not generic enough
        os.system(f"{SUDO} chroot /mnt {SUDO} /usr/sbin/adduser -h /home/{u} -G {g} -s /bin/sh -D {u}")
    else:
        os.system(f"{SUDO} chroot /mnt {SUDO} useradd -m -G {g} -s /bin/sh {u}")
    os.system(f"echo '%{g} ALL=(ALL:ALL) ALL' | {SUDO} tee -a /mnt/etc/sudoers")
    os.system(f"echo 'export XDG_RUNTIME_DIR=\"/run/user/1000\"' | {SUDO} tee -a /mnt/home/{u}/.$(echo $0)rc")

#   BTRFS snapshots
def deploy_base_snapshot():
    os.system(f"{SUDO} btrfs sub snap {'' if is_mutable else '-r'} /mnt /mnt/.snapshots/rootfs/snapshot-0")
    os.system(f"{SUDO} btrfs sub create /mnt/.snapshots/boot/boot-deploy")
    os.system(f"{SUDO} btrfs sub create /mnt/.snapshots/etc/etc-deploy")
    os.system(f"{SUDO} cp -r --reflink=auto /mnt/boot/. /mnt/.snapshots/boot/boot-deploy")
    #shutil.copy("/mnt/boot", "/mnt/.snapshots/boot/boot-deploy", *, follow_symlinks=True)
    os.system(f"{SUDO} cp -r --reflink=auto /mnt/etc/. /mnt/.snapshots/etc/etc-deploy")
    os.system(f"{SUDO} btrfs sub snap {'' if is_mutable else '-r'} /mnt/.snapshots/boot/boot-deploy /mnt/.snapshots/boot/boot-0")
    os.system(f"{SUDO} btrfs sub snap {'' if is_mutable else '-r'} /mnt/.snapshots/etc/etc-deploy /mnt/.snapshots/etc/etc-0")
    if is_mutable: # Mark base snapshot as mutable
            os.system("touch /mnt/.snapshots/rootfs/snapshot-0/usr/share/ash/mutable")
    os.system(f"{SUDO} btrfs sub snap /mnt/.snapshots/rootfs/snapshot-0 /mnt/.snapshots/rootfs/snapshot-deploy")
    os.system(f"{SUDO} chroot /mnt {SUDO} btrfs sub set-default /.snapshots/rootfs/snapshot-deploy")
    os.system(f"{SUDO} cp -r /mnt/root/. /mnt/.snapshots/root/")
    os.system(f"{SUDO} cp -r /mnt/tmp/. /mnt/.snapshots/tmp/")
    os.system(f"{SUDO} rm -rf /mnt/root/*")
    os.system(f"{SUDO} rm -rf /mnt/tmp/*")

#   Copy boot and etc: deployed snapshot <---> common
def deploy_to_common():
    if is_efi:
        os.system(f"{SUDO} umount /mnt/boot/efi")
    os.system(f"{SUDO} umount /mnt/boot")
    os.system(f'{SUDO} mount {bp if is_boot_external else os_root} -o {"subvol="+f"@boot{distro_suffix}"+"," if not is_boot_external else ""}compress=zstd,noatime /mnt/.snapshots/boot/boot-deploy') ### REVIEW_LATER A similar line for is_home_external needed?
###    if is_boot_external: # easier to read
###        os.system(f"{SUDO} mount {bp} -o compress=zstd,noatime /mnt/.snapshots/boot/boot-deploy")
###    else:
###        os.system(ff"{SUDO} mount {os_root} -o subvol=@boot{distro_suffix},compress=zstd,noatime /mnt/.snapshots/boot/boot-deploy")
    os.system(f"{SUDO} cp -r --reflink=auto /mnt/.snapshots/boot/boot-deploy/. /mnt/boot/")
    os.system(f"{SUDO} umount /mnt/etc")
    os.system(f"{SUDO} mount {os_root} -o subvol=@etc{distro_suffix},compress=zstd,noatime /mnt/.snapshots/etc/etc-deploy")
    os.system(f"{SUDO} cp -r --reflink=auto /mnt/.snapshots/etc/etc-deploy/. /mnt/etc/")
    os.system(f"{SUDO} cp -r --reflink=auto /mnt/.snapshots/boot/boot-0/. /mnt/.snapshots/rootfs/snapshot-deploy/boot/")
    os.system(f"{SUDO} cp -r --reflink=auto /mnt/.snapshots/etc/etc-0/. /mnt/.snapshots/rootfs/snapshot-deploy/etc/")

def get_external_partition(thing):
    clear()
    while True:
        print(f"Enter your external {thing} partition (e.g. /dev/sdaX):")
        p = input("> ")
        if p:
            if yes_no("Happy with your choice?"):
                break
            else:
                continue
    return p

def get_name(thing):
    clear()
    while True:
        print(f"Enter {thing} (all lowercase):")
        u = input("> ")
        if u:
            if yes_no(f"Happy with your {thing}?"):
                break
            else:
                continue
    return u

#   This function returns a tuple: 1. choice whether partitioning and formatting should happen
#   2. Underscore plus name of distro if it should be appended to sub-volume names
def get_multiboot(dist):
    clear()
    msg = "Initiate a new AshOS install?\n \
        Y: Wipes root partition\n \
        N: Add to an already installed AshOS (advanced multi-booting)"
    if yes_no(msg):
        return "1", f"_{dist}"
    else:
        return "2", f"_{dist}"

#   Generic function to choose something from a directory
def get_item_from_path(thing, apath):
    clear()
    while True:
        print(f"Select a {thing} (type list to list):")
        ch = input("> ")
        if ch == "list":
            ch = []
            for root, _dirs, files in os.walk(apath, followlinks=True):
                for file in files:
                    ch.append(os.path.join(root, file).replace(f"{apath}/", ""))
            ch = "\n".join(sorted(ch))
            os.system(f"echo '{ch}' | less")
        else:
            temp = str(f"{apath}/{ch}")
            if not ( os.path.isfile(temp) or os.path.isdir(temp) ):
                print(f"Invalid {thing}!")
                continue
            return ch ### REVIEW originally was just break

#   GRUB and EFI
def grub_ash(v):
    os.system(f"{SUDO} sed -i 's/^GRUB_DISTRIBUTOR.*$/GRUB_DISTRIBUTOR=\"{distro_name}\"/' /mnt/etc/default/grub")
    if is_luks:
        os.system(f"{SUDO} sed -i 's/^#GRUB_ENABLE_CRYPTODISK.*$/GRUB_ENABLE_CRYPTODISK=y/' /mnt/etc/default/grub")
        os.system(f"{SUDO} sed -i -E 's|^#?GRUB_CMDLINE_LINUX=\"|GRUB_CMDLINE_LINUX=\"cryptdevice=UUID={to_uuid(args[1])}:luks_root cryptkey=rootfs:/etc/crypto_keyfile.bin|' /mnt/etc/default/grub")
        os.system(f"sed -e 's|DISTRO|{distro}|' -e 's|LUKS_UUID_NODASH|{to_uuid(args[1]).replace('-', '')}|' \
                        -e '/^#/d' ./src/prep/grub_luks2.conf | {SUDO} tee /mnt/etc/grub_luks2.conf")
  # grub-install rewrites default core.img, so run grub-mkimage AFTER!
    if distro != "fedora": # https://bugzilla.redhat.com/show_bug.cgi?id=1917213
        if is_efi:
            #os.system(f'{SUDO} chroot /mnt {SUDO} grub{v}-install {args[2]} --modules="{luks_grub_args}"')
#            os.system(f'{SUDO} chroot /mnt {SUDO} grub{v}-install {args[2]} --bootloader-id={distro} --modules="{luks_grub_args}" --target=x86_64-efi') # --efi-directory=/boot/efi ### OLD TO_DELETE before adding separate boot partition code
            os.system(f'{SUDO} chroot /mnt {SUDO} grub{v}-install {bp if is_boot_external else args[2]} --bootloader-id={distro} --modules="{luks_grub_args}" --target=x86_64-efi') # --efi-directory=/boot/efi ### REVIEW_LATER TODO NEW
        else:
#            os.system(f'{SUDO} chroot /mnt {SUDO} grub{v}-install {args[2]} --bootloader-id={distro} --modules="{luks_grub_args}"') ### REVIEW: specifying --target for non-uefi needed? ### OLD TO_DELETE before adding separate boot partition code
            os.system(f'{SUDO} chroot /mnt {SUDO} grub{v}-install {bp if is_boot_external else args[2]} --bootloader-id={distro} --modules="{luks_grub_args}"') ### REVIEW: specifying --target for non-uefi needed? ### REVIEW_LATER TODO NEW
    if is_luks: # Make LUKS2 compatible grub image
        if is_efi:
            os.system(f'{SUDO} chroot /mnt {SUDO} grub{v}-mkimage -p "(crypto0)/@boot_{distro}/grub{v}" -O x86_64-efi -c /etc/grub_luks2.conf -o /boot/efi/EFI/{distro}/grubx64.efi {luks_grub_args}') # without '/grub' gives error normal.mod not found (maybe only one of these here and grub_luks2.conf is enough?!)
        else:
            os.system(f'{SUDO} chroot /mnt {SUDO} grub{v}-mkimage -p "(crypto0)/@boot_{distro}/grub{v}" -O i386-pc -c /etc/grub_luks2.conf -o /boot/grub{v}/i386-pc/core_luks2.img {luks_grub_args}') # without '/grub' gives error normal.mod not found (maybe only one of these here and grub_luks2.conf is enough?!) ### 'biosdisk' module not needed eh?
#            os.system(f'dd oflag=seek_bytes seek=512 if=/mnt/boot/grub{v}/i386-pc/core_luks2.img of={args[2]}') ### OLD TO_DELETE before adding separate boot partition code
            os.system(f'dd oflag=seek_bytes seek=512 if=/mnt/boot/grub{v}/i386-pc/core_luks2.img of={bp if is_boot_external else args[2]}') ### REVIEW_LATER TODO NEW
#    os.system(f"{SUDO} chroot /mnt {SUDO} grub{v}-mkconfig {args[2]} -o /boot/grub{v}/grub.cfg") ### OLD TO_DELETE before adding separate boot partition code
    os.system(f"{SUDO} chroot /mnt {SUDO} grub{v}-mkconfig {bp if is_boot_external else args[2]} -o /boot/grub{v}/grub.cfg") ### REVIEW_LATER TODO NEW
    os.system(f"{SUDO} mkdir -p /mnt/boot/grub{v}/BAK") # Folder for backing up grub configs created by ashpk
    os.system(f"{SUDO} sed -i 's|subvol=@{distro_suffix}|subvol=@.snapshots{distro_suffix}/rootfs/snapshot-deploy|g' /mnt/boot/grub{v}/grub.cfg")
    # Create a mapping of "distro" <=> "BootOrder number". Ash reads from this file to switch between distros.
    if is_efi:
        if is_boot_external: ### REVIEW_LATER TODO NEW
            os.system(f"efibootmgr -c -d {bp} -p 1 -L {distro_name} -l '\\EFI\\{distro}\\grubx64.efi'")
        ex = os.path.exists("/mnt/boot/efi/EFI/map.txt")
        boot_num = subprocess.check_output(f'efibootmgr -v | grep -i {distro} | awk "{{print $1}}" | sed "s|[^0-9]*||g"', encoding='UTF-8', shell=True)
        with open("/mnt/boot/efi/EFI/map.txt", "a") as m:
            if not ex: m.write("DISTRO,BootOrder\n")
            if boot_num: m.write(distro + ',' + boot_num)

def check_efi():
    return os.path.exists("/sys/firmware/efi")

#   Post bootstrap
def post_bootstrap(super_group):
  # Database and config files
    os.system(f"{SUDO} chmod 700 /mnt/.snapshots/ash/root")
    os.system(f"{SUDO} chmod 1777 /mnt/.snapshots/ash/tmp")
###    os.system(f"{SUDO} ln -srf /mnt/.snapshots/ash/root /mnt/root")
###    os.system(f"{SUDO} ln -srf /mnt/.snapshots/ash/tmp /mnt/tmp")
    os.system(f"echo '0' | {SUDO} tee /mnt/usr/share/ash/snap")
    os.system(f"echo 'mutable_dirs::' | {SUDO} tee /mnt/etc/ash.conf")
    os.system(f"echo 'mutable_dirs_shared::' | {SUDO} tee -a /mnt/etc/ash.conf")
    if distro in ("arch", "cachyos", "endeavouros"):
        os.system(f"echo 'aur::False' | {SUDO} tee -a /mnt/etc/ash.conf")
  # Update fstab
    with open('/mnt/etc/fstab', 'a') as f: # assumes script is run as root
        for mntdir in mntdirs: # common entries
            f.write(f'UUID=\"{to_uuid(os_root)}\" /{mntdir} btrfs subvol=@{mntdir}{distro_suffix},compress=zstd,noatime{"" if mntdir or is_mutable else ",ro"} 0 0\n') # ro only for / entry (and just for immutable installs) ### complex but one-liner
        if is_boot_external:
            f.write(f'UUID=\"{to_uuid(bp)}\" /boot btrfs subvol=@boot{distro_suffix},compress=zstd,noatime 0 0\n')
        if is_home_external:
            f.write(f'UUID=\"{to_uuid(hp)}\" /home btrfs subvol=@home{distro_suffix},compress=zstd,noatime 0 0\n')
        if is_efi:
            f.write(f'UUID=\"{to_uuid(args[3])}\" /boot/efi vfat umask=0077 0 2\n')
        f.write('/.snapshots/ash/root /root none bind 0 0\n')
        f.write('/.snapshots/ash/tmp /tmp none bind 0 0\n')
  # TODO may write these in python
    os.system(f"{SUDO} sed -i '0,/@{distro_suffix}/ s|@{distro_suffix}|@.snapshots{distro_suffix}/rootfs/snapshot-deploy|' /mnt/etc/fstab")
    os.system(f"{SUDO} sed -i '0,/@boot{distro_suffix}/ s|@boot{distro_suffix}|@.snapshots{distro_suffix}/boot/boot-deploy|' /mnt/etc/fstab")
    os.system(f"{SUDO} sed -i '0,/@etc{distro_suffix}/ s|@etc{distro_suffix}|@.snapshots{distro_suffix}/etc/etc-deploy|' /mnt/etc/fstab")
  # Copy common ash files and create symlinks
    os.system(f"{SUDO} mkdir -p /mnt/.snapshots/ash/snapshots")
    os.system(f"echo '{to_uuid(os_root)}' | {SUDO} tee /mnt/.snapshots/ash/part")
    os.system(f"{SUDO} cat ./src/ashpk_core.py ./src/distros/{distro}/ashpk.py > /mnt/.snapshots/ash/ash")
    os.system(f"{SUDO} chmod +x /mnt/.snapshots/ash/ash")
    os.system(f"{SUDO} cp -a ./src/detect_os.sh /mnt/.snapshots/ash/detect_os.sh")
    os.system(f"{SUDO} ln -srf /mnt/.snapshots/ash/ash /mnt/usr/bin/ash")
    os.system(f"{SUDO} ln -srf /mnt/.snapshots/ash/detect_os.sh /mnt/usr/bin/detect_os.sh")
    os.system(f"{SUDO} ln -srf /mnt/.snapshots/ash /mnt/var/lib/ash")
    os.system(f"echo {{\\'name\\': \\'root\\', \\'children\\': [{{\\'name\\': \\'0\\'}}]}} | {SUDO} tee /mnt/.snapshots/ash/fstree") # Initialize fstree
  # Create user and set password
    if distro == "alpine": ### REVIEW not generic enough!
        set_password("root", "") # will fix for "doas"
    else:
        set_password("root")
    if distro !="kicksecure": ### REVIEW not generic enough!
        create_user(username, super_group)
        if distro == "alpine": ### REVIEW not generic enough!
            set_password(username, "") # will fix for "doas"
        else:
            set_password(username) ################ important password for user gets called TWICE!!!
    else:
        print("Username is 'user' please change the default password")
  # Modify OS release information (optional) ### TODO may write in python
    os.system(f"{SUDO} sed -i 's|^ID.*$|ID={distro}_ashos|' /mnt/etc/os-release")
    os.system(f"{SUDO} sed -i 's|^NAME=.*$|NAME=\"{distro_name}\"|' /mnt/etc/os-release")
    os.system(f"{SUDO} sed -i 's|^PRETTY_NAME=.*$|PRETTY_NAME=\"{distro_name}\"|' /mnt/etc/os-release")

#   Common steps before bootstrapping
def pre_bootstrap():
  # Prep (format partition, etc.)
    if is_luks and choice != "2":
        os.system(f"{SUDO} modprobe dm-crypt")
        print("--- Create LUKS partition --- ")
        os.system(f"{SUDO} cryptsetup -y -v -c aes-xts-plain64 -s 512 --hash sha512 --pbkdf pbkdf2 --type luks2 luksFormat {args[1]}")
        print("--- Open LUKS partition --- ")
        os.system(f"{SUDO} cryptsetup --allow-discards --persistent --type luks2 open {args[1]} luks_root")
  # Mount and create necessary sub-volumes and directories
    if is_format_btrfs:
        if not os.path.exists("/dev/btrfs-control"): # Recommended for Alpine for instance (optional)
            os.system(f"{SUDO} btrfs rescue create-control-device")
        if choice == "1":
            os.system(f"{SUDO} mkfs.btrfs -L LINUX -f {os_root}")
            os.system(f"{SUDO} mount -t btrfs {os_root} /mnt")
        elif choice == "2":
            os.system(f"{SUDO} mount -o subvolid=5 {os_root} /mnt")
        for btrdir in btrdirs: # common entries
            os.system(f"{SUDO} btrfs sub create /mnt/{btrdir}")
        if is_boot_external:
            os.system(f"{SUDO} btrfs sub create /mnt/@boot{distro_suffix}")
        if is_home_external:
            os.system(f"{SUDO} btrfs sub create /mnt/@home{distro_suffix}")
        os.system(f"{SUDO} umount /mnt")
        for mntdir in mntdirs: # common entries
            os.system(f"{SUDO} mkdir -p /mnt/{mntdir}") # -p to ignore /mnt exists complaint
            os.system(f"{SUDO} mount {os_root} -o subvol={btrdirs[mntdirs.index(mntdir)]},compress=zstd,noatime /mnt/{mntdir}")
            #os.system(f'{SUDO} mount {bp if is_boot_external and mntdir == "boot" else os_root} -o {"subvol="+btrdirs[mntdirs.index(mntdir)]+"," if not (is_boot_external and mntdir == "boot") else ""}compress=zstd,noatime /mnt/{mntdir}') ### NEWER but won't work because of new structuring of mntdirs so REVERTED to first version. Kept for future reference
    if is_boot_external:
        os.system(f"{SUDO} mkdir /mnt/boot")
        os.system(f"{SUDO} mount -m {bp} -o compress=zstd,noatime /mnt/boot")
    if is_home_external:
        os.system(f"{SUDO} mkdir /mnt/home")
        os.system(f"{SUDO} mount -m {hp} -o compress=zstd,noatime /mnt/home")
    for i in ("tmp", "root"):
        os.system(f"mkdir -p /mnt/{i}")
    for i in ("ash", "boot", "etc", "root", "rootfs", "tmp"): ### REVIEW_LATER is "var" missing here?!!!
        os.system(f"mkdir -p /mnt/.snapshots/{i}")
    for i in ("root", "tmp"): # necessary to prevent error booting some distros
        os.system(f"mkdir -p /mnt/.snapshots/ash/{i}")
    os.system(f"{SUDO} mkdir -p /mnt/usr/share/ash/db") ### REVIEW was in step "Database and config files" before (better to create after bootstrap for aesthetics)
    if is_efi:
        os.system(f"{SUDO} mkdir -p /mnt/boot/efi")
        os.system(f"{SUDO} mount {args[3]} /mnt/boot/efi")

def set_password(u, s="sudo"): ### REVIEW Use super_group?
    clear()
    while True:
        print(f"Setting a password for '{u}':")
        os.system(f"{SUDO} chroot /mnt {s} passwd {u}")
        if yes_no("Was your password set properly?"):
            break
        else:
            continue

def to_uuid(part):
    if 'busybox' in os.path.realpath(which('blkid')): # type: ignore
        u = subprocess.check_output(f"blkid {part}", shell=True).decode('utf-8').strip()
        return search('UUID="(.+?)"' , u).group(1)
    else: # util-linx (non-Alpine)
        return subprocess.check_output(f"blkid -s UUID -o value {part}", shell=True).decode('utf-8').strip()

#   Unmount everything and finish
def unmounts():
    os.system(f"{SUDO} umount --recursive /mnt")
    os.system(f"{SUDO} mount {os_root} -o subvolid=0 /mnt")
    os.system(f"{SUDO} btrfs sub del /mnt/@{distro_suffix}")
    os.system(f"{SUDO} umount --recursive /mnt")
    if is_luks:
        os.system(f"{SUDO} cryptsetup close luks_root")

#   Generic yes no prompt
def yes_no(msg):
    clear()
    while True:
        print(f"{msg} (y/n)")
        reply = input("> ")
        if reply.casefold() in ('yes', 'y'):
            e = True
            break
        elif reply.casefold() in ('no', 'n'):
            e = False
            break
        else:
            print("F: Invalid choice!")
            continue
    return e

# ---------------------------------------------------------------------------- #

print("Welcome to the AshOS installer!\n")
with open('res/logos/logo.txt', 'r') as f:
    print(f.read())

#   Define variables
DEBUG = "" # options: "", " >/dev/null 2>&1"
choice, distro_suffix = get_multiboot(distro)
is_format_btrfs = True ### REVIEW TEMPORARY
is_boot_external = yes_no("Would you like to use a separate boot partition?")
is_home_external = yes_no("Would you like to use a separate home partition?")
is_mutable = yes_no("Would you like this installation to be mutable?")
if is_boot_external and is_home_external:
    btrdirs = [f"@{distro_suffix}", f"@.snapshots{distro_suffix}", f"@etc{distro_suffix}", f"@var{distro_suffix}"]
    mntdirs = ["", ".snapshots", "etc", "var"]
    bp = get_external_partition('boot')
    hp = get_external_partition('home')
elif is_boot_external:
    btrdirs = [f"@{distro_suffix}", f"@.snapshots{distro_suffix}", f"@etc{distro_suffix}", f"@home{distro_suffix}", f"@var{distro_suffix}"]
    mntdirs = ["", ".snapshots", "etc", "home", "var"]
    bp = get_external_partition('boot')
elif is_home_external:
    btrdirs = [f"@{distro_suffix}", f"@.snapshots{distro_suffix}", f"@boot{distro_suffix}", f"@etc{distro_suffix}", f"@var{distro_suffix}"]
    mntdirs = ["", ".snapshots", "boot", "etc", "var"]
    hp = get_external_partition('home')
else:
    btrdirs = [f"@{distro_suffix}", f"@.snapshots{distro_suffix}", f"@boot{distro_suffix}", f"@etc{distro_suffix}", f"@home{distro_suffix}", f"@var{distro_suffix}"]
    mntdirs = ["", ".snapshots", "boot", "etc", "home", "var"]
is_luks = yes_no("Would you like to use LUKS?")
is_efi = check_efi()
if is_luks:
    os_root = "/dev/mapper/luks_root"
    if is_efi:
        luks_grub_args = "luks2 btrfs part_gpt cryptodisk pbkdf2 gcry_rijndael gcry_sha512"
    else:
        luks_grub_args = "luks2 btrfs biosdisk part_msdos cryptodisk pbkdf2 gcry_rijndael gcry_sha512"
else:
    os_root = args[1]
    luks_grub_args = ""
hostname = get_name('hostname')
username = get_name('username') ### REVIEW 2023 made it global variable for Alpine installer
tz = get_item_from_path("timezone", "/usr/share/zoneinfo")

