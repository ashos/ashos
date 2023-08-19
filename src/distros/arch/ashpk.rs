use crate::{check_mutability, chr_delete, get_current_snapshot, immutability_disable, immutability_enable, prepare, post_transactions,
            remove_dir_content, snapshot_config_get, sync_time};

use std::fs::{DirBuilder, read_dir};
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
pub fn auto_upgrade(snapshot: &str) -> Result<(), Error> {
    // Required in virtualbox, otherwise error in package db update
    sync_time()?;
    prepare(snapshot)?;

    // Avoid invalid or corrupted package (PGP signature) error
    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          .args(["pacman", "-Sy", "--noconfirm", "archlinux-keyring"])
                          .status().unwrap();

    if !aur_check(snapshot) {
        // Use pacman
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
        // Use paru if aur is enabled
        let args = format!("paru -Syyu --noconfirm");
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(["su", "aur", "-c", &args])
                                           .status().unwrap();
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
pub fn cache_copy(snapshot: &str) -> Result<(), Error> {
    Command::new("cp").args(["-n", "-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/cache/pacman/pkg", snapshot))
                      .arg("/var/cache/pacman/")
                      .output().unwrap();
    Ok(())
}

// Fix signature invalid error
pub fn fix_package_db(snapshot: &str) -> Result<(), Error> {
    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot fix package man database as snapshot {} doesn't exist.", snapshot)));

        // Make sure snapshot is not in use
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.",
                               snapshot,snapshot)));

    } else if snapshot == "0" {
        // Base snapshot unsupported
        return Err(Error::new(ErrorKind::Unsupported, format!("Snapshot 0 (base) should not be modified.")));

    } else {
        let run_chroot: bool;
        // If snapshot is current running
        run_chroot = if snapshot == get_current_snapshot() {
            false
        } else {
            true
        };

        // Snapshot is mutable so do not make it immutable after fixdb is done
        let flip = if check_mutability(snapshot) {
            false
        } else {
            if immutability_disable(snapshot).is_ok() {
                println!("Snapshot {} successfully made mutable", snapshot);
            }
            true
        };

        // Fix package database
        prepare(snapshot)?;
        let mut cmds: Vec<String> = Vec::new();
        let home_dir = std::env::var_os("HOME").unwrap();
        cmds.push(format!("rm -rf /etc/pacman.d/gnupg {}/.gnupg", home_dir.to_str().unwrap()));
        cmds.push(format!("rm -r /var/lib/pacman/sync/*")); // NOTE return No such file or directory!
        cmds.push(format!("pacman -Syy"));
        cmds.push(format!("gpg --refresh-keys"));
        cmds.push(format!("killall gpg-agent"));
        cmds.push(format!("pacman-key --init"));
        cmds.push(format!("pacman-key --populate archlinux"));
        cmds.push(format!("pacman -Syvv --noconfirm archlinux-keyring"));
        for cmd in cmds {
            if run_chroot {
                Command::new("sh").arg("-c").arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} {}",
                                                   snapshot,cmd)).status()?;
            } else {
                Command::new("sh").arg("-c")
                                  .arg(cmd).status()?;
            }
        }

        // Return snapshot to immutable after fixdb is done if snapshot was immutable
        if flip {
            if immutability_enable(snapshot).is_ok() {
                println!("Snapshot {} successfully made immutable", snapshot);
            }
        }
    }
    Ok(())
}

// Delete init system files (Systemd, OpenRC, etc.)
pub fn init_system_clean(snapshot: &str, from: &str) -> Result<(), Error> {
    if from == "prepare" {
        remove_dir_content(&format!("/.snapshots/rootfs/snapshot-chr{}/var/lib/systemd/", snapshot))?;
    } else if from == "deploy" {
        remove_dir_content("/var/lib/systemd/")?;
        remove_dir_content(&format!("/.snapshots/rootfs/snapshot-{}/var/lib/systemd/", snapshot))?;
    }
    Ok(())
}

