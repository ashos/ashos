use crate::{check_mutability, chr_delete, get_current_snapshot, immutability_disable, immutability_enable, post_transactions,
            prepare, remove_dir_content, snapshot_config_get, sync_time, get_tmp};

use rustix::path::Arg;
use std::fs::{DirBuilder, OpenOptions, read_dir};
use std::io::{Error, ErrorKind, Write};
use std::path::Path;
use std::process::{Command, ExitStatus};
use users::get_user_by_name;
use users::os::unix::UserExt;
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
        panic!("Please insert valid value for aur in /.snapshots/etc/etc-{}/ash/ash.conf", snapshot);
    }
}

// Noninteractive update
pub fn auto_upgrade(snapshot: &str) -> Result<(), Error> {
    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot upgrade as snapshot {} doesn't exist.", snapshot);

    } else {
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
                let mut file = OpenOptions::new().write(true)
                                                 .create(true)
                                                 .truncate(true)
                                                 .open("/.snapshots/ash/upstate")?;
                file.write_all("0 ".as_bytes())?;
                let mut file = OpenOptions::new().append(true)
                                                 .open("/.snapshots/ash/upstate")?;
                let date = Command::new("date").output()?;
                file.write_all(format!("\n{}", &date.stdout.to_string_lossy().as_str()?).as_bytes())?;
            } else {
                chr_delete(snapshot)?;
                let mut file = OpenOptions::new().write(true)
                                                 .create(true)
                                                 .truncate(true)
                                                 .open("/.snapshots/ash/upstate")?;
                file.write_all("1 ".as_bytes())?;
                let mut file = OpenOptions::new().append(true)
                                                 .open("/.snapshots/ash/upstate")?;
                let date = Command::new("date").output()?;
                file.write_all(format!("\n{}", &date.stdout.to_string_lossy().as_str()?).as_bytes())?;
            }
        } else {
            // Use paru if aur is enabled
            let args = format!("paru -Syyu --noconfirm");
            let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                               .args(["su", "aur", "-c", &args])
                                               .status().unwrap();
            if excode.success() {
                post_transactions(snapshot)?;
                let mut file = OpenOptions::new().write(true)
                                                 .create(true)
                                                 .truncate(true)
                                                 .open("/.snapshots/ash/upstate")?;
                file.write_all("0 ".as_bytes())?;
                let mut file = OpenOptions::new().append(true)
                                                 .open("/.snapshots/ash/upstate")?;
                let date = Command::new("date").output()?;
                file.write_all(format!("\n{}", &date.stdout.to_string_lossy().as_str()?).as_bytes())?;
            } else {
                chr_delete(snapshot)?;
                let mut file = OpenOptions::new().write(true)
                                                 .create(true)
                                                 .truncate(true)
                                                 .open("/.snapshots/ash/upstate")?;
                file.write_all("1 ".as_bytes())?;
                let mut file = OpenOptions::new().append(true)
                                                 .open("/.snapshots/ash/upstate")?;
                let date = Command::new("date").output()?;
                file.write_all(format!("\n{}", &date.stdout.to_string_lossy().as_str()?).as_bytes())?;
            }
        }
    }
    Ok(())
}

// Copy cache of downloaded packages to shared
pub fn cache_copy(snapshot: &str, prepare: bool) -> Result<(), Error> {
    let tmp = get_tmp();
    if prepare {
        Command::new("cp").args(["-n", "-r", "--reflink=auto"])
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/var/cache/pacman/pkg", snapshot))
                          .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/cache/pacman/pkg", tmp))
                          .output().unwrap();
    } else {
        Command::new("cp").args(["-n", "-r", "--reflink=auto"])
                          .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/cache/pacman/pkg", snapshot))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/var/cache/pacman/pkg", tmp))
                          .output().unwrap();
    }
    Ok(())
}

