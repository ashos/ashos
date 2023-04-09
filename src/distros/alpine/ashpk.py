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
    os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/var/cache/apk/. /var/cache/apk/{DEBUG}") ### REVIEW IS THIS NEEDED?

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
        os.system(f"{P}apk -sv fix") ### REVIEW NEEDED
        post_transactions(snapshot)
        if flip:
            immutability_enable(snapshot)
        print(f"Snapshot {snapshot}'s package manager database fixed successfully.")
    except subprocess.CalledProcessError:
        chr_delete(snapshot)
        print("F: Fixing package manager database failed.")

#   Delete init system files (Systemd, OpenRC, etc.)
def init_system_clean(snapshot, FROM):
    return ### TODO
#    if FROM == "prepare":
#        os.system(f"rm -rf /.snapshots/rootfs/snapshot-chr{snapshot}/var/lib/systemd/*{DEBUG}")
#    elif FROM == "deploy":
#        os.system(f"rm -rf /var/lib/systemd/*{DEBUG}")
#        os.system(f"rm -rf /.snapshots/rootfs/snapshot-{snapshot}/var/lib/systemd/*{DEBUG}")

#   Copy init system files (Systemd, OpenRC, etc.) to shared
def init_system_copy(snapshot, FROM):
    return ### TODO
#    if FROM == "post_transactions":
#        os.system(f"rm -rf /var/lib/systemd/*{DEBUG}")
#        os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-{snapshot}/var/lib/systemd/. /var/lib/systemd/{DEBUG}")

#   Install atomic-operation
def install_package(snapshot, pkg):
    prepare(snapshot)
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apk add --force-overwrite -i {pkg}") # --root ### REVIEW --needed --overwrite '/var/*

#   Install atomic-operation in live snapshot
def install_package_live(snapshot, tmp, pkg): ### TODO remove 'snapshot' as not used
    return os.system(f"chroot /.snapshots/rootfs/snapshot-{tmp} apk add --force-overwrite {pkg}{DEBUG}") # --root # -Sy --overwrite '*' --noconfirm

#   Get list of packages installed in a snapshot
def pkg_list(CHR, snap):
    return subprocess.check_output(f"chroot /.snapshots/rootfs/snapshot-{CHR}{snap} apk list -i", encoding='utf-8', shell=True).strip().split("\n")

#   Refresh snapshot atomic-operation
def refresh_helper(snapshot):
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apk update -i") ### REVIEW -Syy

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
    prepare(snapshot)
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apk update -i") ### REVIEW "-Syyu" # Default upgrade behaviour is now "safe" update, meaning failed updates get fully discarded

# ---------------------------------------------------------------------------- #

#   Call main
if __name__ == "__main__":
    main()

