use std::path::Path;
use std::process::{Command, ExitStatus};
use crate::{check_mutability, chr_delete, immutability_disable, immutability_enable, prepare, post_transactions,
            remove_dir_content, snapshot_config_get, sync_time};

// Check if AUR is setup right
pub fn aur_check(snapshot: &str) -> bool {
    let options = snapshot_config_get(snapshot);
    if options["aur"] == "True" {
        let aur = true;
        return aur;
    } else if options["aur"] == "False" {
        let aur = false;
        return aur;
    } else {
        panic!("Please insert valid value for aur in /.snapshots/etc/etc-{}/ash.conf", snapshot);
    }
}

// Noninteractive update
pub fn auto_upgrade(snapshot: &str) {
    sync_time(); // Required in virtualbox, otherwise error in package db update
    prepare(snapshot).unwrap();
    if !aur_check(snapshot) {
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(["pacman", "--noconfirm", "-Syyu"]).status().unwrap();
        if excode.success() {
            post_transactions(snapshot).unwrap();
            Command::new("echo").args(["0", ">"]).arg("/.snapshots/ash/upstate").status().unwrap();
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").status().unwrap();
        } else {
            chr_delete(snapshot).unwrap();
            Command::new("echo").args(["1", ">"]).arg("/.snapshots/ash/upstate").status().unwrap();
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").status().unwrap();
        }
    } else {
        let excode = Command::new("sh").arg("-c")
                                       .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} su aur -c 'paru --noconfirm -Syy'", snapshot))
                                       .status().unwrap();
        if excode.success() {
            post_transactions(snapshot).unwrap();
            Command::new("echo").args(["0", ">"]).arg("/.snapshots/ash/upstate").status().unwrap();
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").status().unwrap();
        } else {
            chr_delete(snapshot).unwrap();
            Command::new("echo").args(["1", ">"]).arg("/.snapshots/ash/upstate").status().unwrap();
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").status().unwrap();
        }
    }
}

// Copy cache of downloaded packages to shared
pub fn cache_copy(snapshot: &str) -> std::io::Result<()> {
    Command::new("cp").args(["-n", "-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/cache/pacman/pkg", snapshot))
                      .arg("/var/cache/pacman/")
                      .output().unwrap();
    Ok(())
}

// Fix signature invalid error
pub fn fix_package_db(snapshot: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot fix package man database as snapshot {} doesn't exist.", snapshot);
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot);
    } else {
        let p = if snapshot == "0" {//REVIEW // I think this is wrong. It should be check if snapshot = current-deployed-snapshot, then this.
            "".to_string()
        } else {
            format!("chroot /.snapshots/rootfs/snapshot-chr{}", snapshot)
        };
        let flip = if check_mutability(snapshot) { // Snapshot is mutable so do not make it immutable after fixdb is done.
            false
        } else {
            immutability_disable(snapshot);
            true
        };
        prepare(snapshot).unwrap();
        if flip {
            immutability_enable(snapshot);
        }
        // this is ugly! //REVIEW
        let home_dir = std::env::var_os("HOME").unwrap();
        let excode1 = Command::new("sh").arg("-c").arg(format!("{} rm -rf /etc/pacman.d/gnupg {}/.gnupg", p,home_dir.to_str().unwrap())).status().unwrap();
        let excode2 = Command::new("sh").arg("-c").arg(format!("{} rm -r /var/lib/pacman/sync/*", p)).status().unwrap();
        let excode3 = Command::new("sh").arg("-c").arg(format!("{} pacman -Syy", p)).status().unwrap();
        let excode4 = Command::new("sh").arg("-c").arg(format!("{} gpg --refresh-keys", p)).status().unwrap();
        let excode5 = Command::new("sh").arg("-c").arg(format!("{} killall gpg-agent", p)).status().unwrap();
        let excode6 = Command::new("sh").arg("-c").arg(format!("{} pacman-key --init", p)).status().unwrap();
        let excode7 = Command::new("sh").arg("-c").arg(format!("{} pacman-key --populate archlinux", p)).status().unwrap();
        let excode8 = Command::new("sh").arg("-c").arg(format!("{} pacman -Syvv --noconfirm archlinux-keyring", p)).status().unwrap(); // REVIEW NEEDED? (maybe)
        post_transactions(snapshot).unwrap();
        if excode1.success() && excode2.success() && excode3.success() && excode4.success()
            && excode5.success() && excode6.success() && excode7.success() && excode8.success() {
            println!("Snapshot {}'s package manager database fixed successfully.", snapshot);
        } else {
            chr_delete(snapshot).unwrap();
            println!("Fixing package manager database failed.");
        }
    }
}

