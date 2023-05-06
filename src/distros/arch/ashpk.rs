use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;
use crate::{check_mutability, chr_delete, get_tmp, write_desc};

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

// Noninteractive update  //REVIEW
//pub fn auto_upgrade(snapshot: &str) {
    //sync_time(); // Required in virtualbox, otherwise error in package db update
    //prepare(snapshot);
    //if !aur_check() {
        //let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           //.args(["pacman", "--noconfirm", "-Syyu"]).output().unwrap();
        //if excode.status.success() {
            //post_transactions(snapshot);
            //Command::new("echo").args(["0", ">"]).arg("/.snapshots/ash/upstate").output().unwrap();
            //Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").output().unwrap();
        //} else {
            //chr_delete(snapshot);
            //Command::new("echo").args(["1", ">"]).arg("/.snapshots/ash/upstate").output().unwrap();
            //Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").output().unwrap();
        //}
    //} else {
        //let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           //.args(["su", "aur", "-c", "paru", "--noconfirm", "-Syy"])
                                           //.output().unwrap();
        //if excode.status.success() {
            //post_transactions(snapshot);
            //Command::new("echo").args(["0", ">"]).arg("/.snapshots/ash/upstate").output().unwrap();
            //Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").output().unwrap();
        //} else {
            //chr_delete(snapshot);
            //Command::new("echo").args(["1", ">"]).arg("/.snapshots/ash/upstate").output().unwrap();
            //Command::new("echo").args(["$(date)", ">>"]).arg("/.snapshots/ash/upstate").output().unwrap();
        //}
    //}
//}

// Copy cache of downloaded packages to shared //REVIEW
//pub fn cache_copy(snapshot: &str) {
    //Command::new("cp").args(["-n", "-r", "--reflink=auto"])
                      //.arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/cache/pacman/pkg/.", snapshot))
                      //.arg("/var/cache/pacman/pkg/").status().unwrap();
//}

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

// Make a node mutable //REVIEW
//pub fn immutability_disable(snapshot: &str) {
    //if snapshot != "0" {
        //if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
            //eprintln!("Snapshot {} doesn't exist.", snapshot);
        //} else {
            //if check_mutability(snapshot) {
                //println!("Snapshot {} is already mutable.", snapshot);
            //} else {
                //Command::new("btrfs").args(["property", "set", "-ts"])
                                     //.arg(format!("/.snapshots/rootfs/snapshot-{} ro false", snapshot))
                                     //.status().unwrap();
                //Command::new("touch").arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", snapshot))
                                 //.status().unwrap();
                //println!("Snapshot {} successfully made mutable.", snapshot);
                //write_desc(snapshot, "MUTABLE");
            //}
        //}
    //} else {
        //eprintln!("Snapshot 0 (base) should not be modified.");
    //}
//}

//Make a node immutable //REVIEW
//pub fn immutability_enable(snapshot: &str) {
   // if snapshot != "0" {
        //if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
            //eprintln!("Snapshot {} doesn't exist.", snapshot);
        //} else {
            //if check_mutability(snapshot) {
                //println!("Snapshot {} is already mutable.", snapshot);
            //} else {
                //Command::new("btrfs").args(["property", "set", "-ts"])
                                     //.arg(format!("/.snapshots/rootfs/snapshot-{} ro false", snapshot))
                                     //.status().unwrap();
                //Command::new("touch").arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", snapshot))
                                 //.status().unwrap();
                //println!("Snapshot {} successfully made mutable.", snapshot);
                //Command::new("sed").args(["-i", "'s|", "MUTABLE", "||g'"])
                                   //.arg(format!("/.snapshots/ash/snapshots/{}-desc", snapshot))
                                   //.status().unwrap();
            //}
        //}
    //} else {
        //eprintln!("Snapshot 0 (base) should not be modified.");
    //}
//}

