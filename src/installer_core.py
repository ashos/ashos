#!/usr/bin/env python3

import os
import stat
import socket
import subprocess as sp
import sys
from glob import glob
from re import search
from setup import args, installer_dir, distro, distro_name
from shutil import copy, which, rmtree # REVIEW remove rmtree later
from tempfile import TemporaryDirectory
from urllib.error import URLError, HTTPError
from urllib.request import urlopen

SUDO = "sudo" # REVIEW remove if not needed in any distro

# ------------------------------ CORE FUNCTIONS ------------------------------ #

#   Mount-points needed for chrooting
def ashos_mounts():
    os.system(f"{SUDO} mount -o x-mount.mkdir --rbind --make-rslave /dev /mnt/dev")
    os.system(f"{SUDO} mount -o x-mount.mkdir --types proc /proc /mnt/proc")
    os.system(f"{SUDO} mount -o x-mount.mkdir --bind --make-slave /run /mnt/run")
    os.system(f"{SUDO} mount -o x-mount.mkdir --rbind --make-rslave /sys /mnt/sys")
    if is_efi:
        os.system(f"{SUDO} mount -o x-mount.mkdir --rbind --make-rslave /sys/firmware/efi/efivars /mnt/sys/firmware/efi/efivars")
    os.system(f"{SUDO} cp --dereference /etc/resolv.conf /mnt/etc/") # --remove-destination ? # not writing through dangling symlink! # TODO: 1. move to post_bootstrap 2. try except else

#   Bundle everything ash needs in one executable
def bundler():
    with TemporaryDirectory(dir="/tmp", prefix="ash.") as tmpdir:
        csmp_file = urlopen("https://github.com/ashos/bundle/raw/main/python311anytree.com").read()
        open(f"{tmpdir}/python.com", "wb").write(csmp_file)
        open(f"{tmpdir}/.args", "w").write("/zip/ash\n...")
        os.system(f"cat {installer_dir}/src/ashpk_core.py {installer_dir}/src/distros/{distro}/ashpk.py > {tmpdir}/ash")
        os.system(f"zip -j {tmpdir}/python.com {tmpdir}/ash {tmpdir}/.args")
      # Make it executable
        mode = stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH
        os.chmod(f"{tmpdir}/python.com", mode)
        if is_efi:
            if not os.path.ismount(args[3]): # mount EFI if not mounted
                os.system(f"mount {args[3]} /mnt/boot/efi")
            os.system(f"mv {tmpdir}/python.com /mnt/boot/efi/ash")
            os.system(f"{SUDO} umount {args[3]}") # REVIEW redundant?
        else:
            if not os.path.ismount(f"{args[2]}1"):
                os.mkdir(f"{tmpdir}/temp_ash")
                os.system(f"mount {args[2]}1 {tmpdir}/temp_ash") # important NOT {args[1]}
            os.system(f"mv {tmpdir}/python.com {tmpdir}/temp_ash/ash") # TODO in ashpk.py
            os.system(f"{SUDO} umount {tmpdir}/temp_ash") # REVIEW redundant?
    print("Bundle created!")

