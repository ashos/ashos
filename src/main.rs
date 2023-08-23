extern crate lib;
mod cli;

use cli::*;
use lib::*;
use nix::unistd::Uid;
// Directexplicitories
// All snapshots share one /var
// Global boot is always at @boot
// *-chr                             : temporary directories used to chroot into snapshot or copy snapshots around
// *-deploy and *-deploy-aux         : temporary directories used to boot deployed snapshot
// *-deploy[-aux]-secondary          : temporary directories used to boot secondary deployed snapshot
// /.snapshots/ash/part              : root partition uuid
// /.snapshots/ash/snapshots/*-desc  : descriptions
// /.snapshots/boot/boot-*           : individual /boot for each snapshot
// /.snapshots/etc/etc-*             : individual /etc for each snapshot
// /.snapshots/rootfs/snapshot-*     : snapshots
// /.snapshots/tmp                   : temporary directory
// /usr/sbin/ash                     : ash binary file location
// /usr/share/ash                    : files that store current snapshot info
// /usr/share/ash/db                 : package database
// /use/share/ash/profiles           : default desktop environments profiles path
// /var/lib/ash(/fstree)             : ash files, stores fstree, symlink to /.snapshots/ash/fstree

fn main() {
    if !Uid::effective().is_root() {
        eprintln!("sudo/doas is required to run ash!");
    } else if chroot_check() {
        eprintln!("Please don't use ash inside a chroot!");
    } else {
        // Call cli matches
        let matches = cli().get_matches();
        // Call relevant functions
        match matches.subcommand() {
            // Auto upgrade
            Some(("auto-upgrade", auto_upgrade_matches)) => {
                // Get snapshot value
                let snapshot = if auto_upgrade_matches.contains_id("SNAPSHOT") {
                    let snap = auto_upgrade_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run noninteractive_update
                noninteractive_update(&snapshot).unwrap();
            }
            // Base update
            Some(("base-update", _matches)) => {
                // Run upgrade(0)
                let run = upgrade("0", true);
                match run {
                    Ok(_) => {},
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Boot update command
            Some(("boot", boot_matches)) => {
                // Get snapshot value
                let snapshot = if boot_matches.contains_id("SNAPSHOT") {
                    let snap = boot_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run update-boot
                let run = update_boot(&snapshot, false);
                match run {
                    Ok(_) => println!("Bootloader updated successfully."),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Branch
            Some(("branch", branch_matches)) => {
                // Get snapshot value
                let snapshot = if branch_matches.contains_id("SNAPSHOT") {
                    let snap = branch_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get desc value
                let desc = if branch_matches.contains_id("DESCRIPTION") {
                    let desc = branch_matches.get_one::<String>("DESCRIPTION").map(|s| s.as_str()).unwrap().to_string();
                    desc
                } else {
                    let desc = String::new();
                    desc
                };

                // Run barnch_create
                let run = branch_create(&snapshot, &desc);
                match run {
                    Ok(snapshot_num) => println!("Branch {} added under snapshot {}.", snapshot_num,snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Check update
            Some(("check", _matches)) => {
                // Run check_update
                check_update().unwrap();
            }
            // Chroot
            Some(("chroot", chroot_matches)) => {
                // Get snapshot value
                let snapshot  = if chroot_matches.contains_id("SNAPSHOT") {
                    let snap = chroot_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get cmd value
                let cmd: Vec<String> = Vec::new();

                // Run chroot
                let run = chroot(&snapshot, cmd);
                match run {
                    Ok(_) => (),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Clone
            Some(("clone", clone_matches)) => {
                // Get snapshot value
                let snapshot = if clone_matches.contains_id("SNAPSHOT") {
                    let snap = clone_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get desc value
                let desc = if clone_matches.contains_id("DESCRIPTION") {
                    let desc = clone_matches.get_one::<String>("DESCRIPTION").map(|s| s.as_str()).unwrap().to_string();
                    desc
                } else {
                    let desc = String::new();
                    desc
                };

                // Run clone_as_tree
                let run = clone_as_tree(&snapshot, &desc);
                match run {
                    Ok(snapshot_num) => println!("Tree {} cloned from {}.", snapshot_num,snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Clone a branch
            Some(("clone-branch", clone_branch_matches)) => {
                // Get snapshot value
                let snapshot = if clone_branch_matches.contains_id("SNAPSHOT") {
                    let snap = clone_branch_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run clone_branch
                let run = clone_branch(&snapshot);
                match run {
                    Ok(snapshot_num) => println!("Branch {} added to parent of {}.", snapshot_num,snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Clone recursively
            Some(("clone-tree", clone_tree_matches)) => {
                // Get snapshot value
                let snapshot = if clone_tree_matches.contains_id("SNAPSHOT") {
                    let snap = clone_tree_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run clone_recursive
                let run = clone_recursive(&snapshot);
                match run {
                    Ok(_) => println!("Snapshot {} was cloned recursively.", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Clone under a branch
            Some(("clone-under", clone_under_matches)) => {
                // Get snapshot value
                let snap = clone_under_matches.get_one::<i32>("SNAPSHOT").unwrap();
                let snapshot = format!("{}", snap);

                // Get branch value
                let branch_i32 = clone_under_matches.get_one::<i32>("BRANCH").unwrap();
                let branch = format!("{}", branch_i32);

                // Run clone_under
                let run = clone_under(&snapshot, &branch);
                match run {
                    Ok(snapshot_num) => println!("Branch {} added to parent of {}.", snapshot_num,snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Current snapshot
            Some(("current", _matches)) => {
                // Run get_current_snapshot
                println!("{}", get_current_snapshot());
            }
            // Delete
            Some(("del", del_matches)) => {
                // Get snapshot value
                let snapshots: Vec<_> = del_matches.get_many::<i32>("SNAPSHOT").unwrap().map(|s| format!("{}", s)).collect();

                // Optional values
                let quiet = del_matches.get_flag("quiet");
                let nuke = del_matches.get_flag("nuke");

                // Run delelte_node
                let run = delete_node(&snapshots, quiet, nuke);
                match run {
                    Ok(_) => println!("Snapshot {:?} removed.", snapshots),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Deploy
            Some(("deploy", deploy_matches)) => { //REVIEW
                // Get snapshot value
                let snapshot = if deploy_matches.contains_id("SNAPSHOT") {
                    let snap = deploy_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Optional value
                let secondary = deploy_matches.get_flag("secondary");

                // Run deploy
                let run = deploy(&snapshot, secondary);
                match run {
                    Ok(_) => println!("Snapshot {} deployed to '/'.", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Description
            Some(("desc", desc_matches)) => {
                // Get snapshot value
                let snapshot = if desc_matches.contains_id("SNAPSHOT") {
                    let snap = desc_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get desc value
                let desc = desc_matches.get_one::<String>("DESCRIPTION").map(|s| s.as_str()).unwrap().to_string();

                // Run write_desc
                write_desc(&snapshot, &desc, true).unwrap();
            }
            // Diff two snapshots
            Some(("diff", diff_matches)) => {
                // Get snapshot one value
                let snap1 = diff_matches.get_one::<i32>("SNAPSHOT-1").unwrap();
                let snapshot1 = format!("{}", snap1);

                // Get snapshot two value
                let snap2 = diff_matches.get_one::<i32>("SNAPSHOT-2").unwrap();
                let snapshot2 = format!("{}", snap2);

                // Run diff
                diff(&snapshot1, &snapshot2);
            }
            // Switch distros
            Some(("dist", _matches)) => { //REVIEW
                // Run switch_distro
                switch_distro().unwrap();
            }
            // Edit Ash configuration
            Some(("edit", edit_matches)) => {
                // Get snapshot value
                let snapshot = if edit_matches.contains_id("SNAPSHOT") {
                    let snap = edit_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run snapshot_config_edit
                snapshot_config_edit(&snapshot).unwrap();
            }
            // etc update
            Some(("etc-update", _matches)) => {
                // Run etc-update
                update_etc().unwrap();
            }
            // Fix db commands
            Some(("fixdb", fixdb_matches)) => {
                // Get snapshot value
                let snapshot = if fixdb_matches.contains_id("SNAPSHOT") {
                    let snap = fixdb_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                //Run fixdb
                let run = fixdb(&snapshot);
                match run {
                    Ok(_) => {
                        if post_transactions(&snapshot).is_ok() {
                            println!("Snapshot {}'s package manager database fixed successfully.", snapshot);
                        } else {
                            eprintln!("Fixing package manager database failed.");
                        }
                    },
                    Err(e) => {
                        chr_delete(&snapshot).unwrap();
                        eprintln!("Fixing package manager database failed due to: {}", e);
                    },
                }
            }
            // Switch to Windows (semi plausible deniability)
            Some(("hide", _matches)) => { //REVIEW
                // Run switch_to_windows
                switch_to_windows();
            }
            // Hollow a node
            Some(("hollow", hollow_matches)) => {
                // Get snapshot value
                let snapshot = if hollow_matches.contains_id("SNAPSHOT") {
                    let snap = hollow_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run hollow
                let run = hollow(&snapshot);
                match run {
                    Ok(_) => println!("Snapshot {} hollow operation succeeded. Please reboot!", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Immutability disable
            Some(("immdis", immdis_matches)) => {
                // Get snapshot value
                let snapshot = if immdis_matches.contains_id("SNAPSHOT") {
                    let snap = immdis_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run immutability_disable
                let run = immutability_disable(&snapshot);
                match run {
                    Ok(_) => println!("Snapshot {} successfully made mutable.", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Immutability enable
            Some(("immen", immen_matches)) => {
                // Get snapshot value
                let snapshot = if immen_matches.contains_id("SNAPSHOT") {
                    let snap = immen_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run immutability_enable
                let run = immutability_enable(&snapshot);
                match run {
                    Ok(_) => println!("Snapshot {} successfully made immutable.", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Install command
            Some(("install", install_matches)) => {
                // Get snapshot value
                let snapshot = if install_matches.contains_id("SNAPSHOT") {
                    let snap = install_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get pkgs value
                let pkgs = if install_matches.contains_id("PACKAGE") {
                    let pkgs: Vec<String> = install_matches.get_many::<String>("PACKAGE").unwrap().map(|s| format!("{}", s)).collect();
                    pkgs
                } else {
                    let pkgs: Vec<String> = Vec::new();
                    pkgs
                };

                // Get profile value
                let profile = if install_matches.contains_id("PROFILE") {
                    let profile = install_matches.get_many::<String>("PROFILE").unwrap().map(|s| format!("{}", s)).collect();
                    profile
                } else {
                    let profile = String::new();
                    profile
                };

                // Get user_profile value
                let user_profile = if install_matches.contains_id("USER_PROFILE") {
                    let user_profile = install_matches.get_many::<String>("USER_PROFILE").unwrap().map(|s| format!("{}", s)).collect();
                    user_profile
                } else {
                    let user_profile = String::new();
                    user_profile
                };

                // Optional values
                let live = install_matches.get_flag("live");
                let noconfirm = install_matches.get_flag("noconfirm");
                let force = install_matches.get_flag("force");
                let  secondary= install_matches.get_flag("secondary");

                // Run install_triage
                install_triage(&snapshot, live, pkgs, &profile, force, &user_profile, noconfirm, secondary).unwrap();
            }
            // Package list
            Some(("list", list_matches)) => {
                // Get snapshot value
                let snapshot = if list_matches.contains_id("SNAPSHOT") {
                    let snap = list_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // chr value
                let chr = "";

                // Run list
                let run = list(&snapshot, chr);
                for pkg in run {
                    println!("{}", pkg);
                }
            }
            // Live chroot
            Some(("live-chroot", _matches)) => {
                // Run live_unlock
                live_unlock().unwrap();
            }
            // New
            Some(("new", new_matches)) => {
                // Get desc value
                let desc = if new_matches.contains_id("DESCRIPTION") {
                    let desc = new_matches.get_one::<String>("DESCRIPTION").map(|s| s.as_str()).unwrap().to_string();
                    desc
                } else {
                    let desc = String::new();
                    desc
                };

                // Run snapshot_base_new
                let run = snapshot_base_new(&desc);
                match run {
                    Ok(snap_num) => println!("New tree {} created.", snap_num),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Refresh
            Some(("refresh", refresh_matches)) => {
                // Get snapshot value
                let snapshot = if refresh_matches.contains_id("SNAPSHOT") {
                    let snap = refresh_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run refresh
                refresh(&snapshot).unwrap();
            }
            // Rollback
            Some(("rollback", _matches)) => {
                // Run rollback
                rollback().unwrap();
            }
            // Chroot run
            Some(("run", run_matches)) => {
                // Get snapshot value
                let snapshot = if run_matches.contains_id("SNAPSHOT") {
                    let snap = run_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get cmds value
                let cmds: Vec<String> = run_matches.get_many::<String>("COMMAND").unwrap().map(|s| format!("{}", s)).collect();

                // Run chroot
                let run = chroot(&snapshot, cmds);
                match run {
                    Ok(_) => (),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Subvolumes list
            Some(("sub", _matches)) => {
                // Run list_subvolumes
                list_subvolumes();
            }
            // Tree sync
            Some(("sync", sync_matches)) => { //REVIEW
                // Get treename value
                let treename = if sync_matches.contains_id("TREENAME") {
                    let snap = sync_matches.get_one::<i32>("TREENAME").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Optional values
                let live = sync_matches.get_flag("live");
                let force_offline = sync_matches.get_flag("force");

                // Run tree_sync
                let run = tree_sync(&treename, force_offline, live);
                match run {
                    Ok(_) => println!("Tree {} synced.", treename),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // tmp (clear tmp)
            Some(("tmp", _matches)) => {
                // Run temp_snapshots_clear
                temp_snapshots_clear().unwrap();
            }
            // Tree
            Some(("tree", _matches)) => {
                //Run tree_show
                tree_show();
            }
            // Tree remove
            Some(("tremove", tremove_matches)) => { //REVIEW
                // Get treename value
                let treename = if tremove_matches.contains_id("TREENAME") {
                    let snap = tremove_matches.get_one::<i32>("TREENAME").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get pkgs value
                let pkgs = if tremove_matches.contains_id("PACKAGE") {
                    let pkgs: Vec<String> = tremove_matches.get_many::<String>("PACKAGE").unwrap().map(|s| format!("{}", s)).collect();
                    pkgs
                } else {
                    let pkgs: Vec<String> = Vec::new();
                    pkgs
                };

                // Get profiles value
                let profiles = if tremove_matches.contains_id("PROFILE") {
                    let profiles: Vec<String> = tremove_matches.get_many::<String>("PROFILE").unwrap().map(|s| format!("{}", s)).collect();
                    profiles
                } else {
                    let profiles: Vec<String> = Vec::new();
                    profiles
                };

                // Get user_profiles value
                let user_profiles = if tremove_matches.contains_id("USER_PROFILE") {
                    let user_profiles: Vec<String> = tremove_matches.get_many::<String>("USER_PROFILE").unwrap().map(|s| format!("{}", s)).collect();
                    user_profiles
                } else {
                    let user_profiles: Vec<String> = Vec::new();
                    user_profiles
                };

                // Run remove_from_tree
                let run = remove_from_tree(&treename, &pkgs, &profiles, &user_profiles);
                match run {
                    Ok(_) => println!("Tree {} updated.", treename),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Tree run
            Some(("trun", trun_matches)) => { //REVIEW
                // Get snapshot value
                let treename = if trun_matches.contains_id("TREENAME") {
                    let snap = trun_matches.get_one::<i32>("TREENAME").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let treename = get_current_snapshot();
                    treename
                };

                // Get cmds value
                let cmds: Vec<String> = trun_matches.get_many::<String>("COMMAND").unwrap().map(|s| format!("{}", s)).collect();

                // Run tree_run
                for cmd in cmds {
                    let run = tree_run(&treename, &cmd);
                    match run {
                        Ok(_) => println!("Tree {} updated.", treename),
                        Err(e) => eprintln!("{}", e),
                    }
                }
            }
            // Tree upgrade
            Some(("tupgrade", tupgrade_matches)) => { //REVIEW
                // Get treename value
                let treename = if tupgrade_matches.contains_id("TREENAME") {
                    let snap = tupgrade_matches.get_one::<i32>("TREENAME").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run tree_upgrade
                let run = tree_upgrade(&treename);
                match run {
                    Ok(_) => println!("Tree {} updated.", treename),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Uninstall package(s) from a snapshot
            Some(("uninstall", uninstall_matches)) => {
                // Get snapshot value
                let snapshot = if uninstall_matches.contains_id("SNAPSHOT") {
                    let snap = uninstall_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get pkgs value
                let pkgs = if uninstall_matches.contains_id("PACKAGE") {
                    let pkgs: Vec<String> = uninstall_matches.get_many::<String>("PACKAGE").unwrap().map(|s| format!("{}", s)).collect();
                    pkgs
                } else {
                    let pkgs: Vec<String> = Vec::new();
                    pkgs
                };

                // Get profile value
                let profile = if uninstall_matches.contains_id("PROFILE") {
                    let profile = uninstall_matches.get_many::<String>("PROFILE").unwrap().map(|s| format!("{}", s)).collect();
                    profile
                } else {
                    let profile = String::new();
                    profile
                };

                // Get user_profile value
                let user_profile = if uninstall_matches.contains_id("USER_PROFILE") {
                    let user_profile = uninstall_matches.get_many::<String>("USER_PROFILE").unwrap().map(|s| format!("{}", s)).collect();
                    user_profile
                } else {
                    let user_profile = String::new();
                    user_profile
                };

                // Optional values
                let live = uninstall_matches.get_flag("live");
                let noconfirm = uninstall_matches.get_flag("noconfirm");

                // Run uninstall_triage
                uninstall_triage(&snapshot, live, pkgs, &profile, &user_profile, noconfirm).unwrap();
            }
            // Unlock a snapshot
            Some(("unlock", unlock_matches)) => {
                // Get snapshot value
                let snapshot = if unlock_matches.contains_id("SNAPSHOT") {
                    let snap = unlock_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run snapshot_unlock
                snapshot_unlock(&snapshot).unwrap();
            }
            // Upgrade a snapshot
            Some(("upgrade", upgrade_matches)) => {
                // Get snapshot value
                let snapshot = if upgrade_matches.contains_id("SNAPSHOT") {
                    let snap = upgrade_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run upgrade
                let run = upgrade(&snapshot, false);
                match run {
                    Ok(_) => {},
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Ash version
            Some(("version", _matches)) => {
                // Run ash_version
                ash_version().unwrap();
            }
            // Which snapshot(s) contain a package
            Some(("whichsnap", whichsnap_matches)) => {
                // Get pkgs value
                let pkgs: Vec<String> = whichsnap_matches.get_many::<String>("PACKAGE").unwrap().map(|s| format!("{}", s)).collect();

                // Run which_snapshot_has
                which_snapshot_has(pkgs);
            }
            // Which deployment is active
            Some(("whichtmp", _matches)) => {
                // Run get_tmp
                println!("{}", get_tmp());
            }
           _=> unreachable!(), // If all subcommands called, anything else is unreachable
        }
    }
}
