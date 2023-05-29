use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;
use crate::{check_mutability, chr_delete, get_tmp, immutability_disable, immutability_enable, prepare, post_transactions, write_desc};

// Check if AUR is setup right
pub fn aur_check() -> bool {
    let options = snapshot_config_get();
    if options["aur"] == "True" {
        let aur = true;
        return aur;
    } else if options["aur"] == "False" {
        let aur = false;
        return aur;
    } else {
        panic!("Please insert valid value for aur in /.snapshots/etc/etc-{}/ash.conf", snap());
    }
}

// Noninteractive update
pub fn auto_upgrade(snapshot: &str) {
    sync_time(); // Required in virtualbox, otherwise error in package db update
    prepare(snapshot);
    if !aur_check() {
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
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(["su", "aur", "-c", "paru --noconfirm -Syy"])
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

// Copy cache of downloaded packages to shared
pub fn cache_copy(snapshot: &str) {
    Command::new("cp").args(["-n", "-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/cache/pacman/pkg/.", snapshot))
                      .arg("/var/cache/pacman/pkg/").status().unwrap();
}

// Everything after '#' is a comment
fn comment_after_hash(line: &mut String) -> &str {
    if line.contains("#") {
        let line = line.split("#").next().unwrap();
        return line;
    } else {
        return line;
    }
}

// Fix signature invalid error //This's ugly code //REVIEW
//pub fn fix_package_db(snapshot: &str) {
    //if Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        //eprintln!("Cannot fix package manager database as snapshot {} doesn't exist.", snapshot);
    //} else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        //eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot, snapshot);
    //} else if snapshot == "0" {
        //let mut p = Command::new("chroot");
        //while p.status().is_ok() {
            //if check_mutability(snapshot) {
                //immutability_disable(snapshot);
            //}
            //prepare(snapshot);
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                  //.args(["rm", "-rf"])
                                  //.arg("/etc/pacman.d/gnupg")
                                  //.arg("/home/me/.gnupg").output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["rm", "-r"])
             //.arg("/var/lib/pacman/db.lck").output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["pacman", "-Syy"]).output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["gpg", "--refresh-keys"]).output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["killall", "gpg-agent"]).output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["pacman-key", "--init"]).output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["pacman-key", "--populate", "archlinux"])
             //.output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["pacman", "-Syvv", "--noconfirm", "archlinux-keyring"])
             //.output().expect("Fixing package manager database failed.");
            //post_transactions(snapshot);
            //if !check_mutability(snapshot) {
                //immutability_enable(snapshot);
            //}
            //println!("Snapshot {}'s package manager database fixed successfully.", snapshot);
            //break;
        //}
        //if p.status().is_err(){
            //chr_delete(snapshot);
        //}
    //} else {
        //let mut p = Command::new("");
        //while p.status().is_ok() {
            //if check_mutability(snapshot) {
                //immutability_disable(snapshot);
            //}
            //prepare(snapshot);
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                  //.args(["rm", "-rf"])
                                  //.arg("/etc/pacman.d/gnupg")
                                  //.arg("/home/me/.gnupg").output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["rm", "-r"])
             //.arg("/var/lib/pacman/db.lck").output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["pacman", "-Syy"]).output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["gpg", "--refresh-keys"]).output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["killall", "gpg-agent"]).output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["pacman-key", "--init"]).output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["pacman-key", "--populate", "archlinux"])
             //.output().expect("Fixing package manager database failed.");
            //p.arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
             //.args(["pacman", "-Syvv", "--noconfirm", "archlinux-keyring"])
             //.output().expect("Fixing package manager database failed.");
            //post_transactions(snapshot);
            //if !check_mutability(snapshot) {
                //immutability_enable(snapshot);
            //}
            //println!("Snapshot {}'s package manager database fixed successfully.", snapshot);
            //break;
        //}
        //if p.status().is_err(){
            //chr_delete(snapshot);
//        }
//    }
//}

// Delete init system files (Systemd, OpenRC, etc.)
pub fn init_system_clean(snapshot: &str, from: &str) {
    if from == "prepare"{
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

// Install atomic-operation //REVIEW
//pub fn install_package(snapshot:&str, pkg: &str) {
    // This extra pacman check is to avoid unwantedly triggering AUR if package is official but user answers no to prompt
    //let excode = Command::new("pacman").arg("-Si")
                                       //.arg(format!("{}", pkg))
                                       //.status().unwrap(); // --sysroot
    //if !excode.success() {
        //prepare(snapshot);
        //if aur_check() {
            //Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                  //.args(["su", "aur", "-c", "\'paru", "-S"])
                                  //.arg(format!("{}", pkg))
                                  //.args(["--needed", "--overwrite", "'/var/*''"])
                                  //.status().unwrap();
        //} else {
            //eprintln!("AUR is not enabled!");
        //}
    //} else {
        //prepare(snapshot);
        //Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                              //.args(["pacman", "-S"])
                              //.arg(format!("{}", pkg))
                              //.args(["--needed", "--overwrite", "'/var/*'"])
                              //.status().unwrap();
    //}
//}

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
pub fn refresh_helper(snapshot: &str) {
    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          .args(["pacman", "-Syy"]).status().unwrap();
}

// Read snap file
pub fn snap() -> String {
    let source_dep = get_tmp();
    let sfile = File::open(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", source_dep)).unwrap();
    let mut buf_read = BufReader::new(sfile);
    let mut snap_value = String::new();
    buf_read.read_line(&mut snap_value).unwrap();
    let snap = snap_value.replace(" ", "").replace("\n", "");
    snap
}

// Get per-snapshot configuration options
pub fn snapshot_config_get() -> HashMap<String, String> {
    let mut options = HashMap::new();

    if !Path::new(&format!("/.snapshots/etc/etc-{}/ash.conf", snap())).try_exists().unwrap() {
        // defaults here
        options.insert(String::from("aur"), String::from("False"));
        options.insert(String::from("mutable_dirs"), String::new());
        options.insert(String::from("mutable_dirs_shared"), String::new());
        return options;
    } else {
        let optfile = File::open(format!("/.snapshots/etc/etc-{}/ash.conf", snap())).unwrap();
        let reader = BufReader::new(optfile);

        for line in reader.lines() {
            let mut line = line.unwrap();
            // Skip line if there's no option set
            if comment_after_hash(&mut line).contains("::") {
                // Split options with '::'
                let (left, right) = line.split_once("::").unwrap();
                // Remove newline here
                options.insert(left.to_string(), right.trim_end().to_string().split("#").next().unwrap().to_string());
            }
        }
        return options;
    }
}

// Show diff of packages between 2 snapshots
pub fn snapshot_diff(snap1: &str, snap2: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snap1)).try_exists().unwrap() {
        println!("Snapshot {} not found.", snap1);
    } else if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snap2)).try_exists().unwrap() {
        println!("Snapshot {} not found.", snap2);
    } else {
        Command::new("bash")
                .arg("-c")
                .arg(format!("diff <(ls /.snapshots/rootfs/snapshot-{}/usr/share/ash/db/local)\\
 <(ls /.snapshots/rootfs/snapshot-{}/usr/share/ash/db/local) | grep '^>\\|^<' | sort", snap1, snap2))
                .status().unwrap();
    }
}

// Sync time
pub fn sync_time() {
    Command::new("sh")
        .arg("-c")
        .arg("date -s \"$(curl --tlsv1.3 --proto =https -I https://google.com 2>&1 | grep Date: | cut -d\" \" -f3-6)Z\"")
        .status().unwrap();
}

// Uninstall package(s) atomic-operation
pub fn uninstall_package_helper(snapshot: &str, pkg: &str) {
    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          .args(["pacman", "--noconfirm", "-Rns"])
                          .arg(format!("{}", pkg)).status().unwrap();
}

// Upgrade snapshot atomic-operation
pub fn upgrade_helper(snapshot: &str) -> String {
    prepare(snapshot);
    if !aur_check() {
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(["pacman", "-Syyu"])
                                           .status().unwrap().to_string();
        excode
    } else {
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(["su", "aur", "-c", "paru -Syyu"])
                                           .status().unwrap().to_string();
        excode
    }
}