#   Bundle everything ash needs in one executable
def bundler_advanced():
    with TemporaryDirectory(dir="/tmp", prefix="ash.") as tmpdir:
        anytree_url = ""
        ext = ""
        try:
            temp = urlopen("https://api.github.com/repos/c0fec0de/anytree/releases/latest").read().decode('utf-8')
            if which("unzip"):
                anytree_url = search('zipball_url":"(.+?)"', temp).group(1)
                ext = ".zip"
            elif which("tar"):
                anytree_url = search('tarball_url":"(.+?)"', temp).group(1)
                ext = ".tar.gz"
            else:
                print("F: package zip/tar not available!")
            anytree_file = urlopen(anytree_url).read()
            open(f"{tmpdir}/anytree{ext}", "wb").write(anytree_file)
            os.mkdir(f"{tmpdir}/.python") # modules folder to be bundled
            if ext == ".zip":
                os.mkdir(f"{tmpdir}/TEMP")
                os.system(f"unzip {tmpdir}/anytree{ext} -d {tmpdir}/TEMP")
                os.system(f"mv {tmpdir}/TEMP/*anytree*/anytree {tmpdir}/.python/")
            elif ext == ".tar.gz":
                if "busybox" in os.path.realpath(which("tar")):
                    os.mkdir(f"{tmpdir}/TEMP")
                    os.system(f"tar x -f {tmpdir}/anytree{ext} -C {tmpdir}/TEMP")
                    os.system(f"mv {tmpdir}/TEMP/*anytree*/anytree {tmpdir}/.python/")
                else:
                    os.mkdir(f"{tmpdir}/TEMP")
                    os.system(f"tar x -f {tmpdir}/*anytree*{ext} -C {tmpdir}/TEMP")
                    os.system(f"mv {tmpdir}/TEMP/*anytree*/anytree {tmpdir}/.python/")
            csmp_file = urlopen("http://justine.lol/ftrace/python.com").read()
            open(f"{tmpdir}/python.com", "wb").write(csmp_file)
          # .args
            open(f"{tmpdir}/.args", "w").write("/zip/ash\n...")
          # six
            six_file = urlopen("https://raw.githubusercontent.com/benjaminp/six/master/six.py").read().decode('utf-8')
            open(f"{tmpdir}/six.py", 'w').write(six_file)
            os.system(f"mv {tmpdir}/six.py {tmpdir}/.python/")
        except (HTTPError, URLError):
            print(f"F: Failed to bundle ash.")
        else:
            os.system(f"cat {installer_dir}/src/ashpk_core.py {installer_dir}/src/distros/{distro}/ashpk.py > {tmpdir}/ash")
            os.system(f"cd {tmpdir} && zip -ur python.com .python ash .args")
          # Make it executable
            mode = stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH
            os.chmod(f"{tmpdir}/python.com", mode)
            if is_efi:
                if not os.path.ismount(args[3]): # mount EFI if not mounted
                    os.system(f"mount {args[3]} /mnt/boot/efi")
                os.system(f"mv {tmpdir}/python.com /mnt/boot/efi/ash")
                os.system(f"{SUDO} umount {args[3]}") # REVIEW redundant?
            else:
                if not os.path.ismount(f"{args[2]}1"):
                    os.mkdir(f"{tmpdir}/temp_ash")
                    os.system(f"mount {args[2]}1 {tmpdir}/temp_ash") # important NOT {args[1]}
                os.system(f"mv {tmpdir}/python.com {tmpdir}/temp_ash/ash") # TODO in ashpk.py
                os.system(f"{SUDO} umount {tmpdir}/temp_ash") # REVIEW redundant?
            #anytree_file.close() # REVIEW redundant
            #csmp_file.close() # REVIEW redundant
            print("Bundle created!")

def chroot_in(path):
    real_root = os.open("/", os.O_RDONLY) # or "." ?
    os.chroot(path) # can switch places with next line?
    os.chdir(path)
    return real_root

def chroot_out(rr):
    os.fchdir(rr)
    os.chroot(".") # or "/" ?
    os.close(rr) # Back to old root

#   Clear screen
def clear():
    os.system("#clear")

#   Users
def create_user(u, g):
  # Get path to useradd/adduser even if not in PATH (e.g. Fedora)
    uau = find_command(["useradd", "adduser"])
    if 'busybox' in os.path.realpath(uau):
        os.system(f"{uau} -h /home/{u} -G {g} -s /bin/sh -D {u}")
    else: # util-linx (non-Alpine)
        os.system(f"{uau} -m -G {g} -s /bin/sh {u}")
    open("/etc/sudoers", "a").write(f"%{g} ALL=(ALL:ALL) ALL")
  # Get true name of target OS shell # NOT $(echo $0) or os.environ.get('SHELL')
    sh = os.path.basename(os.path.realpath(which('sh')))
    if sh == "busybox":
        sh = "ash" # REVIEW not generic # ~/.profile ?
    user_id = sp.check_output(f"id -u {u}", encoding='UTF-8', shell=True).strip() # usually 1000
    open(f"/home/{u}/.{sh}rc", "a").write(f'export XDG_RUNTIME_DIR="/run/user/{user_id}"')