// Delete init system files (Systemd, OpenRC, etc.) //REVIEW
//pub fn init_system_clean(snapshot: &str, from: &str) {
    //if from == "prepare"{
        //Command::new("rm").arg("-rf")
                          //.arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/lib/systemd/*", snapshot))
                          //.status().unwrap();
    //} else if from == "deploy" {
        //Command::new("rm").args(["-rf", "/var/lib/systemd/*"])
                          //.status().unwrap();
        //Command::new("rm").arg("-rf")
                          //.arg(format!("/.snapshots/rootfs/snapshot-{}/var/lib/systemd/*", snapshot))
                          //.status().unwrap();
    //}
//}

// Copy init system files (Systemd, OpenRC, etc.) to shared //REVIEW
//pub fn init_system_copy(snapshot: &str, from: &str) {
    //if from == "post_transactions" {
        //Command::new("rm").args(["-rf" ,"/var/lib/systemd/*"])
                          //.status().unwrap();
        //Command::new("cp").args(["-r", "--reflink=auto",])
                          //.arg(format!("/.snapshots/rootfs/snapshot-{}/var/lib/systemd/.", snapshot))
                          //.arg("/var/lib/systemd/")
                          //.status().unwrap();
    //}
//}

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

// Get list of packages installed in a snapshot //REVIEW
//pub fn pkg_list(chr: &str, snap: &str) {
    //let excode = String::from_utf8::new(Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}{}", chr,snap))
                          //.args(["pacman", "-Qq"])
                          //.status().unwrap().to_string());
    //excode.to.strip().split("\n");
//}

// Prepare snapshot to chroot dir to install or chroot into
//pub fn prepare(snapshot: &str) {
    //chr_delete(snapshot)
    //os.system(f"btrfs sub snap /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-chr{snapshot}{DEBUG}")
  //# Pacman gets weird when chroot directory is not a mountpoint, so the following mount is necessary ### REVIEW
    //os.system(f"mount --bind --make-slave /.snapshots/rootfs/snapshot-chr{snapshot} /.snapshots/rootfs/snapshot-chr{snapshot}{DEBUG}")
    //os.system(f"mount --rbind --make-rslave /dev /.snapshots/rootfs/snapshot-chr{snapshot}/dev{DEBUG}")
    //os.system(f"mount --bind --make-slave /home /.snapshots/rootfs/snapshot-chr{snapshot}/home{DEBUG}")
    //os.system(f"mount --rbind --make-rslave /proc /.snapshots/rootfs/snapshot-chr{snapshot}/proc{DEBUG}")
    //os.system(f"mount --bind --make-slave /root /.snapshots/rootfs/snapshot-chr{snapshot}/root{DEBUG}")
    //os.system(f"mount --rbind --make-rslave /run /.snapshots/rootfs/snapshot-chr{snapshot}/run{DEBUG}")
    //os.system(f"mount --rbind --make-rslave /sys /.snapshots/rootfs/snapshot-chr{snapshot}/sys{DEBUG}")
    //os.system(f"mount --rbind --make-rslave /tmp /.snapshots/rootfs/snapshot-chr{snapshot}/tmp{DEBUG}")
    //os.system(f"mount --bind --make-slave /var /.snapshots/rootfs/snapshot-chr{snapshot}/var{DEBUG}")
  //# File operations for snapshot-chr
    //os.system(f"btrfs sub snap /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-chr{snapshot}{DEBUG}")
    //os.system(f"btrfs sub snap /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-chr{snapshot}{DEBUG}")
    //os.system(f"cp -r --reflink=auto /.snapshots/boot/boot-chr{snapshot}/. /.snapshots/rootfs/snapshot-chr{snapshot}/boot{DEBUG}")
    //os.system(f"cp -r --reflink=auto /.snapshots/etc/etc-chr{snapshot}/. /.snapshots/rootfs/snapshot-chr{snapshot}/etc{DEBUG}") ### btrfs sub snap etc-{snapshot} to etc-chr-{snapshot} not needed before this?
    //init_system_clean(snapshot, "prepare")
    //os.system(f"cp /etc/machine-id /.snapshots/rootfs/snapshot-chr{snapshot}/etc/machine-id")
    //os.system(f"mkdir -p /.snapshots/rootfs/snapshot-chr{snapshot}/.snapshots/ash && cp -f /.snapshots/ash/fstree /.snapshots/rootfs/snapshot-chr{snapshot}/.snapshots/ash/")
  //# Special mutable directories
    //options = snapshot_config_get(snapshot)
    //mutable_dirs = options["mutable_dirs"].split(',').remove('')
    //mutable_dirs_shared = options["mutable_dirs_shared"].split(',').remove('')
    //if mutable_dirs:
        //for mount_path in mutable_dirs:
            //os.system(f"mkdir -p /.snapshots/mutable_dirs/snapshot-{snapshot}/{mount_path}")
            //os.system(f"mkdir -p /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path}")
            //os.system(f"mount --bind /.snapshots/mutable_dirs/snapshot-{snapshot}/{mount_path} /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path}")
    //if mutable_dirs_shared:
        //for mount_path in mutable_dirs_shared:
            //os.system(f"mkdir -p /.snapshots/mutable_dirs/{mount_path}")
            //os.system(f"mkdir -p /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path}")
            //os.system(f"mount --bind /.snapshots/mutable_dirs/{mount_path} /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path}")