// Copy init system files (Systemd, OpenRC, etc.) to shared
pub fn init_system_copy(snapshot: &str, from: &str) -> Result<(), Error> {
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
pub fn install_package_helper(snapshot:&str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    prepare(snapshot)?;
    for pkg in pkgs {
        // This extra pacman check is to avoid unwantedly triggering AUR if package is official
        let excode = Command::new("pacman").arg("-Si")
                                           .arg(format!("{}", pkg))
                                           .output()?; // --sysroot
        if excode.status.success() {
            let pacman_args = if noconfirm {
                format!("pacman -S --noconfirm --needed --overwrite '/var/*' {}", pkg)
            } else {
                format!("pacman -S --needed --overwrite '/var/*' {}", pkg)
            };
            Command::new("sh").arg("-c")
                              .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} {}", snapshot,pacman_args))
                              .status()?;
        } else if aur_check(snapshot) {
            // Use paru if aur is enabled
            let paru_args = if noconfirm {
                format!("paru -S --noconfirm --needed --overwrite '/var/*' {}", pkg)
            } else {
                format!("paru -S --needed --overwrite '/var/*' {}", pkg)
            };
            Command::new("chroot")
                .arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                .args(["su", "aur", "-c"])
                .arg(&paru_args)
                .status()?;
        }

        // Check if succeeded
        if !is_package_installed(snapshot, &pkg) {
            return Err(Error::new(ErrorKind::NotFound,
                                  format!("Failed to install {}", pkg)));
        }
    }
    Ok(())
}

// Install atomic-operation in live snapshot
pub fn install_package_helper_live(snapshot: &str, tmp: &str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    for pkg in pkgs {
        // This extra pacman check is to avoid unwantedly triggering AUR if package is official
        let excode = Command::new("pacman").arg("-Si")
                                           .arg(format!("{}", pkg))
                                           .output()?; // --sysroot
        if excode.status.success() {
            let pacman_args = if noconfirm {
                format!("pacman -Sy --noconfirm --overwrite '*' {}", pkg)
            } else {
                format!("pacman -Sy --overwrite '*' {}", pkg)
            };
            Command::new("sh")
                .arg("-c")
                .arg(format!("chroot /.snapshots/rootfs/snapshot-{} {}", tmp,pacman_args))
                .status()?;
        } else if aur_check(snapshot) {
            // Use paru if aur is enabled
            let paru_args = if noconfirm {
                format!("paru -Sy --noconfirm --overwrite '*' {}", pkg)
            } else {
                format!("paru -Sy --overwrite '*' {}", pkg)
            };
            Command::new("chroot")
                .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                .args(["su", "aur", "-c"])
                .arg(&paru_args)
                .status()?;
        }

        // Check if succeeded
        if !is_package_live_installed(&pkg) {
            return Err(Error::new(ErrorKind::NotFound,
                                  format!("Failed to install {}", pkg)));
        }
    }
    Ok(())
}