#   BTRFS snapshots
def deploy_base_snapshot(): # REVIEW removed "{SUDO}" from all lines below
    btrfs = find_command(["btrfs"])
    os.system(f"{btrfs} sub snap {'' if is_mutable else '-r'} / /.snapshots/rootfs/snapshot-0")
    os.system(f"{btrfs} sub create /.snapshots/boot/boot-deploy")
    os.system(f"{btrfs} sub create /.snapshots/etc/etc-deploy")
    os.system("cp -r --reflink=auto /boot/. /.snapshots/boot/boot-deploy")
    os.system("cp -r --reflink=auto /etc/. /.snapshots/etc/etc-deploy")
    os.system(f"{btrfs} sub snap {'' if is_mutable else '-r'} /.snapshots/boot/boot-deploy /.snapshots/boot/boot-0")
    os.system(f"{btrfs} sub snap {'' if is_mutable else '-r'} /.snapshots/etc/etc-deploy /.snapshots/etc/etc-0")
    if is_mutable: # Mark base snapshot as mutable
            os.system("touch /.snapshots/rootfs/snapshot-0/usr/share/ash/mutable")
    os.system(f"{btrfs} sub snap /.snapshots/rootfs/snapshot-0 /.snapshots/rootfs/snapshot-deploy")
    os.system(f"{btrfs} sub set-default /.snapshots/rootfs/snapshot-deploy")
    os.system("cp -r /root/. /.snapshots/root/")
    os.system("cp -r /tmp/. /.snapshots/tmp/")
    rmrf_star("/root")
    rmrf_star("/tmp")

# deploy_to_common_in_chroot()
#   Copy boot and etc: deployed snapshot <---> common
def deploy_to_common():
    if is_efi:
        os.system("umount /boot/efi")
    os.system("umount /boot")
    #os.system(f'mount {bp if is_boot_external else os_root} -o {"subvol="+f"@boot{distro_suffix}"+"," if not is_boot_external else ""}compress=zstd,noatime /.snapshots/boot/boot-deploy') # REVIEW A similar line for is_home_external needed?
    if is_boot_external:
        os.system(f"mount {bp} -o compress=zstd,noatime /.snapshots/boot/boot-deploy")
    else:
        os.system(f"mount {os_root} -o subvol=@boot{distro_suffix},compress=zstd,noatime /.snapshots/boot/boot-deploy")
    os.system("cp -r --reflink=auto /.snapshots/boot/boot-deploy/. /boot/")
    os.system("umount /etc")
    os.system(f"mount {os_root} -o subvol=@etc{distro_suffix},compress=zstd,noatime /.snapshots/etc/etc-deploy")
    os.system("cp -r --reflink=auto /.snapshots/etc/etc-deploy/. /etc/")
    os.system("cp -r --reflink=auto /.snapshots/boot/boot-0/. /.snapshots/rootfs/snapshot-deploy/boot/")
    os.system("cp -r --reflink=auto /.snapshots/etc/etc-0/. /.snapshots/rootfs/snapshot-deploy/etc/")

#   Find a command stubbornly
def find_command(cmds):
    r = None
    for cmd in cmds:
        a = which(cmd)
        b = which(f"/sbin/{cmd}")
        c = which(f"/usr/sbin/{cmd}")
        if a:
            r = a
        elif b: # to solve issue of blkid not found when os-prober is run
            r = b
            os.symlink(f"/sbin/{cmd}", f"/bin/{cmd}")
        elif c:
            r = c
            os.symlink(f"/usr/sbin/{cmd}", f"/bin/{cmd}")
    if r:
        return r
    else:
        sys.exit(f"F: Command {cmds} not found!")
#    return which(cmd) or which(f"/sbin/{cmd}") or which("/usr/sbin/{cmd}")

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

#   Return IP address
def get_ip():
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.settimeout(0)
    try:
        # doesn't even have to be reachable
        s.connect(('10.254.254.254', 1))
        IP = s.getsockname()[0]
    except Exception:
        IP = '127.0.0.1'
    finally:
        s.close()
    return IP