//  # Important: Do not move the following line above (otherwise error)
    //os.system(f"mount --bind --make-slave /etc/resolv.conf /.snapshots/rootfs/snapshot-chr{snapshot}/etc/resolv.conf{DEBUG}")
//}

// Post transaction function, copy from chroot dirs back to read only snapshot dir
//pub fn post_transactions(snapshot: &str) {
    //tmp = get_tmp()
  //# Some operations were moved below to fix hollow functionality ###
  //# File operations in snapshot-chr
//#    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{snapshot}{DEBUG}") ### REVIEW # Moved to a few lines below
    //os.system(f"rm -rf /.snapshots/boot/boot-chr{snapshot}/*{DEBUG}")
    //os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/boot/. /.snapshots/boot/boot-chr{snapshot}{DEBUG}")
    //os.system(f"rm -rf /.snapshots/etc/etc-chr{snapshot}/*{DEBUG}")
    //os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/etc/. /.snapshots/etc/etc-chr{snapshot}{DEBUG}")
  //# Keep package manager's cache after installing packages. This prevents unnecessary downloads for each snapshot when upgrading multiple snapshots
    //cache_copy(snapshot, "post_transactions")
    //os.system(f"btrfs sub del /.snapshots/boot/boot-{snapshot}{DEBUG}")
    //os.system(f"btrfs sub del /.snapshots/etc/etc-{snapshot}{DEBUG}")
    //os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{snapshot}{DEBUG}")
    //if os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}/usr/share/ash/mutable"):
        //immutability = ""
    //else:
        //immutability = "-r"
    //os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-chr{snapshot} /.snapshots/boot/boot-{snapshot}{DEBUG}")
    //os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-chr{snapshot} /.snapshots/etc/etc-{snapshot}{DEBUG}")
  //# Copy init system files to shared
    //init_system_copy(tmp, "post_transactions")
    //os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-chr{snapshot} /.snapshots/rootfs/snapshot-{snapshot}{DEBUG}")
  //# ---------------------- fix for hollow functionality ---------------------- #
  //# Unmount in reverse order
    //os.system(f"umount /.snapshots/rootfs/snapshot-chr{snapshot}/etc/resolv.conf{DEBUG}")
    //os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/dev{DEBUG}")
    //os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/home{DEBUG}")
    //os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/proc{DEBUG}")
    //os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/root{DEBUG}")
    //os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/run{DEBUG}")
    //os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/sys{DEBUG}")
    //os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}{DEBUG}")
  //# Special mutable directories
    //options = snapshot_config_get(snapshot)
    //mutable_dirs = options["mutable_dirs"].split(',').remove('')
    //mutable_dirs_shared = options["mutable_dirs_shared"].split(',').remove('')
    //if mutable_dirs:
        //for mount_path in mutable_dirs:
            //os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path}{DEBUG}")
    //if mutable_dirs_shared:
        //for mount_path in mutable_dirs_shared:
            //os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path}{DEBUG}")
  //# ---------------------- fix for hollow functionality ---------------------- #
    //chr_delete(snapshot)
