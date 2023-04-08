# ---------------------------- SPECIFIC FUNCTIONS ---------------------------- #

#   Noninteractive update
def auto_upgrade(snapshot):
    sync_time() # Required in virtualbox, otherwise error in package db update
    prepare(snapshot)
    excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apk update") ### REVIEW --noconfirm -Syyu
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
    os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/var/cache/apk/pkg/. /var/cache/apk/pkg/{DEBUG}") ### REVIEW IS THIS NEEDED?
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
        os.system(f"{P}pacman -Syvv --noconfirm archlinux-keyring") ### REVIEW NEEDED? (maybe)
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
        subprocess.check_output(f"apk add --force-overwrite -i {pkg}", shell=True) # --sysroot ### REVIEW '/var/*'
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
        subprocess.check_output(f"apk add --force-overwrite {pkg}", shell=True) # --sysroot # -S --overwrite \\* --noconfirm
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
    excode = str(os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apk update -i")) ### REVIEW -Syy

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
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apk del --purge {pkg}") ### -Rns REVIEW

#   Upgrade snapshot atomic-operation
def upgrade_helper(snapshot):
    prepare(snapshot) ### REVIEW tried it outside of this function in ashpk_core before aur_install and it works fine!
    return  os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apk update -i") ### REVIEW "-Syyu" # Default upgrade behaviour is now "safe" update, meaning failed updates get fully discarded

# ---------------------------------------------------------------------------- #

#   Call main
if __name__ == "__main__":
    main()

