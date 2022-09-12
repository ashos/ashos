# ---------------------------- SPECIFIC FUNCTIONS ---------------------------- #

#   Noninteractive update
def auto_upgrade(snapshot):
    sync_time() # Required in virtualbox, otherwise error in package db update
    prepare(snapshot)
    excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apt-get update -y")
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
    os.system(f"cp -r -n --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/var/cache/apt/* /var/cache/apt/ >/dev/null 2>&1")

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
        #os.system(f"{P}pacman -S --noconfirm archlinux-keyring")
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
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-chr{snapshot}/var/lib/systemd/* >/dev/null 2>&1")
    elif FROM == "deploy":
        os.system("rm -rf /var/lib/systemd/* >/dev/null 2>&1")
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-{snapshot}/var/lib/systemd/* >/dev/null 2>&1")

#   Copy init system files (Systemd, OpenRC, etc.) to shared
def init_system_copy(snapshot, FROM):
    if FROM == "post_transactions":
        os.system("rm -rf /var/lib/systemd/* >/dev/null 2>&1")
        os.system(f"cp --reflink=auto -r /.snapshots/rootfs/snapshot-{snapshot}/var/lib/systemd/* /var/lib/systemd/ >/dev/null 2>&1")

#   Install atomic-operation
def install_package(snapshot, pkg):
    #excode = str(os.system(f'chroot /.snapshots/rootfs/snapshot-chr{snapshot} apt-get -o Dpkg::Options::="--force-overwrite" install -y {pkg}'))
    try:
        subprocess.run(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apt-get install -f -y {pkg}", shell=True, check=True) ### --overwrite '/var/*'
        return 0
    except subprocess.CalledProcessError as e:
        print(f"F: Install failed and changes discarded: {e.output}.")
        return 1

#   Install atomic-operation in live snapshot
def install_package_live(tmp, pkg):
    try:
        subprocess.run(f"chroot /.snapshots/rootfs/snapshot-{tmp} apt-get install -y {pkg} >/dev/null 2>&1", shell=True, check=True) ### --overwrite \\*
        print("Done!")
        return 0
    except subprocess.CalledProcessError as e:
        print(f"F: Live install failed and changes discarded: {e.output}.")
        return 1

#   Refresh snapshot
def refresh(snapshot):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot refresh as snapshot {snapshot} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"):
        print(f"F: Snapshot {snapshot} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {snapshot}'.")
    elif snapshot == "0":
        print("F: Changing base snapshot is not allowed.")
    else:
        prepare(snapshot)
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman -Syy")
        if excode == 0:
            post_transactions(snapshot)
            print(f"Snapshot {snapshot} refreshed successfully.")
        else:
            chr_delete(snapshot)
            print("F: Refresh failed and changes discarded.")

#   Show diff of packages between 2 snapshots TODO: make this function not depend on bash
def snapshot_diff(snap1, snap2):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap1}"):
        print(f"Snapshot {snap1} not found.")
    elif not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap2}"):
        print(f"Snapshot {snap2} not found.")
    else:
        os.system(f"diff -qrly --no-dereference /.snapshots/rootfs/snapshot-{snap1}/usr/share/ash/db/dpkg/info /.snapshots/rootfs/snapshot-{snap2}/usr/share/ash/db/dpkg/info")

#   Uninstall package(s)
def uninstall_package(snapshot, pkg):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot remove as snapshot {snapshot} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"):
        print(f"F: Snapshot {snapshot} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {snapshot}'.")
    elif snapshot == "0":
        print("F: Changing base snapshot is not allowed.")
    else:
        prepare(snapshot)
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apt-get remove {pkg}")
        if excode == 0:
            post_transactions(snapshot)
            print(f"Package {pkg} removed from snapshot {snapshot} successfully.")
        else:
            chr_delete(snapshot)
            print("F: Remove failed and changes discarded.")

#   Upgrade snapshot
def upgrade(snapshot, baseup=False):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot upgrade as snapshot {snapshot} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"):
        print(f"F: Snapshot {snapshot} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {snapshot}'.")
    elif snapshot == "0" and not baseup:
        print("F: Changing base snapshot is not allowed.")
    else:
        prepare(snapshot)
      # Default upgrade behaviour is now "safe" update, meaning failed updates get fully discarded
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} apt-get update")
        if excode == 0:
            post_transactions(snapshot)
            print(f"Snapshot {snapshot} upgraded successfully.")
        else:
            chr_delete(snapshot)
            print("F: Upgrade failed and changes discarded.")

# ---------------------------------------------------------------------------- #

#   Call main
if __name__ == "__main__":
    main()

