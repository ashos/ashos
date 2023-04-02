# ---------------------------- SPECIFIC FUNCTIONS ---------------------------- #

#   Noninteractive update
def auto_upgrade(snapshot):
    sync_time() # Required in virtualbox, otherwise error in package db update
    prepare(snapshot)
    excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} xbps-install -Syu")
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
    os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/var/cache/xbps/. /var/cache/xbps/{DEBUG}") ### REVIEW IS THIS NEEDED?

#   Fix signature invalid error
def fix_package_db(snapshot = "0"):
    return 0

#   Delete init system files (Systemd, OpenRC, etc.)
def init_system_clean(snapshot, FROM):
    if FROM == "prepare":
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-chr{snapshot}/var/service/*{DEBUG}")
    elif FROM == "deploy":
        os.system(f"rm -rf /var/lib/systemd/*{DEBUG}")
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-{snapshot}/var/service/*{DEBUG}")

#   Copy init system files (Systemd, OpenRC, etc.) to shared
def init_system_copy(snapshot, FROM):
    if FROM == "post_transactions":
        os.system(f"rm -rf /var/lib/systemd/*{DEBUG}")
        os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-{snapshot}/var/service/. /var/service/{DEBUG}")

#   Install atomic-operation
def install_package(snapshot, pkg):
    prepare(snapshot)
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} xbps-install -Iy {pkg}") ### TODO: --overwrite '/var/*'

#   Install atomic-operation in live snapshot
def install_package_live(tmp, pkg):
    #options = snapshot_config_get(tmp)
    return os.system(f"chroot /.snapshots/rootfs/snapshot-{tmp} xbps-install -Iy {pkg}{DEBUG}") ### TODO: --overwrite \\*

#   Refresh snapshot atomic-operation
def refresh_helper(snapshot):
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} xbps-install -Sy")

#   Show diff of packages between 2 snapshots TODO: make this function not depend on bash
def snapshot_diff(snap1, snap2):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap1}"):
        print(f"Snapshot {snap1} not found.")
    elif not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap2}"):
        print(f"Snapshot {snap2} not found.")
    else:
        os.system(f"bash -c \"diff <(ls /.snapshots/rootfs/snapshot-{snap1}/usr/share/ash/db/xbps) <(ls /.snapshots/rootfs/snapshot-{snap2}/usr/share/ash/db/xbps) | grep '^>\|^<' | sort\"")

#   Uninstall package(s)
def uninstall_package_helper(snapshot, pkg):
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} xbps-remove -R {pkg}")

#   Upgrade snapshot atomic-operation
def upgrade_helper(snapshot):
    prepare(snapshot) ### REVIEW tried it outside of this function in ashpk_core before aur_install and it works fine!
    return os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} xbps-install -Su") ###pacman -Syyu

# ---------------------------------------------------------------------------- #

#   Call main
if __name__ == "__main__":
    main()

