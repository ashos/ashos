stratisd-dracut#!/bin/sh

if [ $(id -u) -ne 0 ]; then echo "Please run as root!"; exit 1; fi

if [ $# -eq 0 ]; then
    echo "No hard drive specified!"
    exit 1
fi

parted --align minimal --script $1 mklabel gpt unit MiB \
        mkpart ESP fat32 0% 256 set 1 boot on \
        mkpart primary ext4 256 80% \
        mkpart primary ext4 80% 90%
mkfs.vfat -F32 -n EFI ${1}1


systemctl start --now stratisd

# removed all sudo prefixes in commands as sudo not available in rhel live iso
# rhel DOES need blscfg=false
# I might need to install pip so I can instal subscription-manager
# dnf repolist - list all enabled Yum repositories in your system

#######3 step2 begins here

# CTRL+ALT+F2 and do the following in terminal

stratis pool create --no-overprovision myrhel9pool /dev/sda2
stratis filesystem create myrhel9pool fs01

#stratis filesystem list

#mkdir /mnt/fs01

### By default the filesystem used fofr stratis pool is xfs so the following command is not needed at all
### mkfs.xfs /dev/mapper/stratis-1-XYZ-thin-fs-UUID <-- what is random string XYZ???

#mount /dev/stratis/myrhel9pool/fs01 /mnt/fs01

#mount /dev/mapper/stratis-1-XYZ-thin-fs-ABC /mnt/fs01

# stratis pool list
# stratis blockdev list
# stratis pool add-data myrhel9pool /dev/sdX7 <-- expand stratis pool
# stratis pool add-cache myrhel9cachepool /dev/sdZ9 <-- add stratis cache pool
# stratis pool rename myrhel9pool yourrhel9pool
# umount /mnt/fs05 && stratis filesystem destroy myrhel9pool fs05

# Actual install with terminal attempt 1
mount /dev/stratis/myrhel9pool/fs01 /mnt
mkdir /mnt/boot
mkfs.ext4 /dev/sda2
mount /dev/sda2 /mnt/boot

if is_efi:
        os.system("sudo mkdir -p /mnt/boot/efi")
        os.system(f"sudo mount {args[3]} /mnt/boot/efi")

mkdir /mnt/etc
## didn't create these but maybe needed to be uniform with other types of installation? /mnt/home var boot .snapshot

#   Mount-points needed for chrooting
def ash_chroot():
    os.system("sudo mount -o x-mount.mkdir --rbind --make-rslave /dev /mnt/dev")
    os.system("sudo mount -o x-mount.mkdir --types proc /proc /mnt/proc")
    os.system("sudo mount -o x-mount.mkdir --bind --make-slave /run /mnt/run")
    os.system("sudo mount -o x-mount.mkdir --rbind --make-rslave /sys /mnt/sys")
    if is_efi:
        os.system("sudo mount -o x-mount.mkdir --rbind --make-rslave /sys/firmware/efi/efivars /mnt/sys/firmware/efi/efivars")
        
####### step3 begins here
        
    os.system("sudo cp --dereference /etc/resolv.conf /mnt/etc/") # --remove-destination ### not writing through dangling symlink! (TODO: try except) <----------- NOT SURE IF THIS NEEDED FOR RHEL+STRATIS install?!!!

subscription-manager register


#For rhel
packages = "kernel dnf efibootmgr passwd sudo shim-x64 grub2-efi-x64 grub2-efi-x64-modules \
            glibc-langpack-en glibc-locale-source dhcp-server dhcp-client dhcp-common NetworkManager stratisd  stratis-cli dracut stratisd-dracut grub2-tools" ## python-anytree sqlite-tools btrfs-progs not available in rhel!

#for fedora
packages = "kernel dnf efibootmgr passwd sudo shim-x64 grub2-efi-x64 grub2-efi-x64-modules \
            glibc-langpack-en glibc-locale-source dhcpcd NetworkManager stratisd stratis-cli dracut stratisd-dracut grub2-tools" ## python-anytree sqlite-tools btrfs-progs not available in rhel!

if live_iso == "arch":
    RELEASEVER=""
elif live_iso in ("fedora", "rhel"):
    RELEASEVER="--releasever=/"

excode = os.system(f"dnf {RELEASEVER} --installroot=/mnt install -y {packages}") # Ran without -c /etc/yum.repos.d/redhat.repo and  --releasever={RELEASE} --forcearch={ARCH}
# first I did 'dnf --installroot=/mnt install dnf' successfully
# then I did 'dnf --installroot=/mnt install kernel' which gave error: no enabled repo in /mnt/etc/yum.repos.d or /mnt/etc/yum/repos.d or /mnt/etc/distro.repos.d

### for whatever reason any subsequent dnf install command on chroot fails!!! <---- fix: sync_time() NOPE!
#Real solution: copy /etc/yum.repos.d/redhat.repo to /mnt/etc/yum.repos.d/redhat.repo

#   4. Update hostname, hosts, locales and timezone, hosts
os.system(f"echo {hostname} | tee /mnt/etc/hostname")
os.system(f"echo 127.0.0.1 {hostname} {distro} | tee -a /mnt/etc/hosts")
os.system("sudo chroot /mnt sudo localedef -v -c -i en_US -f UTF-8 en_US.UTF-8")
#os.system("sudo sed -i 's|^#en_US.UTF-8|en_US.UTF-8|g' /mnt/etc/locale.gen")
#os.system("sudo chroot /mnt sudo locale-gen")
os.system("echo 'LANG=en_US.UTF-8' | tee /mnt/etc/locale.conf")
os.system(f"ln -srf /mnt{tz} /mnt/etc/localtime")
os.system("chroot /mnt hwclock --systohc")

#then run post_bootstrap

#fstab
/dev/sda2    /boot   ext4    defaults,rw,relatime   0    2
/dev/sda1    /boot/efi   vfat    umask=0077   0    2

    if is_efi:
        os.system(f"echo 'UUID=\"{to_uuid(args[3])}\" /boot/efi vfat umask=0077 0 2' | sudo tee -a /mnt/etc/fstab")
        /dev/stratis/[STRATIS_SYMLINK] / xfs defaults 0 1
    #fstab
    #/dev/stratis/[STRATIS_SYMLINK] [MOUNT_POINT] xfs defaults,x-systemd.requires=stratis-fstab-setup@[POOL_UUID].service,x-systemd.after=stratis-fstab-setup@[POOL_UUID].service 0 2

    #/dev/stratis/[STRATIS_SYMLINK] [MOUNT_POINT] xfs defaults,x-systemd.requires=stratis-fstab-setup@[POOL_UUID].service,x-systemd.after=stratis-fstab-setup@[POOL_UUID].service,nofail 0 2

    # Create user and set password
    set_password("root")
    username = get_username()
    create_user(username, super_group)
    set_password(username)

#endof post_bootstrap section

#   5. Services (init, network, etc.)
os.system("sudo chroot /mnt systemctl enable NetworkManager")
#os.system("sudo chroot /mnt systemctl disable rpmdb-migrate") # https://fedoraproject.org/wiki/Changes/RelocateRPMToUsr NOT NEEDED for stratis I guess?!!

os.system('grep -qxF "[#]?GRUB_ENABLE_BLSCFG.*" /mnt/etc/default/grub && sudo sed -i "s/[#]?GRUB_ENABLE_BLSCFG.*$/GRUB_ENABLE_BLSCFG=true/" /mnt/etc/default/grub || \
           echo GRUB_ENABLE_BLSCFG="false" | sudo tee -a /mnt/etc/default/grub')

if is_efi: # This needs to go before grub_ash otherwise map.txt entry would be empty
    os.system(f"efibootmgr -c -L 'RHELstratis' -l '\\EFI\\redhat\\shim.efi'") ### REVIEW NOTE it creates redhat and not rhel which is distro_id!

# for /mnt/etc/default/grub
GRUB_ENABLE_BLSCFG="false"
GRUB_CMDLINE_LINUX="root=[STRATIS_FS_SYMLINK] stratis.rootfs.pool_uuid=[POOL_UUID]"
Should I change GRUB_CMDLINE_LINUX instead for these?

#Build Initramfs
sudo chroot /mnt sudo dracut -f /boot/initramfs-<kernelVersion>.img <kernelVersion>

chroot /mnt grub2-mkconfig /dev/sda -o /boot/grub2/grub.cfg

Then edit /mnt/boot/grub2/grub.cfg to remove redundatn old root=UUID from redhat entry

# CTRL+ALT+F6 and go back to Anaconda GUI







 # subscription-manager repos --disable fast-datapath-for-rhel-8-x86_64-rpms  
 # subscription-manager repos --enable fast-datapath-for-rhel-8-x86_64-rpms


anaconda --dirinstall=/mnt