#   Generic function to choose something from a directory
def get_item_from_path(thing, a_path):
    clear()
    while True:
        print(f"Select a {thing} (type list to list):")
        ch = input("> ")
        if ch == "list":
            ch = []
            for root, _dirs, files in os.walk(a_path, followlinks=True):
                for file in files:
                    ch.append(os.path.join(root, file).replace(f"{a_path}/", ""))
            ch = "\n".join(sorted(ch))
            os.system(f"echo '{ch}' | less")
        else:
            temp = str(f"{a_path}/{ch}")
            if not ( os.path.isfile(temp) or os.path.isdir(temp) ):
                print(f"Invalid {thing}!")
                continue
            return ch # REVIEW originally was just break

#   GRUB and EFI
def grub_ash(v): # REVIEW removed "{SUDO}" from all lines below
    grub_install = find_command([f"grub{v}-install"])
    grub_mkconfig = find_command([f"grub{v}-mkconfig"])
    grub_mkimage = find_command([f"grub{v}-mkimage"])
    if is_format_btrfs:
        find_command(["btrfs"]) # REVIEW symlink to prevent error in os-prober
    os.system(f"sed -i 's/^GRUB_DISTRIBUTOR.*$/GRUB_DISTRIBUTOR=\"{distro_name}\"/' /etc/default/grub")
    if is_luks:
        os.system("sed -i 's/^#GRUB_ENABLE_CRYPTODISK.*$/GRUB_ENABLE_CRYPTODISK=y/' /etc/default/grub")
        os.system(f"sed -i -E 's|^#?GRUB_CMDLINE_LINUX=\"|GRUB_CMDLINE_LINUX=\"cryptdevice=UUID={to_uuid(args[1])}:luks_root cryptkey=rootfs:/etc/crypto_keyfile.bin|' /etc/default/grub")
        os.system(f"sed -e 's|DISTRO|{distro}|' -e 's|LUKS_UUID_NODASH|{to_uuid(args[1]).replace('-', '')}|' \
                        -e '/^#/d' /tmp/grub_luks2.conf > /etc/grub_luks2.conf") # REVIEW
  # grub-install rewrites default core.img, so run grub-mkimage AFTER!
    if distro != "fedora": # https://bugzilla.redhat.com/show_bug.cgi?id=1917213
        if is_efi:
            os.system(f'{grub_install} {bp if is_boot_external else args[2]} --bootloader-id={distro} --modules="{luks_grub_args}" --target=x86_64-efi') # --efi-directory=/boot/efi # REVIEW
        else:
            os.system(f'{grub_install} {bp if is_boot_external else args[2]} --bootloader-id={distro} --modules="{luks_grub_args}"') # REVIEW: --target needed for non-uefi?
    if is_luks: # make LUKS2-compatible grub image
        if is_efi:
            os.system(f'{grub_mkimage} -p "(crypto0)/@boot_{distro}/grub{v}" -O x86_64-efi -c /etc/grub_luks2.conf -o /boot/efi/EFI/{distro}/grubx64.efi {luks_grub_args}') # without '/grub' gives error normal.mod not found (maybe only one of these here and grub_luks2.conf is enough?!)
        else:
            os.system(f'{grub_mkimage} -p "(crypto0)/@boot_{distro}/grub{v}" -O i386-pc -c /etc/grub_luks2.conf -o /boot/grub{v}/i386-pc/core_luks2.img {luks_grub_args}') # 'biosdisk' module not needed eh?
            os.system(f'dd oflag=seek_bytes seek=512 if=/boot/grub{v}/i386-pc/core_luks2.img of={bp if is_boot_external else args[2]}') # REVIEW
    os.system(f"{grub_mkconfig} {bp if is_boot_external else args[2]} -o /boot/grub{v}/grub.cfg")
    os.system(f"mkdir -p /boot/grub{v}/BAK") # Folder for backing up grub configs created by ashpk
    os.system(f"sed -i 's|subvol=@{distro_suffix}|subvol=@.snapshots{distro_suffix}/rootfs/snapshot-deploy|g' /boot/grub{v}/grub.cfg")
    # Create a mapping of "distro" <=> "BootOrder number". Ash reads from this file to switch between distros.
    if is_efi:
        efibootmgr = find_command(["efibootmgr"])
        if is_boot_external:
            os.system(f"{efibootmgr} -c -d {bp} -p 1 -L {distro_name} -l '\\EFI\\{distro}\\grubx64.efi'")
        ex = os.path.exists("/boot/efi/EFI/map.txt")
        boot_num = sp.check_output(f'{efibootmgr} -v | grep -i {distro} | awk "{{print $1}}" | sed "s|[^0-9]*||g"', encoding='UTF-8', shell=True)
        with open("/boot/efi/EFI/map.txt", "a") as m:
            if not ex: m.write("DISTRO,BootOrder\n")
            if boot_num: m.write(distro + ',' + boot_num)