// Fix signature invalid error
pub fn fix_package_db(snapshot: &str) -> Result<(), Error> {
    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() && !snapshot.is_empty() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot fix package man database as snapshot {} doesn't exist.", snapshot)));

        // Make sure snapshot is not in use
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                               snapshot,snapshot)));

    } else if snapshot.is_empty() && get_current_snapshot() == "0" {
        // Base snapshot unsupported
        return Err(Error::new(ErrorKind::Unsupported, format!("Snapshot 0 (base) should not be modified.")));

    } else if snapshot == "0" {
        // Base snapshot unsupported
        return Err(Error::new(ErrorKind::Unsupported, format!("Snapshot 0 (base) should not be modified.")));

    } else {
        let run_chroot: bool;
        // If snapshot is current running
        run_chroot = if snapshot.is_empty() {
            false
        } else {
            true
        };

        // Snapshot is mutable so do not make it immutable after fixdb is done
        let flip = if check_mutability(snapshot) {
            false
        } else {
            if immutability_disable(snapshot).is_ok() {
                println!("Snapshot {} successfully made mutable.", snapshot);
            }
            true
        };

        // Fix package database
        if run_chroot {
            prepare(snapshot)?;
        }
        let mut cmds: Vec<String> = Vec::new();
        let username = std::env::var_os("SUDO_USER").unwrap();
        let user = get_user_by_name(&username).unwrap();
        let home_dir = user.home_dir();
        let home = home_dir.to_str().unwrap();
        if run_chroot {
            let etc_gnupg = format!("/.snapshots/rootfs/snapshot-chr{}/etc/pacman.d/gnupg", snapshot);
            if Path::new(&etc_gnupg).try_exists().unwrap() && read_dir(&etc_gnupg)?.count() > 0 {
                cmds.push(format!("rm -rf /etc/pacman.d/gnupg"));
            }
            if Path::new(&format!("{}/.gnupg", home)).try_exists().unwrap() && read_dir(&format!("{}/.gnupg", home))?.count() > 0 {
                cmds.push(format!("rm -rf {}/.gnupg", home));
            }
            if Path::new("/var/lib/pacman/sync").try_exists().unwrap() && read_dir("/var/lib/pacman/sync")?.count() > 0 {
                cmds.push(format!("rm -r /var/lib/pacman/sync/*"));
            }
            cmds.push(format!("pacman -Syy"));
            cmds.push(format!("sudo -u {} gpg --refresh-keys", username.to_str().unwrap()));
            cmds.push(format!("pacman-key --init"));
            cmds.push(format!("pacman-key --populate archlinux"));
            cmds.push(format!("pacman -Syvv --noconfirm archlinux-keyring"));
        } else {
            if Path::new("/etc/pacman.d/gnupg").try_exists().unwrap() && read_dir("/etc/pacman.d/gnupg")?.count() > 0 {
                cmds.push(format!("rm -rf /etc/pacman.d/gnupg"));
            }
            if Path::new(&format!("{}/.gnupg", home)).try_exists().unwrap() && read_dir(&format!("{}/.gnupg", home))?.count() > 0 {
                cmds.push(format!("rm -rf {}/.gnupg", home));
            }
            if Path::new("/var/lib/pacman/sync").try_exists().unwrap() && read_dir("/var/lib/pacman/sync")?.count() > 0 {
                cmds.push(format!("rm -r /var/lib/pacman/sync/*"));
            }
            cmds.push(format!("sudo -u {} gpg --refresh-keys", username.to_str().unwrap()));
            cmds.push(format!("pacman-key --init"));
            cmds.push(format!("pacman-key --populate archlinux"));
        }
        for cmd in cmds {
            if run_chroot {
                let excode = Command::new("sh").arg("-c")
                                                .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} {}",
                                                             snapshot,&cmd)).status()?;
                if !excode.success() {
                    return Err(Error::new(ErrorKind::Other,
                                          format!("Run command {} failed.", &cmd)));
                }
            } else {
                let excode = Command::new("sh").arg("-c")
                                  .arg(&cmd).status()?;
                if !excode.success() {
                    return Err(Error::new(ErrorKind::Other,
                                          format!("Run command {} failed.", &cmd)));
                }
            }
        }
        if snapshot.is_empty() {
            let snapshot = get_current_snapshot();
            prepare(&snapshot)?;
            refresh_helper(&snapshot).expect("Refresh failed.");
        }

        // Return snapshot to immutable after fixdb is done if snapshot was immutable
        if flip {
            if immutability_enable(snapshot).is_ok() {
                println!("Snapshot {} successfully made immutable.", snapshot);
            }
        }
    }
    Ok(())
}

