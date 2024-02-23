use crate::{chroot_exec, deploy_recovery, detect_distro, get_aux_tmp, get_part, get_recovery_aux_tmp,
            get_recovery_tmp, get_tmp, post_transactions, prepare};

use chrono::{NaiveDateTime, Local};
use nix::mount::{mount, MntFlags, MsFlags, umount2};
use std::fs::{copy, File, OpenOptions};
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Write};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use walkdir::{DirEntry, WalkDir};

// Delete old grub.cfg
pub fn delete_old_grub_files(grub: &str) -> Result<(), Error> {
    let bak_path = Path::new(grub).join("BAK");
    for entry in WalkDir::new(bak_path) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.to_str().unwrap().contains("grub.cfg") {
            let file_ext = path.extension().unwrap().to_str().unwrap();
            let file_create_time = NaiveDateTime::parse_from_str(file_ext, "%Y%m%d-%H%M%S").unwrap();
            let cutoff_time = Local::now().naive_local().signed_duration_since(file_create_time);
            if cutoff_time.num_days() >= 30 {
                std::fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}

// Get Grub path
fn get_grub() -> Option<String> {
    let boot_dir = "/boot";
    let grub_dirs: Vec<String> = WalkDir::new(boot_dir)
        .into_iter()
        .filter_map(|entry| {
            let entry: DirEntry = entry.unwrap();
            if entry.file_type().is_dir() {
                if let Some(dir_path) = entry.path().file_name() {
                    if dir_path == "grub" {
                        return Some(entry.path().strip_prefix("/boot/").unwrap().to_string_lossy().into_owned());
                    }
                }
            }
            None
        })
        .collect();
    grub_dirs.get(0).cloned()
}

// Switch between /recovery-tmp deployments
pub fn switch_recovery_tmp() -> Result<(), Error> {
    let grub = get_grub().unwrap();
    let part = get_part();
    let tmp_boot = TempDir::new_in("/.snapshots/tmp")?;

    // Mount boot partition for writing
    mount(Some(part.as_str()), tmp_boot.path().as_os_str(),
          Some("btrfs"), MsFlags::empty(), Some("subvol=@boot_linux".as_bytes()))?;

    // Swap deployment subvolumes: deploy <-> deploy-aux
    let source_dep = get_recovery_tmp();
    let target_dep = get_recovery_aux_tmp(&source_dep);
    let boot_location = tmp_boot.path().to_str().unwrap();

    // Read the contents of the file into a string
    let grub_cfg = format!("{}/{}/grub.cfg", boot_location, grub);
    let src_file_path = format!("/.snapshots/rootfs/snapshot-{}/boot/{}/grub.cfg", source_dep,grub);
    let sfile = if Path::new(&src_file_path).is_file() {
        File::open(&src_file_path)?
    } else {
        File::open(format!("/.snapshots/rootfs/snapshot-{}/boot/{}/grub.cfg", target_dep,grub))?
    };
    let reader = BufReader::new(sfile);
    let mut gconf = String::new();
    let mut in_10_linux = false;
    let mut menu = false;
    for line in reader.lines() {
        let line = line?;
        if line.contains("BEGIN /etc/grub.d/10_linux") {
            in_10_linux = true;
        } else if in_10_linux {
            if line.contains("menuentry") {
                menu = true;
            }
            if line.contains("}") && menu {
                gconf.push_str(&line);
                gconf.push_str("\n### END /etc/grub.d/41_custom ###");
                break;
            } else if menu {
                gconf.push_str(&format!("\n{}",&line));
            }
        }
    }

    // Remove old recovery
    let gfile = File::open(&grub_cfg)?;
    let reader = BufReader::new(gfile);
    let mut ngrub_cfg = String::new();
    let mut recovery_mode = false;
    for line in reader.lines() {
        let line = line?;
        if line.contains("menuentry 'Recovery Mode'") {
            recovery_mode = true;
        } else if recovery_mode {
            if line.contains("}") {
                ngrub_cfg.retain(|s| s.to_string() != line);
                break;
            } else {
                ngrub_cfg.retain(|s| s.to_string() != line);
            }
        } else {
            ngrub_cfg.push_str(&format!("\n{}",&line));
        }
    }
    let mut gfile = File::create(&grub_cfg)?;
    gfile.write_all(ngrub_cfg.as_bytes())?;

    // Remove END of 41_custom
    let mut contents = String::new();
    let mut sfile = File::open(&grub_cfg)?;
    sfile.read_to_string(&mut contents)?;
    let modified_grub_contents = contents.replace("### END /etc/grub.d/41_custom ###", "");
    let mut nfile = File::create(&grub_cfg)?;
    nfile.write_all(modified_grub_contents.as_bytes())?;

    // Change recovery tmp
    let first_quote_index = gconf.find('\'').unwrap_or(0);
    let second_quote_index = gconf[first_quote_index + 1..].find('\'').unwrap_or(gconf.len()) + first_quote_index + 1;
    let updated_line = gconf.replace(&gconf[first_quote_index + 1..second_quote_index], "Recovery Mode");
    let modified_cfg_contents = if updated_line.contains(&source_dep) {
        updated_line.replace(&format!("@.snapshots_linux/rootfs/snapshot-{}", source_dep),
                             &format!("@.snapshots_linux/rootfs/snapshot-{}", target_dep))
    } else {
        let src_tmp = if updated_line.contains("deploy-aux") && !updated_line.contains("secondary") {
            "deploy-aux"
        } else if updated_line.contains("secondary") && !updated_line.contains("aux") {
            "deploy-secondary"
        } else if updated_line.contains("aux-secondary") {
            "deploy-aux-secondary"
        } else {
            "deploy"
        };

        updated_line.replace(&format!("@.snapshots_linux/rootfs/snapshot-{}", src_tmp),
                             &format!("@.snapshots_linux/rootfs/snapshot-{}", target_dep))
    };

    // Write the modified contents back to the file
    let mut file = OpenOptions::new().append(true)
                                     .create(true)
                                     .read(true)
                                     .open(grub_cfg)?;
    file.write_all(modified_cfg_contents.as_bytes())?;

    // Umount boot partition
    umount2(Path::new(&format!("{}", tmp_boot.path().as_os_str().to_str().unwrap())),
            MntFlags::MNT_DETACH)?;

    Ok(())
}

// Switch between /tmp deployments
pub fn switch_tmp(secondary: bool, reset: bool) -> Result<String, Error> {
    let grub = get_grub().unwrap();
    let part = get_part();
    let rec_tmp = get_recovery_tmp();
    let tmp_boot = TempDir::new_in("/.snapshots/tmp")?;

    // Mount boot partition for writing
    mount(Some(part.as_str()), tmp_boot.path().as_os_str(),
          Some("btrfs"), MsFlags::empty(), Some("subvol=@boot_linux".as_bytes()))?;

    // Swap deployment subvolumes: deploy <-> deploy-aux
    let source_dep = get_tmp();
    let target_dep = get_aux_tmp(source_dep.to_string(), secondary);
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/boot/grub", target_dep))
                      .arg(format!("{}", tmp_boot.path().to_str().unwrap()))
                      .output()?;

    // Update fstab for new deployment
    let fstab_file = format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", target_dep);
    // Read the contents of the file into a string
    let mut contents = String::new();
    let mut file = File::open(&fstab_file)?;
    file.read_to_string(&mut contents)?;
    let modified_boot_contents = contents.replace(&format!("@.snapshots_linux/boot/boot-{}", source_dep),
                                                  &format!("@.snapshots_linux/boot/boot-{}", target_dep));
    let modified_etc_contents = modified_boot_contents.replace(&format!("@.snapshots_linux/etc/etc-{}", source_dep),
                                                               &format!("@.snapshots_linux/etc/etc-{}", target_dep));
    let modified_var_contents = modified_etc_contents.replace(&format!("@.snapshots_linux/var/var-{}", source_dep),
                                                               &format!("@.snapshots_linux/var/var-{}", target_dep));
    let modified_rootfs_contents = modified_var_contents.replace(&format!("@.snapshots_linux/rootfs/snapshot-{}", source_dep),
                                                                 &format!("@.snapshots_linux/rootfs/snapshot-{}", target_dep));
    // Write the modified contents back to the file
    let mut file = File::create(fstab_file)?;
    file.write_all(modified_rootfs_contents.as_bytes())?;

    // Recovery GRUB configurations
    if !reset {
        for boot_location in [&tmp_boot.path().to_str().unwrap()] {
            // Get old grub configurations
            let grub_path = format!("{}/{}/grub.cfg", boot_location, grub);
            let src_file_path = format!("/.snapshots/rootfs/snapshot-{}/boot/{}/grub.cfg", source_dep,grub);
            let sfile = File::open(&src_file_path)?;
            let reader = BufReader::new(sfile);
            let mut gconf = String::new();
            let mut in_10_linux = false;
            let mut menu = false;
            for line in reader.lines() {
                let line = line?;
                if line.contains("BEGIN /etc/grub.d/10_linux") {
                    in_10_linux = true;
                } else if in_10_linux {
                    if line.contains("menuentry") {
                        menu = true;
                    }
                    if line.contains("}") && menu {
                        gconf.push_str(&line);
                        gconf.push_str("\n### END /etc/grub.d/41_custom ###");
                        break;
                    } else if menu {
                        gconf.push_str(&format!("\n{}",&line));
                    }
                }
            }

            // Remove END of 41_custom
            let mut contents = String::new();
            let mut sfile = File::open(&grub_path)?;
            sfile.read_to_string(&mut contents)?;
            let modified_grub_contents = contents.replace("### END /etc/grub.d/41_custom ###", "");
            let mut nfile = File::create(&grub_path)?;
            nfile.write_all(modified_grub_contents.as_bytes())?;

            // Open the file in read and write mode
            let mut file = OpenOptions::new().read(true).write(true).append(true).open(&grub_path)?;

            // Write the modified content back to the file
            file.write_all(format!("\n\n{}", &gconf).as_bytes())?;

            // Add recovery mode
            if Path::new(&format!("/.snapshots/rootfs/snapshot-{}", rec_tmp)).is_file() {
                // Remove END of 41_custom
                let mut contents = String::new();
                let mut sfile = File::open(&grub_path)?;
                sfile.read_to_string(&mut contents)?;
                let modified_grub_contents = contents.replace("### END /etc/grub.d/41_custom ###", "");
                let mut nfile = File::create(&grub_path)?;
                nfile.write_all(modified_grub_contents.as_bytes())?;

                let first_quote_index = gconf.find('\'').unwrap_or(0);
                let second_quote_index = gconf[first_quote_index + 1..].find('\'').unwrap_or(gconf.len()) + first_quote_index + 1;
                let updated_line = gconf.replace(&gconf[first_quote_index + 1..second_quote_index], "Recovery Mode");

                // Change recovery tmp
                let gconf = updated_line.replace(&format!("@.snapshots_linux/rootfs/snapshot-{}", source_dep),
                                                 &format!("@.snapshots_linux/rootfs/snapshot-{}", rec_tmp));

                // Open the file in read and write mode
                let mut file = OpenOptions::new().read(true).write(true).append(true).open(&grub_path)?;

                // Write the modified content back to the file
                file.write_all(format!("\n\n{}", &gconf).as_bytes())?;
            } else {
                deploy_recovery()?;
            }
        }
    } else {
        deploy_recovery()?;
    }

    // Umount boot partition
    umount2(Path::new(&format!("{}", tmp_boot.path().as_os_str().to_str().unwrap())),
            MntFlags::MNT_DETACH)?;

    Ok(target_dep)
}

// Update boot
pub fn update_boot(snapshot: &str, secondary: bool) -> Result<(), Error> {
    // Path to grub directory
    let grub = get_grub().unwrap();

    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists()? {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot update boot as snapshot {} doesn't exist.", snapshot)));

    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists()? {
        // Make sure snapshot is not in use by another ash process
        return Err(
            Error::new(
                ErrorKind::Unsupported,
                format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                        snapshot,snapshot)));

    } else {
        // Get tmp
        let tmp = get_tmp();
        let tmp = get_aux_tmp(tmp, secondary);

        // Partition path
        let part = get_part();

        // Prepare for update
        prepare(snapshot)?;

        // Remove grub configurations older than 30 days
        if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/boot/{}/BAK/", snapshot,grub)).try_exists()? && snapshot != "0" {
            delete_old_grub_files(&format!("/.snapshots/rootfs/snapshot-chr{}/boot/{}", snapshot,grub).as_str())?;
        }

        // Get current time
        let time = Local::now().naive_local();
        let formatted = time.format("%Y%m%d-%H%M%S").to_string();

        // Copy backup
        if snapshot != "0" {
            copy(format!("/.snapshots/rootfs/snapshot-chr{}/boot/{}/grub.cfg", snapshot,grub),
                 format!("/.snapshots/rootfs/snapshot-chr{}/boot/{}/BAK/grub.cfg.{}", snapshot,grub,formatted))?;
        }

        // Run update commands in chroot
        let distro_name = detect_distro::distro_name(snapshot);
        let mkconfig = format!("/usr/sbin/grub-mkconfig {} -o /boot/{}/grub.cfg", part,grub);
        let sed_snap = format!("sed -i 's|snapshot-chr{}|snapshot-{}|g' /boot/{}/grub.cfg", snapshot,tmp,grub);
        let sed_distro = format!("sed -i '0,\\|{}| s||{} snapshot {}|' /boot/{}/grub.cfg",
                                 distro_name,distro_name,snapshot,grub);
        let cmds = [mkconfig, sed_snap, sed_distro];
        for cmd in cmds {
            chroot_exec(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot), &cmd)?;
        }

        // Finish the update
        post_transactions(snapshot)?;
    }
    Ok(())
}