// Check if package installed
fn is_package_installed(snapshot: &str, pkg: &str) -> bool {
    let package_db_path = format!("/.snapshots/rootfs/snapshot-chr{}/usr/share/ash/db/local", snapshot);

    if let Ok(entries) = read_dir(package_db_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let file_name = entry.file_name();
                if let Some(file_name_str) = file_name.to_str() {
                    if file_name_str.starts_with(pkg) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

// Check if package installed
fn is_package_live_installed(pkg: &str) -> bool {
    let package_db_path = format!("/usr/share/ash/db/local");

    if let Ok(entries) = read_dir(package_db_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let file_name = entry.file_name();
                if let Some(file_name_str) = file_name.to_str() {
                    if file_name_str.starts_with(pkg) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

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
    let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                       .args(["pacman", "-Syy"]).status().unwrap();
    // Avoid invalid or corrupted package (PGP signature) error
    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          .args(["pacman", "-S", "--noconfirm", "archlinux-keyring"])
                          .status().unwrap();
    return excode;
}

// Show diff of packages between 2 snapshots
pub fn snapshot_diff(snapshot1: &str, snapshot2: &str) -> Result<(), Error> {
    // Make sure snapshot one does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot1)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Snapshot {} not found.", snapshot1)));

        // Make sure snapshot two does exist
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

// Sync tree helper function
pub fn tree_sync_helper(s_f: &str, s_t: &str, chr: &str) -> Result<(), Error>  {
    DirBuilder::new().recursive(true)
                     .create("/.snapshots/tmp-db/local/")?;
    //remove_dir_content("/.snapshots/tmp-db/")?;
    let pkg_list_to = pkg_list(s_t, "chr");
    let pkg_list_from = pkg_list(s_f, "");

    // Get packages to be inherited
    let mut pkg_list_new = Vec::new();
    for j in pkg_list_from {
        if !pkg_list_to.contains(&j) {
            pkg_list_new.push(j);
        }
    }
    let pkg_list_from = pkg_list_new;
    Command::new("cp").arg("-r")
                      .arg(format!("/.snapshots/rootfs/snapshot-{}{}/usr/share/ash/db/local/.", chr,s_t))
                      .arg("/.snapshots/tmp-db/local/").output()?;
    Command::new("cp").args(["-n", "-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/.", s_f))
                      .arg(format!("/.snapshots/rootfs/snapshot-{}{}/", chr,s_t))
                      .output()?;
    remove_dir_content(format!("/.snapshots/rootfs/snapshot-{}{}/usr/share/ash/db/local", chr,s_t).as_str())?;
    Command::new("cp").arg("-r")
                      .arg("/.snapshots/tmp-db/local/.")
                      .arg(format!("/.snapshots/rootfs/snapshot-{}{}/usr/share/ash/db/local/", chr,s_t))
                      .output()?;
    for entry in pkg_list_from {
        Command::new("cp").arg("-r")
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/db/local/{}-[0-9]*", s_f,entry))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}{}/usr/share/ash/db/local/'", chr,s_t))
                          .output()?;
        }
    remove_dir_content("/.snapshots/tmp-db/local")?;
    Ok(())
}

// Uninstall package(s) atomic-operation
pub fn uninstall_package_helper(snapshot: &str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    for pkg in pkgs {
        // Check if package installed
        if !is_package_installed(snapshot, &pkg) {
            return Err(Error::new(ErrorKind::NotFound,
                                  format!("Package {} is not installed", pkg)));
        } else {
            let pacman_args = if noconfirm {
                ["pacman", "--noconfirm", "-Rns"]
            } else {
                ["pacman", "--confirm", "-Rns"]
            };

            Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                  .args(pacman_args)
                                  .arg(format!("{}", pkg)).status()?;

            // Check if package uninstalled successfully
            if is_package_installed(snapshot, &pkg) {
                return Err(Error::new(ErrorKind::AlreadyExists,
                                      format!("Failed to uninstall {}", pkg)));
            }
        }
    }
    Ok(())
}

// Uninstall package(s) atomic-operation live snapshot
pub fn uninstall_package_helper_live(tmp: &str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    for pkg in pkgs {
        // Check if package installed
        if !is_package_live_installed(&pkg) {
            return Err(Error::new(ErrorKind::NotFound,
                                  format!("Package {} is not installed", pkg)));
        } else {
            let pacman_args = if noconfirm {
                ["pacman", "--noconfirm", "-Rns"]
            } else {
                ["pacman", "--confirm", "-Rns"]
            };

            Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                                  .args(pacman_args)
                                  .arg(format!("{}", pkg)).status()?;

            // Check if package uninstalled successfully
            if is_package_live_installed(&pkg) {
                return Err(Error::new(ErrorKind::AlreadyExists,
                                      format!("Failed to uninstall {}", pkg)));
            }
        }
    }
    Ok(())
}

// Upgrade snapshot atomic-operation
pub fn upgrade_helper(snapshot: &str) -> ExitStatus {
    prepare(snapshot).unwrap();
    // Avoid invalid or corrupted package (PGP signature) error
    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          .args(["pacman", "-Syy", "archlinux-keyring"])
                          .status().unwrap();
    if !aur_check(snapshot) {
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(["pacman", "-Syyu"])
                                           .status().unwrap();
        excode
    } else {
        let excode = Command::new("sh").arg("-c")
                                       .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} su aur -c 'paru -Syyu'", snapshot))
                                       .status().unwrap();
        excode
    }
}

// Live upgrade snapshot atomic-operation
pub fn upgrade_helper_live(snapshot: &str) -> ExitStatus {
    // Avoid invalid or corrupted package (PGP signature) error
    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                          .args(["pacman", "-Sy", "--noconfirm", "archlinux-keyring"])
                          .status().unwrap();
    if !aur_check(snapshot) {
        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                           .args(["pacman", "--noconfirm", "-Syyu"])
                                           .status().unwrap();
        excode
    } else {
        let excode = Command::new("sh")
            .arg("-c")
            .arg(format!("chroot /.snapshots/rootfs/snapshot-{} su aur -c 'paru --noconfirm -Syyu'",
                         snapshot))
            .status().unwrap();
        excode
    }
}