// Install atomic-operation
pub fn install_package_helper(snapshot:&str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    prepare(snapshot)?;
    for pkg in pkgs {
        // This extra pacman check is to avoid unwantedly triggering AUR if package is official
        let pacman_si_arg = format!("pacman -Si {}", pkg);
        let excode = Command::new("sh").arg("-c")
                                       .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} {}", snapshot,pacman_si_arg))
                                       .output()?; // --sysroot
        let pacman_sg_arg = format!("pacman -Sg {}", pkg);
        let excode_group = Command::new("sh").arg("-c")
                                             .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} {}", snapshot,pacman_sg_arg))
                                             .output()?;
        if excode.status.success() || excode_group.status.success() {
            let pacman_args = if noconfirm {
                format!("pacman -S --noconfirm --needed --overwrite '/var/*' {}", pkg)
            } else {
                format!("pacman -S --needed --overwrite '/var/*' {}", pkg)
            };
            let excode = Command::new("sh").arg("-c")
                                            .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} {}", snapshot,pacman_args))
                                            .status()?;
            if !excode.success() {
                return Err(Error::new(ErrorKind::Other,
                                      format!("Failed to install {}.", pkg)));
            }
        } else if aur_check(snapshot) {
            // Use paru if aur is enabled
            let paru_args = if noconfirm {
                format!("paru -S --noconfirm --needed --overwrite '/var/*' {}", pkg)
            } else {
                format!("paru -S --needed --overwrite '/var/*' {}", pkg)
            };
            let excode = Command::new("chroot")
                .arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                .args(["su", "aur", "-c"])
                .arg(&paru_args)
                .status()?;
            if !excode.success() {
                return Err(Error::new(ErrorKind::Other,
                                      format!("Failed to install {}.", pkg)));
            }
        } else if !aur_check(snapshot) {
            return Err(Error::new(ErrorKind::NotFound,
                                  "Please enable AUR."));
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
        let excode_group = Command::new("pacman").arg("-Sg")
                                                 .arg(format!("{}", pkg))
                                                 .output()?;
        if excode.status.success() || excode_group.status.success() {
            let pacman_args = if noconfirm {
                format!("pacman -Sy --noconfirm --overwrite '*' {}", pkg)
            } else {
                format!("pacman -Sy --overwrite '*' {}", pkg)
            };
            let excode = Command::new("sh")
                .arg("-c")
                .arg(format!("chroot /.snapshots/rootfs/snapshot-{} {}", tmp,pacman_args))
                .status()?;
            if !excode.success() {
                return Err(Error::new(ErrorKind::Other,
                                      format!("Failed to install {}.", pkg)));
            }
        } else if aur_check(snapshot) {
            // Use paru if aur is enabled
            let paru_args = if noconfirm {
                format!("paru -Sy --noconfirm --overwrite '*' {}", pkg)
            } else {
                format!("paru -Sy --overwrite '*' {}", pkg)
            };
            let excode = Command::new("chroot")
                .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                .args(["su", "aur", "-c"])
                .arg(&paru_args)
                .status()?;
            if !excode.success() {
                return Err(Error::new(ErrorKind::Other,
                                      format!("Failed to install {}.", pkg)));
            }
        }
    }
    Ok(())
}

// Get list of packages installed in a snapshot
pub fn pkg_list(snapshot: &str, chr: &str) -> Vec<String> {
    let excode = Command::new("sh").arg("-c")
                                   .arg(format!("chroot /.snapshots/rootfs/snapshot-{}{} pacman -Qq", chr,snapshot))
                                   .output().unwrap();
    let stdout = String::from_utf8_lossy(&excode.stdout).trim().to_string();
    stdout.split('\n').map(|s| s.to_string()).collect()
}

// Pacman query
pub fn pkg_query(pkg: &str) -> Result<ExitStatus, Error> {
    let excode = Command::new("pacman").arg("-Q").arg(pkg).status();
    excode
}

// Refresh snapshot atomic-operation
pub fn refresh_helper(snapshot: &str) -> Result<(), Error> {
    let refresh = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                        .args(["pacman", "-Syy"])
                                        .status()?;
    // Avoid invalid or corrupted package (PGP signature) error
    let keyring = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                        .args(["pacman", "-S", "--noconfirm", "archlinux-keyring"])
                                        .status()?;
    if !refresh.success() {
        return Err(Error::new(ErrorKind::Other,
                              "Refresh failed."));
    }
    if !keyring.success() {
        return Err(Error::new(ErrorKind::Other,
                              "Failed to update archlinux-keyring."));
    }
   Ok(())
}

// Enable service(s) (Systemd, OpenRC, etc.)
pub fn service_enable(snapshot: &str, services: &Vec<String>) -> Result<(), Error> {
    for service in services {
        // Systemd
        if Path::new("/var/lib/systemd/").try_exists().unwrap() {
            let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                               .arg("systemctl")
                                               .arg("enable")
                                               .arg(&service).status()?;
            if !excode.success() {
                return Err(Error::new(ErrorKind::Other,
                                      format!("Failed to enable {}.", service)));
            }
        } //TODO add OpenRC
    }
    Ok(())
}

