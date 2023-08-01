use crate::{check_mutability, chr_delete, immutability_disable, immutability_enable, prepare, post_transactions,
            remove_dir_content, snapshot_config_get, sync_time};
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::process::{Command, ExitStatus};
use walkdir::WalkDir;

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
pub fn auto_upgrade(snapshot: &str) -> std::io::Result<()> {
    // Required in virtualbox, otherwise error in package db update
    sync_time();
    prepare(snapshot)?;
    if !aur_check(snapshot) {
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(["pacman", "--noconfirm", "-Syyu"]).status()?;
        if excode.success() {
            post_transactions(snapshot)?;
            Command::new("echo").args(["0", ">"]).arg("/.snapshots/ash/upstate").output()?;
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").output()?;
        } else {
            chr_delete(snapshot)?;
            Command::new("echo").args(["1", ">"]).arg("/.snapshots/ash/upstate").output()?;
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").output()?;
        }
    } else {
        let excode = Command::new("sh").arg("-c")
                                       .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} su aur -c 'paru --noconfirm -Syy'", snapshot))
                                       .status()?;
        if excode.success() {
            post_transactions(snapshot)?;
            Command::new("echo").args(["0", ">"]).arg("/.snapshots/ash/upstate").output()?;
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").output()?;
        } else {
            chr_delete(snapshot)?;
            Command::new("echo").args(["1", ">"]).arg("/.snapshots/ash/upstate").output()?;
            Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").output()?;
        }
    }
    Ok(())
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
            immutability_disable(snapshot).unwrap();
            true
        };
        prepare(snapshot).unwrap();
        if flip {
            immutability_enable(snapshot).unwrap();
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
pub fn install_package(snapshot:&str, pkg: &str) -> std::io::Result<()> {
    // This extra pacman check is to avoid unwantedly triggering AUR if package is official
    let excode = Command::new("pacman").arg("-Si")
                          .arg(format!("{}", pkg))
                          .output()?; // --sysroot
    if !excode.status.success() {
        // Use paru if aur is enabled
        if aur_check(snapshot) {
            Command::new("sh")
                .arg("-c")
                .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} su aur -c \'paru -S {} --needed --overwrite '/var/*''\'", snapshot,pkg))
                .status()?;
        } else {
            return Err(Error::new(ErrorKind::NotFound,
                                  format!("AUR is not enabled!")));
        }
    } else {
        prepare(snapshot)?;
        Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                              .args(["pacman", "-S"])
                              .arg(format!("{}", pkg))
                              .args(["--needed", "--overwrite", "'/var/*'"])
                              .status()?;
    }
    Ok(())
}

// Install atomic-operation in live snapshot
pub fn install_package_live(snapshot: &str, tmp: &str, pkg: &str) -> std::io::Result<()> {
    // This extra pacman check is to avoid unwantedly triggering AUR if package is official
    let excode = Command::new("pacman").arg("-Si")
                                       .arg(format!("{}", pkg))
                                       .output()?; // --sysroot
    if excode.status.success() {
        Command::new("sh")
            .arg("-c")
            .arg(format!("chroot /.snapshots/rootfs/snapshot-{} pacman -Sy --overwrite '*' --noconfirm {}", tmp,pkg))
            .status()?;
    } else {
        // Use paru if aur is enabled
        if aur_check(snapshot) {
            Command::new("sh")
                .arg("-c")
                .arg(format!("chroot /.snapshots/rootfs/snapshot-{} su aur -c 'paru -Sy --overwrite '*' --noconfirm {}'", tmp,pkg))
                .status()?;
        } else {
            return Err(Error::new(ErrorKind::NotFound,
                                  format!("AUR is not enabled!")));
        }
    }
    Ok(())
}

//  Distro-specific function to setup snapshot based on preset parameters
//def presets_helper(prof_cp, snap): ### TODO before: prof_section
    //if prof_cp.has_option('presets', 'enable_aur'):
//###        if aur is not already set to True: ### TODO IMPORTANT, concat generic preset and distro preset and paste it in /.snapshots/etc
        //print("Opening snapshot's config file... Please change AUR to True")
        //snapshot_config_edit(snap, False, False) ### TODO move to core.py ? Run prepare but skip post transaction (Optimize code) Update: post_tran need to run too
        //aur_install(snap, False, False) ### Skip prepare, but run post transaction (Optimize code) ### update: because of last step, had to run prepare again too!


// Get list of packages installed in a snapshot
pub fn pkg_list(snapshot: &str, chr: &str) -> Vec<String> {
    let excode = Command::new("sh").arg("-c")
                                   .arg(format!("chroot /.snapshots/rootfs/snapshot-{}{} pacman -Qq", chr,snapshot))
                                   .output().unwrap();
    let stdout = String::from_utf8_lossy(&excode.stdout).trim().to_string();
    stdout.split('\n').map(|s| s.to_string()).collect()
}

// Refresh snapshot atomic-operation
pub fn refresh_helper(snapshot: &str) -> ExitStatus {
    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          .args(["pacman", "-Syy"]).status().unwrap()
}

// Show diff of packages between 2 snapshots
pub fn snapshot_diff(snapshot1: &str, snapshot2: &str) -> std::io::Result<()> {
    // Make sure snapshot one does exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot1)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Snapshot {} not found.", snapshot1)));

        // Make sure snapshot two does exists
        } else if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot2)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Snapshot {} not found.", snapshot2)));

    } else {
        let snap1 = format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/db/local", snapshot1);
        let snap2 = format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/db/local", snapshot2);
        let path1 = Path::new(&snap1);
        let path2 = Path::new(&snap2);
        // Iterate over all directories in dir2 and check if any are missing in dir1
        let mut missing_dirs = Vec::new();
        for entry in WalkDir::new(path2) {
            let entry = entry.unwrap();
            if entry.file_type().is_dir() {
                let relative_path = entry.path().strip_prefix(path2).unwrap();
                let dir1_path = path1.join(relative_path);

                if !dir1_path.exists() {
                    let dir_name = relative_path.file_name().unwrap();
                    missing_dirs.push(dir_name.to_string_lossy().to_string());
                }
            }
        }

        // Print the missing directory names
        if !missing_dirs.is_empty() {
            missing_dirs.sort();
            for dir_name in missing_dirs {
                println!("{}", dir_name);
            }
        }
    }
    Ok(())
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