def check_efi():
    return os.path.exists("/sys/firmware/efi")

#   Post bootstrap
def post_bootstrap(super_group): # REVIEW removed "{SUDO}" from all lines below
  # Database and config files
    #os.system(f"{SUDO} ln -srf /mnt/.snapshots/ash/root /mnt/root")
    #os.system(f"{SUDO} ln -srf /mnt/.snapshots/ash/tmp /mnt/tmp")
    os.system("chmod 700 /.snapshots/ash/root")
    os.system("chmod 1777 /.snapshots/ash/tmp")
    os.system(f"echo '0' > /usr/share/ash/snap")
    os.system(f"echo 'mutable_dirs::' > /etc/ash.conf")
    os.system(f"echo 'mutable_dirs_shared::' >> /etc/ash.conf")
    if distro in ("arch", "cachyos", "endeavouros"):
        os.system(f"echo 'aur::False' >> /etc/ash.conf")
  # Update fstab
    with open('/etc/fstab', 'a') as f: # assumes script run as root # REVIEW 'w'
        for mntdir in mntdirs: # common entries
            f.write(f'UUID=\"{to_uuid(os_root)}\" /{mntdir} btrfs subvol=@{mntdir}{distro_suffix},compress=zstd,noatime{"" if mntdir or is_mutable else ",ro"} 0 0\n') # ro for / entry (only for immutable installs)
        if is_boot_external:
            f.write(f'UUID=\"{to_uuid(bp)}\" /boot btrfs subvol=@boot{distro_suffix},compress=zstd,noatime 0 0\n')
        if is_home_external:
            f.write(f'UUID=\"{to_uuid(hp)}\" /home btrfs subvol=@home{distro_suffix},compress=zstd,noatime 0 0\n')
        if is_efi:
            f.write(f'UUID=\"{to_uuid(args[3])}\" /boot/efi vfat umask=0077 0 2\n')
        if is_ash_bundle and not is_efi:
            f.write(f'UUID=\"{to_uuid(args[2]+"1")}\" /.snapshots/ash/bundle vfat umask=0077 0 2\n')
        f.write('/.snapshots/ash/root /root none bind 0 0\n')
        f.write('/.snapshots/ash/tmp /tmp none bind 0 0\n')
  # TODO may write these in python
    os.system(f"sed -i '0,/@{distro_suffix}/ s|@{distro_suffix}|@.snapshots{distro_suffix}/rootfs/snapshot-deploy|' /etc/fstab")
    os.system(f"sed -i '0,/@boot{distro_suffix}/ s|@boot{distro_suffix}|@.snapshots{distro_suffix}/boot/boot-deploy|' /etc/fstab")
    os.system(f"sed -i '0,/@etc{distro_suffix}/ s|@etc{distro_suffix}|@.snapshots{distro_suffix}/etc/etc-deploy|' /etc/fstab")
  # files operations (part 2) - create symlinks
    os.system("mkdir -p /.snapshots/ash/snapshots")
    os.system(f"echo '{to_uuid(os_root)}' > /.snapshots/ash/part")
    if is_ash_bundle:
        if is_efi:
            os.system("ln -sf /boot/efi/ash /usr/bin/ash")
        else:
            os.system("ln -sf /.snapshots/ash/bundle/ash /usr/bin/ash")
    else:
        os.system("ln -sf /.snapshots/ash/ash /usr/bin/ash")
    #os.system(f"{SUDO} ln -srf /mnt/.snapshots/ash/detect_os.sh /mnt/usr/bin/detect_os.sh")
    os.system("ln -sf /.snapshots/ash /var/lib/ash")
  # Initialize fstree
    os.system("echo {\\'name\\': \\'root\\', \\'children\\': [{\\'name\\': \\'0\\'}]} > /.snapshots/ash/fstree")
  # Create user and set password
    if distro == "alpine": # REVIEW not generic
        set_password("root", "") # will fix for "doas"
    else:
        set_password("root")
    if distro !="kicksecure": # REVIEW not generic
        create_user(username, super_group)
        if distro == "alpine": # REVIEW not generic
            set_password(username, "") # will fix for "doas"
        else:
            set_password(username)
    else:
        print("Username is 'user' please change the default password")
  # Modify OS release information (optional)
    os.system(f"sed -i 's|^ID.*$|ID={distro}_ashos|' /etc/os-release")
    os.system(f"sed -i 's|^NAME=.*$|NAME=\"{distro_name}\"|' /etc/os-release")
    os.system(f"sed -i 's|^PRETTY_NAME=.*$|PRETTY_NAME=\"{distro_name}\"|' /etc/os-release")

