# ---------------------------- SPECIFIC FUNCTIONS ---------------------------- #

#   Check if AUR is setup right
def aur_check(snap):
    return os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}/usr/bin/paru")

#   Set up AUR in snapshot (if enabled, return true)
def aur_install(snapshot):
    options = snapshot_config_get(snapshot)
    aur = False
    if options["aur"] == 'True':
        aur = True
        if aur and not aur_check(snapshot):
            prepare(snapshot) ### REVIEW NEEDED? Being called twice!
            excode = aur_install_helper(snapshot)
            if excode:
                chr_delete(snapshot)
                print("F: Setting up AUR failed!")
                sys.exit(1) ### REVIEW changed from sys.exit()
            post_transactions(snapshot)
    return aur

#   Set up AUR in snapshot
def aur_install_helper(snap):
    required = ["sudo", "git", "base-devel"]
    excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} pacman -Sy --needed --noconfirm {' '.join(required)}")
    if excode:
        print("F: failed to install necessary packages to target!")
        chr_delete(snap)
        return str(excode)
    os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} useradd aur")
    os.system(f"chmod +w /.snapshots/rootfs/snapshot-chr{snap}/etc/sudoers")
    os.system(f"echo 'aur ALL=(ALL:ALL) NOPASSWD: ALL' >> /.snapshots/rootfs/snapshot-chr{snap}/etc/sudoers")
    os.system(f"chmod -w /.snapshots/rootfs/snapshot-chr{snap}/etc/sudoers")
    os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} mkdir -p /home/aur")
    os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} chown -R aur /home/aur{DEBUG}")
    # TODO: more checking here
    excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} su aur -c 'rm -rf /home/aur/paru-bin && cd /home/aur && git clone https://aur.archlinux.org/paru-bin.git'{DEBUG}")
    if excode:
        print("F: failed to download paru-bin")
        chr_delete(snap)
        return excode
    excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} su aur -c 'cd /home/aur/paru-bin && makepkg -si'")
    if excode:
        print("F: failed installing paru-bin")
        chr_delete(snap)
        return excode
    return 0

#   Set up AUR support for live snapshot
def aur_install_live_helper(snap):
    print("setting up AUR...")
    excode = os.system(f"chroot /.snapshots/rootfs/snapshot-{snap} pacman -S --noconfirm --needed sudo git base-devel{DEBUG}")
    if excode:
        return excode
    os.system(f"chroot /.snapshots/rootfs/snapshot-{snap} useradd aur")
    os.system(f"chmod +w /.snapshots/rootfs/snapshot-{snap}/etc/sudoers")
    os.system(f"echo 'aur ALL=(ALL:ALL) NOPASSWD: ALL' >> /.snapshots/rootfs/snapshot-{snap}/etc/sudoers")
    os.system(f"chmod -w /.snapshots/rootfs/snapshot-{snap}/etc/sudoers")
    os.system(f"chroot /.snapshots/rootfs/snapshot-{snap} mkdir -p /home/aur")
    os.system(f"chroot /.snapshots/rootfs/snapshot-{snap} chown -R aur /home/aur{DEBUG}")
    # TODO: no error checking here
    excode = os.system(f"chroot /.snapshots/rootfs/snapshot-{snap} su aur -c 'rm -rf /home/aur/paru-bin && cd /home/aur && git clone https://aur.archlinux.org/paru-bin.git'{DEBUG}")
    if excode:
        print("F: failed to download paru-bin")
        return excode
    excode = os.system(f"chroot /.snapshots/rootfs/snapshot-{snap} su aur -c 'cd /home/aur/paru-bin && makepkg --noconfirm -si{DEBUG}'")
    if excode:
        print("F: failed installing paru-bin")
        return excode
    return 0

#   Noninteractive update
def auto_upgrade(snapshot):
    sync_time() # Required in virtualbox, otherwise error in package db update
    aur = aur_install(snapshot)
    prepare(snapshot)
    if not aur:
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman --noconfirm -Syyu")
    else:
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} su aur -c 'paru --noconfirm -Syy'")
    if excode == 0:
        post_transactions(snapshot)
        os.system("echo 0 > /.snapshots/ash/upstate")
        os.system("echo $(date) >> /.snapshots/ash/upstate")
    else:
        chr_delete(snapshot)
        os.system("echo 1 > /.snapshots/ash/upstate")
        os.system("echo $(date) >> /.snapshots/ash/upstate")

#   Copy cache of downloaded packages to shared
def cache_copy(snapshot, FROM):
    os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/var/cache/pacman/pkg/. /var/cache/pacman/pkg/{DEBUG}")
    #if aur_enabled:
    #    os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/var/cache/pacman/aur/. /var/cache/pacman/aur/{DEBUG}")

#   Fix signature invalid error
def fix_package_db(snapshot = "0"):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot fix package manager database as snapshot {snapshot} doesn't exist.")
        return
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"):
        print(f"F: Snapshot {snapshot} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {snapshot}'.")
        return
    elif snapshot == "0":
        P = "" ### I think this is wrong. It should be check if snapshot = current-deployed-snapshot, then this.
    else:
        P = f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} "
    try:
        if check_mutability(snapshot):
            flip = False # Snapshot is mutable so do not make it immutable after fixdb is done
        else:
            immutability_disable(snapshot)
            flip = True
        prepare(snapshot)
        os.system(f"{P}rm -rf /etc/pacman.d/gnupg /home/me/.gnupg") ### $HOME vs /root NEEDS fixing # If folder not present and subprocess.run is used, throws error and stops
        os.system(f"{P}rm -r /var/lib/pacman/db.lck")
        os.system(f"{P}pacman -Syy")
        os.system(f"{P}gpg --refresh-keys")
        os.system(f"{P}killall gpg-agent")
        os.system(f"{P}pacman-key --init")
        os.system(f"{P}pacman-key --populate archlinux")
        os.system(f"{P}pacman-key --populate cachyos")
        os.system(f"{P}pacman -Syvv --noconfirm archlinux-keyring cachyos-keyring") ### REVIEW NEEDED? (maybe)
        post_transactions(snapshot)
        if flip:
            immutability_enable(snapshot)
        print(f"Snapshot {snapshot}'s package manager database fixed successfully.")
    except subprocess.CalledProcessError:
        chr_delete(snapshot)
        print("F: Fixing package manager database failed.")