// Disable service(s) (Systemd, OpenRC, etc.)
pub fn service_disable(snapshot: &str, services: &Vec<String>) -> Result<(), Error> {
    for service in services {
        // Systemd
        if Path::new("/var/lib/systemd/").try_exists().unwrap() {
            let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                               .arg("systemctl")
                                               .arg("disable")
                                               .arg(&service).status()?;
            if !excode.success() {
                return Err(Error::new(ErrorKind::Other,
                                      format!("Failed to disable {}.", service)));
            }
        } //TODO add OpenRC
    }
    Ok(())
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
    remove_dir_content(&format!("/.snapshots/rootfs/snapshot-{}{}/usr/share/ash/db/local", chr,s_t))?;
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
        let pacman_args = if noconfirm {
            ["pacman", "--noconfirm", "-Rns"]
        } else {
            ["pacman", "--confirm", "-Rns"]
        };

        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           .args(pacman_args)
                                           .arg(format!("{}", pkg)).status()?;

        if !excode.success() {
            return Err(Error::new(ErrorKind::Other,
                                  format!("Failed to uninstall {}.", pkg)));
        }
    }
    Ok(())
}

// Uninstall package(s) atomic-operation live snapshot
pub fn uninstall_package_helper_live(tmp: &str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    for pkg in pkgs {
        let pacman_args = if noconfirm {
            ["pacman", "--noconfirm", "-Rns"]
        } else {
            ["pacman", "--confirm", "-Rns"]
        };

        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                                           .args(pacman_args)
                                           .arg(format!("{}", pkg)).status()?;

        if !excode.success() {
            return Err(Error::new(ErrorKind::Other,
                                  format!("Failed to uninstall {}.", pkg)));
        }
    }
    Ok(())
}

// Upgrade snapshot atomic-operation
pub fn upgrade_helper(snapshot: &str, noconfirm: bool) -> Result<(), Error> {
    // Prepare snapshot
    prepare(snapshot).unwrap();
    // Avoid invalid or corrupted package (PGP signature) error
    let pacman_args = if noconfirm {
        ["pacman", "--noconfirm", "-Syy", "archlinux-keyring"]
    } else {
        ["pacman", "--confirm", "-Syy", "archlinux-keyring"]
    };

    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          .args(pacman_args)
                          .status().unwrap();
    if !aur_check(snapshot) {
        let pacman_args = if noconfirm {
            ["pacman", "--noconfirm", "-Syyu"]
        } else {
            ["pacman", "--confirm", "-Syyu"]
        };

        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                            .args(pacman_args)
                                            .status().unwrap();
        if !excode.success() {
            return Err(Error::new(ErrorKind::Other,
                                  format!("Failed to upgrade snapshot {}.", snapshot)));
        }
    } else {
        let paru_args = if noconfirm {
            "paru --noconfirm -Syyu"
        } else {
            "paru -Syyu"
        };

        let excode = Command::new("sh").arg("-c")
                                        .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} su aur -c '{}'", snapshot,paru_args))
                                        .status().unwrap();
        if !excode.success() {
            return Err(Error::new(ErrorKind::Other,
                                  format!("Failed to upgrade snapshot {}.", snapshot)));
        }
    }
    Ok(())
}

// Live upgrade snapshot atomic-operation
pub fn upgrade_helper_live(tmp: &str, noconfirm: bool) -> Result<(), Error> {
    // Avoid invalid or corrupted package (PGP signature) error
    let pacman_args = if noconfirm {
        ["pacman", "--noconfirm", "-Syy", "archlinux-keyring"]
    } else {
        ["pacman", "--confirm", "-Syy", "archlinux-keyring"]
    };

    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                          .args(pacman_args)
                          .status().unwrap();
    if !aur_check(tmp) {
        let pacman_args = if noconfirm {
            ["pacman", "--noconfirm", "-Syyu"]
        } else {
            ["pacman", "--confirm", "-Syyu"]
        };

        let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                                           .args(pacman_args)
                                           .status().unwrap();
        if !excode.success() {
            return Err(Error::new(ErrorKind::Other,
                                  "Failed to upgrade current/live snapshot."));
        }
    } else {
        let paru_args = if noconfirm {
            "paru --noconfirm -Syyu"
        } else {
            "paru --confirm -Syyu"
        };

        let excode = Command::new("sh")
            .arg("-c")
            .arg(format!("chroot /.snapshots/rootfs/snapshot-{} su aur -c '{}'",
                         tmp,paru_args))
            .status().unwrap();
        if !excode.success() {
            return Err(Error::new(ErrorKind::Other,
                                  "Failed to upgrade current/live snapshot."));
        }
    }
    Ok(())
}
