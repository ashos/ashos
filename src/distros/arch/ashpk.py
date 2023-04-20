# ---------------------------- SPECIFIC FUNCTIONS ---------------------------- #

#   Check if AUR is setup right
def aur_check(snap):
    return os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}/usr/bin/paru")

#   Set up AUR in snapshot (if enabled, return true)
def aur_install(snap, skip_prep=False, skip_post=False):
    options = snapshot_config_get(snap)
    aur = False
    if options["aur"] == 'True':
        aur = True
        if aur and not aur_check(snap):
            if not skip_prep:
                prepare(snap) ### REVIEW NEEDED? Being called twice!
            excode = aur_install_helper(snap)
            if excode:
                chr_delete(snap)
                print("F: Setting up AUR failed!")
                sys.exit(1) ### REVIEW changed from sys.exit()
            if not skip_post:
                post_transactions(snap)
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
def auto_upgrade(snap):
    sync_time() # Required in virtualbox, otherwise error in package db update
#    aur = aur_install(snap) ### OLD
    prepare(snap)
    aur = aur_install(snap, True, True) # skip both prepare and post
    if not aur:
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} pacman --noconfirm -Syyu")
    else:
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} su aur -c 'paru --noconfirm -Syy'")
    if excode == 0:
        post_transactions(snap)
        os.system("echo 0 > /.snapshots/ash/upstate")
        os.system("echo $(date) >> /.snapshots/ash/upstate")
    else:
        chr_delete(snap)
        os.system("echo 1 > /.snapshots/ash/upstate")
        os.system("echo $(date) >> /.snapshots/ash/upstate")

#   Copy cache of downloaded packages to shared
def cache_copy(snap, FROM):
    os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snap}/var/cache/pacman/pkg/. /var/cache/pacman/pkg/{DEBUG}")
    #if aur_enabled:
    #    os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snap}/var/cache/pacman/aur/. /var/cache/pacman/aur/{DEBUG}")

#   Fix signature invalid error
def fix_package_db(snap = 0):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot fix package manager database as snapshot {snap} doesn't exist.")
        return
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}"):
        print(f"F: Snapshot {snap} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {snap}'.")
        return
    elif snap == 0:
        P = "" ### I think this is wrong. It should be check if snapshot = current-deployed-snapshot, then this.
    else:
        P = f"chroot /.snapshots/rootfs/snapshot-chr{snap} "
    try:
        if check_mutability(snap):
            flip = False # Snapshot is mutable so do not make it immutable after fixdb is done
        else:
            immutability_disable(snap)
            flip = True
        prepare(snap)
        os.system(f"{P}rm -rf /etc/pacman.d/gnupg $HOME/.gnupg") ### $HOME vs /root NEEDS fixing # If folder not present and subprocess.run is used, throws error and stops
        os.system(f"{P}rm -r /var/lib/pacman/db.lck")
        os.system(f"{P}pacman -Syy")
        os.system(f"{P}gpg --refresh-keys")
        os.system(f"{P}killall gpg-agent")
        os.system(f"{P}pacman-key --init")
        os.system(f"{P}pacman-key --populate archlinux")
        os.system(f"{P}pacman -Syvv --noconfirm archlinux-keyring") ### REVIEW NEEDED? (maybe)
        post_transactions(snap)
        if flip:
            immutability_enable(snap)
        print(f"Snapshot {snap}'s package manager database fixed successfully.")
    except subprocess.CalledProcessError:
        chr_delete(snap)
        print("F: Fixing package manager database failed.")