//}

// Refresh snapshot atomic-operation //REVIEW
//pub fn refresh_helper(snapshot: &str) {
    //Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          //.args(["pacman", "-Syy"]).status().unwrap();
//}

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
    // defaults here
    let mut options = HashMap::new();
    options.insert(String::from("aur"), String::from("False"));
    options.insert(String::from("mutable_dirs"), String::new());
    options.insert(String::from("mutable_dirs_shared"), String::new());

    if !Path::new(&format!("/.snapshots/etc/etc-{}/ash.conf", snap())).try_exists().unwrap() {
        return options;
    } else {
        let optfile = File::open(format!("/.snapshots/etc/etc-{}/ash.conf", snap())).unwrap();
        let reader = BufReader::new(optfile);

        for line in reader.lines() {
            let line = line.unwrap();
            if line.contains('#') {
                // Everything after '#' is a comment
                line.split('#').next().unwrap();
            }
            // Skip line if there's no option set
            if line.contains("::") {
                // Split options with '::'
                let (left, right) = line.split_once("::").unwrap();
                // Remove newline here
                options.insert(left.to_string(), right.trim_end().to_string());
            }
        }
        return options;
    }
}

// Show diff of packages between 2 snapshots //REVIEW
//pub fn snapshot_diff(snap1: &str, snap2: &str) {
    //if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snap1)).try_exists().unwrap() {
        //println!("Snapshot {} not found.", snap1);
    //} else if !Path::new(format!("/.snapshots/rootfs/snapshot-{}", snap2)).try_exists().unwrap() {
        //println!("Snapshot {} not found.", snap2);
    //} else {
        //Command::new("bash").args(["-c", "\'diff", "<(ls"])
                            //.arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/db/local", snap1))
                            //.arg("<(ls")
                            //.arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/db/local", snap2))
                            //.args(["|", "grep",])
                            //.arg(format!("'^>\|^<'"))
                            //.args(["|", "sort\'"]).status().unwrap();
    //}
//}

// Sync time //REVIEW
//pub fn sync_time() {
    //Command::new("sudo")
        //.arg("date")
        //.arg("-s")
        //.arg(format!("{}Z",String::from_utf8(Command::new("curl")
                                             //.args(&["--tlsv1.3",
                                                     //"--proto",
                                                     //"=https",
                                                     //"-I",
                                                     //"https://google.com"
                                             //])
                                             //.output()
                                             //.unwrap()
                                             //.stderr
                                             //.split(|&x| x == b'\n')
                                             //.find(|line| line.starts_with(b"Date:"))
                                             //.unwrap_or(&[])
                                             //[6..25]
                                             //.to_vec(),).unwrap())).status().unwrap();
//}

// Uninstall package(s) atomic-operation //REVIEW
//pub fn uninstall_package_helper(snapshot: &str, pkg: &str) {
    //Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          //.args(["pacman", "--noconfirm", "-Rns"])
                          //.arg(format!("{}", pkg)).status().unwrap();
//}

// Upgrade snapshot atomic-operation //REVIEW
//pub fn upgrade_helper(snapshot: &str) -> String {
    //prepare(snapshot);
    //if !aur_check() {
        //let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           //.args(["pacman", "-Syyu"])
                                           //.status().unwrap().to_string();
    //} else {
        //let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                           //.args(["su", "aur", "-c", "'paru", "-Syyu'"]).status().unwrap().to_string();
    //}
//}