#   Delete init system files (Systemd, OpenRC, etc.)
def init_system_clean(snapshot, FROM):
    if FROM == "prepare":
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-chr{snapshot}/var/lib/systemd/*{DEBUG}")
    elif FROM == "deploy":
        os.system(f"rm -rf /var/lib/systemd/*{DEBUG}")
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-{snapshot}/var/lib/systemd/*{DEBUG}")

#   Copy init system files (Systemd, OpenRC, etc.) to shared
def init_system_copy(snapshot, FROM):
    if FROM == "post_transactions":
        os.system(f"rm -rf /var/lib/systemd/*{DEBUG}")
        os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-{snapshot}/var/lib/systemd/. /var/lib/systemd/{DEBUG}")

#   Install atomic-operation
def install_package(snapshot, pkg):
    try:
      # This extra pacman check is to avoid unwantedly triggering AUR if package is official but user answers no to prompt
        subprocess.check_output(f"pacman -Si {pkg}", shell=True) # --sysroot
    except subprocess.CalledProcessError:
        aur = aur_install(snapshot) ### TODO: do a paru -Si {pkg} check to avoid setup_aur if package already installed!
        prepare(snapshot)
        if aur:
            return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} su aur -c \"paru -S {pkg} --needed --overwrite '/var/*'\"")
        else:
            print("F: AUR is not enabled!")
            return 1
    else:
        prepare(snapshot)
        return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman -S {pkg} --needed --overwrite '/var/*'")

#   Install atomic-operation in live snapshot
def install_package_live(snapshot, tmp, pkg):
    try:
      # This extra pacman check is to avoid unwantedly triggering AUR if package is official but user answers no to prompt
        subprocess.check_output(f"pacman -Si {pkg}", shell=True) # --sysroot
    except subprocess.CalledProcessError:
        options = snapshot_config_get(tmp)
        if options["aur"] == "True":
            aur_in_tmp = True
        else:
            aur_in_tmp = False
        if aur_in_tmp and not aur_check(tmp):
            excode = aur_install_live_helper(tmp)
            if excode:
                os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}")
                os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")
                print("F: Live install failed!") # Before: Live install failed and changes discarded
                return excode
        if snapshot_config_get(snapshot)["aur"] == "True":
            aur_in_destination_snapshot = True
        else:
            aur_in_destination_snapshot = False
            print("F: AUR not enabled in target snapshot!") ### REVIEW
        ### REVIEW - error checking, handle the situation better altogether
        if aur_in_destination_snapshot and not aur_in_tmp:
            print("F: AUR is not enabled in current live snapshot, but is enabled in target.\nEnable AUR for live snapshot? (y/n)")
            reply = input("> ")
            while reply.casefold() != "y" and reply.casefold() != "n":
                print("Please enter 'y' or 'n':")
                reply = input("> ")
            if reply == "y":
                if not aur_check(tmp):
                    excode = aur_install_live_helper(tmp)
                    if excode:
                        os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}")
                        os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")
                        print("F: Live install failed!") # Before: Live install failed and changes discarded
                        return excode # i.e. aur = True
            else:
                print("F: Not enabling AUR for live snapshot!")
                excode = 1 # i.e. aur = False
    else:
        #ash_chroot_mounts(tmp) ### REVIEW If issues to have this in ashpk_core.py, uncomment this
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-{tmp} pacman -Sy --overwrite '*' --noconfirm {pkg}{DEBUG}") ### REVIEW Maybe just do this in try section and remove else section!
    return excode

#   Get list of packages installed in a snapshot
def pkg_list(CHR, snap):
    return subprocess.check_output(f"chroot /.snapshots/rootfs/snapshot-{CHR}{snap} pacman -Qq", encoding='utf-8', shell=True).strip().split("\n")

#   Refresh snapshot atomic-operation
def refresh_helper(snapshot):
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman -Syy")

#   Show diff of packages between two snapshots TODO: make this function not depend on bash
def snapshot_diff(snap1, snap2):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap1}"):
        print(f"Snapshot {snap1} not found.")
    elif not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap2}"):
        print(f"Snapshot {snap2} not found.")
    else:
        os.system(f"bash -c \"diff <(ls /.snapshots/rootfs/snapshot-{snap1}/usr/share/ash/db/local) <(ls /.snapshots/rootfs/snapshot-{snap2}/usr/share/ash/db/local) | grep '^>\|^<' | sort\"")

#   Uninstall package(s) atomic-operation
def uninstall_package_helper(snapshot, pkg):
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman --noconfirm -Rns {pkg}")

#   Upgrade snapshot atomic-operation
def upgrade_helper(snapshot):
    aur = aur_install(snapshot)
    prepare(snapshot) ### REVIEW tried it outside of this function in ashpk_core before aur_install and it works fine!
    if not aur:
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman -Syyu")
    else:
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} su aur -c 'paru -Syyu'")
    return excode

# ---------------------------------------------------------------------------- #

#   Call main
if __name__ == "__main__":
    main()