#   Delete init system files (Systemd, OpenRC, etc.)
def init_system_clean(snap, FROM):
    if FROM == "prepare":
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-chr{snap}/var/lib/systemd/*{DEBUG}")
    elif FROM == "deploy":
        os.system(f"rm -rf /var/lib/systemd/*{DEBUG}")
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-{snap}/var/lib/systemd/*{DEBUG}")

#   Copy init system files (Systemd, OpenRC, etc.) to shared
def init_system_copy(snap, FROM):
    if FROM == "post_transactions":
        os.system(f"rm -rf /var/lib/systemd/*{DEBUG}")
        os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-{snap}/var/lib/systemd/. /var/lib/systemd/{DEBUG}")

#   Install atomic-operation
def install_package(pkg, snap):
    prepare(snap)
    excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} pacman -S {pkg} --needed --overwrite '/var/*'")
    if excode:
        aur = aur_install(snap, True, True) ### TODO: do a paru -Si {pkg} check to avoid setup_aur if package already installed!
        if aur:
            return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} su aur -c \"paru -S {pkg} --needed --overwrite '/var/*'\"")
        else:
            print("F: AUR is not enabled!")
            if yes_no("Enable AUR?"):
                print("Opening snapshot's config file... Please change AUR to True")
                snapshot_config_edit(snap, True, False) ### TODO move to core.py ? Run prepare but skip post transaction (Optimize code) Update: post_tran need to run too
                aur = aur_install(snap, True, False)
                return aur
            else:
                return 1
    else:
        return 0

#   Install atomic-operation
def install_package_old(pkg, snap):
    try:
      # This extra pacman check is to avoid unwantedly triggering AUR if package is official but user answers no to prompt
        ### TODO IMPORTANT this doesn't work for a package group e.g. "lxqt" errors out even though it's not in AUR, which makes following code malfunction!
        subprocess.check_output(f"pacman -Si {pkg}", shell=True, stderr=subprocess.PIPE) # --sysroot ### do not print if pkg not found
    except subprocess.CalledProcessError:
        aur = aur_install(snap) ### TODO: do a paru -Si {pkg} check to avoid setup_aur if package already installed!
        prepare(snap)
        if aur:
            return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} su aur -c \"paru -S {pkg} --needed --overwrite '/var/*'\"")
        else:
            print("F: AUR is not enabled!")
            return 1
    else:
        prepare(snap)
        return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} pacman -S {pkg} --needed --overwrite '/var/*'")

#   Install atomic-operation in live snapshot
def install_package_live(pkg, snap, tmp):
    excode = 1 ### REVIEW
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
                print("F: Live installation failed!") # Before: Live install failed and changes discarded
                return excode
        if snapshot_config_get(snap)["aur"] == "True":
            aur_in_target_snap = True
        else:
            aur_in_target_snap = False
            print("F: AUR not enabled in target snapshot!") ### REVIEW
        ### REVIEW - error checking, handle the situation better altogether
        if aur_in_target_snap and not aur_in_tmp:
            print("F: AUR is not enabled in current live snapshot, but is enabled in target.")
            if yes_no("Enable AUR for live snapshot?"):
                if aur_install_live_helper(tmp) and not aur_check(tmp):
                    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}")
                    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")
                    print("F: Live installation failed!") # Before: Live install failed and changes discarded
                    return excode # i.e. aur = True
            else:
                print("F: Not enabling AUR for live snapshot!")
                excode = 1 # i.e. aur = False
    else:
        #ash_chroot_mounts(tmp) ### REVIEW If issues to have this in ashpk_core.py, uncomment this
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-{tmp} pacman -Sy --overwrite '*' --noconfirm {pkg}{DEBUG}") ### REVIEW Maybe just do this in try section and remove else section!
    return excode

#   Get list of packages installed in a snapshot
def pkg_list(snap, CHR=""):
    return subprocess.check_output(f"chroot /.snapshots/rootfs/snapshot-{CHR}{snap} pacman -Qq", encoding='utf-8', shell=True).strip().split("\n")

#   Distro-specific function to setup snapshot based on preset parameters
def presets_helper(prof_cp, snap): ### TODO before: prof_section
    if prof_cp.has_option('presets', 'enable_aur'):
###        if aur is not already set to True: ### TODO IMPORTANT, concat generic preset and distro preset and paste it in /.snapshots/etc
        print("Opening snapshot's config file... Please change AUR to True")
        snapshot_config_edit(snap, False, False) ### TODO move to core.py ? Run prepare but skip post transaction (Optimize code) Update: post_tran need to run too
        aur_install(snap, False, False) ### Skip prepare, but run post transaction (Optimize code) ### update: because of last step, had to run prepare again too!

#   Refresh snapshot atomic-operation
def refresh_helper(snap):
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} pacman -Syy")

#   Show diff of packages between two snapshots TODO: make this function not depend on bash
def snapshot_diff(snap1, snap2):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap1}"):
        print(f"Snapshot {snap1} not found.")
    elif not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap2}"):
        print(f"Snapshot {snap2} not found.")
    else:
        os.system(f"bash -c \"diff <(ls /.snapshots/rootfs/snapshot-{snap1}/usr/share/ash/db/local) <(ls /.snapshots/rootfs/snapshot-{snap2}/usr/share/ash/db/local) | grep '^>\\|^<' | sort\"") ### REVIEW

#   Uninstall package(s) atomic-operation
def uninstall_package_helper(pkg, snap):
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} pacman --noconfirm -Rns {pkg}")

#   Upgrade snapshot atomic-operation
def upgrade_helper(snap):
    aur = aur_install(snap)
    prepare(snap) ### REVIEW tried it outside of this function in ashpk_core before aur_install and it works fine!
    if not aur:
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} pacman -Syyu")
    else:
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} su aur -c 'paru -Syyu'")
    return excode

# ---------------------------------------------------------------------------- #

#   Call main
if __name__ == "__main__":
    main()