#   Common steps before bootstrapping
def pre_bootstrap(): # REVIEW removed {SUDO} from all lines below
  # Prep (format partition, etc.)
    if is_luks and choice != "2":
        os.system("modprobe dm-crypt")
        print("--- Create LUKS partition --- ")
        os.system(f"cryptsetup -y -v -c aes-xts-plain64 -s 512 --hash sha512 --pbkdf pbkdf2 --type luks2 luksFormat {args[1]}")
        print("--- Open LUKS partition --- ")
        os.system(f"cryptsetup --allow-discards --persistent --type luks2 open {args[1]} luks_root")
  # Mount and create necessary sub-volumes and directories
    if is_format_btrfs:
        if not os.path.exists("/dev/btrfs-control"): # recommended for Alpine (optional)
            os.system("btrfs rescue create-control-device")
        if choice == "1":
            os.system(f"mkfs.btrfs -L LINUX -f {os_root}")
            os.system(f"mount -t btrfs {os_root} /mnt")
        elif choice == "2":
            os.system(f"mount -o subvolid=5 {os_root} /mnt")
        for btrdir in btrdirs: # common entries
            os.system(f"btrfs sub create /mnt/{btrdir}")
        if is_boot_external:
            os.system(f"btrfs sub create /mnt/@boot{distro_suffix}")
        if is_home_external:
            os.system(f"btrfs sub create /mnt/@home{distro_suffix}")
        os.system("umount /mnt")
        for mntdir in mntdirs: # common entries
            os.system(f"mkdir -p /mnt/{mntdir}") # -p to ignore /mnt exists complaint
            os.system(f"mount {os_root} -o subvol={btrdirs[mntdirs.index(mntdir)]},compress=zstd,noatime /mnt/{mntdir}")
    if is_boot_external:
        os.system("mkdir /mnt/boot")
        os.system(f"mount -m {bp} -o compress=zstd,noatime /mnt/boot")
    if is_home_external:
        os.system("mkdir /mnt/home")
        os.system(f"mount -m {hp} -o compress=zstd,noatime /mnt/home")
    for i in ("tmp", "root"):
        os.system(f"mkdir -p /mnt/{i}")
    for i in ("ash", "boot", "etc", "root", "rootfs", "tmp"): # REVIEW "var" missing here?
        os.system(f"mkdir -p /mnt/.snapshots/{i}")
    for i in ("root", "tmp"): # necessary to prevent error booting some distros
        os.system(f"mkdir -p /mnt/.snapshots/ash/{i}")
    if is_ash_bundle and not is_efi:
        os.system(f"mkdir -p /mnt/.snapshots/ash/bundle") # REVIEW /mnt/boot/bundle better?
    os.system("mkdir -p /mnt/usr/share/ash/db") # REVIEW was in step "Database and config files" before (better to create after bootstrap for aesthetics)
    if is_efi:
        os.system("mkdir -p /mnt/boot/efi")
        os.system(f"mount {args[3]} /mnt/boot/efi")
  # files operations (part 1) - copy to chroot, as still accessible (inside host)