// Delete init system files (Systemd, OpenRC, etc.)
pub fn init_system_clean(snapshot: &str, from: &str) -> std::io::Result<()> {
    if from == "prepare" {
        remove_dir_content(format!("/.snapshots/rootfs/snapshot-chr{}/var/lib/systemd/", snapshot).as_str())?;
    } else if from == "deploy" {
        remove_dir_content("/var/lib/systemd/")?;
        remove_dir_content(format!("/.snapshots/rootfs/snapshot-{}/var/lib/systemd/", snapshot).as_str())?;
    }
    Ok(())
}

// Copy init system files (Systemd, OpenRC, etc.) to shared
pub fn init_system_copy(snapshot: &str, from: &str) -> std::io::Result<()> {
    if from == "post_transactions" {
        remove_dir_content("/var/lib/systemd/").unwrap();
        Command::new("cp").args(["-r", "--reflink=auto",])
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/var/lib/systemd/", snapshot))
                          .arg("/var/lib/systemd/")
                          .output().unwrap();
    }
    Ok(())
}

// Install atomic-operation
pub fn install_package(snapshot:&str, pkg: &str) -> i32 {
    // This extra pacman check is to avoid unwantedly triggering AUR if package is official but user answers no to prompt
    let excode = Command::new("pacman").arg("-Si")
                                       .arg(format!("{}", pkg))
                                       .status().unwrap(); // --sysroot
    if !excode.success() {
        prepare(snapshot).unwrap();
        if aur_check(snapshot) {
            let excode = Command::new("sh")
                .arg("-c")
                .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} su aur -c \'paru -S {} --needed --overwrite '/var/*''\'", snapshot,pkg))
                .status().unwrap();
            if excode.success() {
                return 0;
            } else {
                return 1;
            }
        } else {
            eprintln!("AUR is not enabled!");
            return 1;
        }
    } else {
        prepare(snapshot).unwrap();
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(["pacman", "-S"])
                                           .arg(format!("{}", pkg))
                                           .args(["--needed", "--overwrite", "'/var/*'"])
                                           .status().unwrap();
        if excode.success() {
            return 0;
        } else {
            return 1;
        }
    }
}

// Install atomic-operation in live snapshot
pub fn install_package_live(snapshot: &str, tmp: &str, pkg: &str) -> ExitStatus {
    let excode = Command::new("pacman").arg("-Si")
                                       .arg(format!("{}", pkg))
                                       .output().unwrap(); // --sysroot
    if excode.status.success() {
        let excode = Command::new("sh")
            .arg("-c")
            .arg(format!("chroot /.snapshots/rootfs/snapshot-{} pacman -Sy --overwrite '*' --noconfirm {}", tmp,pkg))
            .status().unwrap();
        return excode;
    } else {
        if aur_check(snapshot) {
            let excode = Command::new("sh")
                .arg("-c")
                .arg(format!("chroot /.snapshots/rootfs/snapshot-{} su aur -c 'paru -Sy --overwrite '*' --noconfirm {}'", tmp,pkg))
                .status().unwrap();
            return excode;
        } else {
            eprint!("AUR is not enabled!");
            return excode.status;
        }
    }
}

// Get list of packages installed in a snapshot
pub fn pkg_list(chr: &str, snap: &str) -> Vec<String> {
    let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}{}", chr,snap))
                          .args(["pacman", "-Qq"])
                          .output().unwrap();
    let stdout = String::from_utf8_lossy(&excode.stdout).trim().to_string();
    stdout.split('\n').map(|s| s.to_string()).collect()
}

// Refresh snapshot atomic-operation
pub fn refresh_helper(snapshot: &str) -> ExitStatus {
    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          .args(["pacman", "-Syy"]).status().unwrap()
}

// Uninstall package(s) atomic-operation
pub fn uninstall_package_helper(snapshot: &str, pkg: &str) -> ExitStatus {
    let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                       .args(["pacman", "--noconfirm", "-Rns"])
                                       .arg(format!("{}", pkg)).status().unwrap();
    excode
}

// Upgrade snapshot atomic-operation
pub fn upgrade_helper(snapshot: &str) -> ExitStatus {
    prepare(snapshot).unwrap();
    if !aur_check(snapshot) {
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(["pacman", "-Syyu"])
                                           .status().unwrap();
        excode
    } else {
        let excode = Command::new("sh").arg("-c")
                                       .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} su aur -c 'paru -Syyu", snapshot))
                                       .status().unwrap();
        excode
    }
}
