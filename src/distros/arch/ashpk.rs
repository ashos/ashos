use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, ExitStatus};
use crate::{check_mutability, chr_delete, get_tmp, immutability_disable, immutability_enable, prepare, post_transactions,
            snapshot_config_get, sync_time, write_desc};

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
    prepare(snapshot);
    if !aur_check(snapshot) {
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(["pacman", "--noconfirm", "-Syyu"]).status().unwrap();
        if excode.success() {
            post_transactions(snapshot);
            Command::new("echo").args(["0", ">"]).arg("/.snapshots/ash/upstate").status().unwrap();
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").status().unwrap();
        } else {
            chr_delete(snapshot);
            Command::new("echo").args(["1", ">"]).arg("/.snapshots/ash/upstate").status().unwrap();
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").status().unwrap();
        }
    } else {
        let excode = Command::new("sh").arg("-c")
                                       .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} su aur -c 'paru --noconfirm -Syy'", snapshot))
                                       .status().unwrap();
        if excode.success() {
            post_transactions(snapshot);
            Command::new("echo").args(["0", ">"]).arg("/.snapshots/ash/upstate").status().unwrap();
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").status().unwrap();
        } else {
            chr_delete(snapshot);
            Command::new("echo").args(["1", ">"]).arg("/.snapshots/ash/upstate").status().unwrap();
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").status().unwrap();
        }
    }
}

// Fix signature invalid error
pub fn fix_package_db(snapshot: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot fix package manager database as snapshot {} doesn't exist.", snapshot);
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
        prepare(snapshot);
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
        post_transactions(snapshot);
        if excode1.success() && excode2.success() && excode3.success() && excode4.success()
            && excode5.success() && excode6.success() && excode7.success() && excode8.success() {
            println!("Snapshot {}'s package manager database fixed successfully.", snapshot);
        } else {
            chr_delete(snapshot);
            println!("Fixing package manager database failed.");
        }
    }
}

// Delete init system files (Systemd, OpenRC, etc.)
pub fn init_system_clean(snapshot: &str, from: &str) {
    if from == "prepare" {
        Command::new("rm").arg("-rf")
                          .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/lib/systemd/*", snapshot))
                          .status().unwrap();
    } else if from == "deploy" {
        Command::new("rm").args(["-rf", "/var/lib/systemd/*"])
                          .status().unwrap();
        Command::new("rm").arg("-rf")
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/var/lib/systemd/*", snapshot))
                          .status().unwrap();
    }
}

// Copy init system files (Systemd, OpenRC, etc.) to shared
pub fn init_system_copy(snapshot: &str, from: &str) {
    if from == "post_transactions" {
        Command::new("rm").args(["-rf" ,"/var/lib/systemd/*"])
                          .status().unwrap();
        Command::new("cp").args(["-r", "--reflink=auto",])
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/var/lib/systemd/.", snapshot))
                          .arg("/var/lib/systemd/")
                          .status().unwrap();
    }
}

// Install atomic-operation
pub fn install_package(snapshot:&str, pkg: &str) -> i32 {
    // This extra pacman check is to avoid unwantedly triggering AUR if package is official but user answers no to prompt
    let excode = Command::new("pacman").arg("-Si")
                                       .arg(format!("{}", pkg))
                                       .status().unwrap(); // --sysroot
    if !excode.success() {
        prepare(snapshot);
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
        prepare(snapshot);
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

// Install atomic-operation in live snapshot //REVIEW
//pub fn install_package_live(snapshot: &str, pkg: &str) {
    // This extra pacman check is to avoid unwantedly triggering AUR if package is official but user answers no to prompt
    //let excode = Command::new("pacman").arg("-Si")
                                       //.arg(format!("{}", pkg))
                                       //.status().unwrap(); // --sysroot
    //if !excode.success() {
        //let options = snapshot_config_get();
        //if options["aur"] == "True" {
            //let aur_in_tmp = true;
        //} else {
            //let aur_in_tmp = false;
            //if aur_in_tmp && aur_check {
                //excode = aur_install_live_helper(tmp)
            //if excode:
                //os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}")
                //os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")
                //print("F: Live install failed and changes discarded!")
                //return excode
        //if snapshot_config_get(snapshot)["aur"] == "True":
            //aur_in_destination_snapshot = True
        //else:
            //aur_in_destination_snapshot = False
            //print("F: AUR not enabled in target snapshot!") ### REVIEW
        //### REVIEW - error checking, handle the situation better altogether
        //if aur_in_destination_snapshot and not aur_in_tmp:
            //print("F: AUR is not enabled in current live snapshot, but is enabled in target.\nEnable AUR for live snapshot? (y/n)")
            //reply = input("> ")
            //while reply.casefold() != "y" and reply.casefold() != "n":
                //print("Please enter 'y' or 'n':")
                //reply = input("> ")
            //if reply == "y":
                //if not aur_check(tmp):
                    //excode = aur_install_live_helper(tmp)
                    //if excode:
                        //os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}")
                        //os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")
                        //print("F: Live install failed and changes discarded!")
                        //return excode
            //else:
                //print("F: Not enabling AUR for live snapshot!")
                //excode = 1
    //else:
        //#ash_chroot_mounts(tmp) ### REVIEW If issues to have this in ashpk_core.py, uncomment this
        //excode = os.system(f"chroot /.snapshots/rootfs/snapshot-{tmp} pacman -Sy --overwrite '*' --noconfirm {pkg}{DEBUG}") ### REVIEW Maybe just do this in try section and remove else section!
        //return excode
//}

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
    prepare(snapshot);
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