#    if not os.path.exists("/mnt/etc/profile.d"):
#        os.mkdir("/mnt/etc/profile.d")
#    os.system(f"cp -a {installer_dir}/src/prep/env_path.sh /mnt/etc/profile.d/ashos.sh") # REVIEW
    if is_luks:
        os.system(f"cp -a {installer_dir}/src/prep/grub_luks2.conf /mnt/tmp/")
    if not is_ash_bundle: # else: post function will handle
        os.system(f"cat {installer_dir}/src/ashpk_core.py {installer_dir}/src/distros/{distro}/ashpk.py > /mnt/.snapshots/ash/ash")
        os.system("chmod +x /mnt/.snapshots/ash/ash")
        os.system(f"cp -a {installer_dir}/src/detect_os.py /mnt/.snapshots/ash/")

#   rm -rf for deleting everything recursively (even top folder)
def rmrf(a_path):
    os.system(f"rm -rf {a_path}")

#   rm -rf for just deleting contents
def rmrf_star(a_path):
    os.system(f"rm -rf {a_path}/*")

#   rm -rf for deleting everything recursively (even top folder)
def rmrf_ERRORS(*item): # TODO
    for f in item:
        if os.path.isdir(f):
            rmtree(f)
        else:
            os.unlink(f)

#   rm -rf for just deleting contents
def rmrf_star_ERROR(a_path): # TODO
    files = glob(f"{a_path}/*")
    rmrf(*files)

def set_password(u, s="sudo"): # REVIEW Use super_group?
    clear()
    while True:
        print(f"Setting a password for '{u}':")
        os.system(f"{s} passwd {u}")
        if yes_no("Was your password set properly?"):
            break
        else:
            continue

def to_uuid(part):
  # Get path to blkid even if not in PATH (e.g. Fedora)
    blkid = find_command(["blkid"])
    if 'busybox' in os.path.realpath(blkid):
        u = sp.check_output(f"{blkid} {part}", encoding='UTF-8', shell=True).strip()
        return search('UUID="(.+?)"', u).group(1)
    else: # util-linx (non-Alpine)
        return sp.check_output(f"{blkid} -s UUID -o value {part}", encoding='UTF-8', shell=True).strip()

#   Unmount everything
def unmounts(install=""): # REVIEW at least for Arch, {SUDO} is not needed
    os.system(f"{SUDO} umount --recursive /mnt")
    os.system(f"{SUDO} mount {os_root} -o subvolid=0 /mnt")
    if install == "failed":
        os.system(f"{SUDO} btrfs sub del /mnt/@*")
    else:
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

print(f"Welcome to the AshOS installer for {distro_name}!\n")
with open(f'{installer_dir}/res/logos/logo.txt', 'r') as f:
    print(f.read())

#   Define variables
DEBUG = "" # options: "", " >/dev/null 2>&1"
choice, distro_suffix = get_multiboot(distro)
is_format_btrfs = True # REVIEW temporary
is_efi = check_efi()
is_ash_bundle = yes_no("Would you like ash as a bundle?")
if is_ash_bundle and not is_efi:
    print("A special partitioning layout should be used to achieve this. Please modify and run MBR prep script.")
    if not yes_no("Confirm that you have done previous step?"):
        sys.exit("F: Please modify and run MBR prep script and run setup later!")
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
#if is_ash_bundle and not is_efi: # disadvantage: only for BTRFS
#    mntdirs += " bundle"
#    btrdirs.append(f"@bundle{distro_suffix}")
is_luks = yes_no("Would you like to use LUKS?")
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
username = get_name('username') # REVIEW made it global variable for Alpine installer
tz = get_item_from_path("timezone", "/usr/share/zoneinfo")

