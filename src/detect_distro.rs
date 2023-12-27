use std::fs::{read_dir, read_to_string};

// Capitalize first letter
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    if let Some(first_char) = chars.next() {
        let capitalized = first_char.to_uppercase().chain(chars).collect();
        return capitalized;
    }
    String::new()
}

// Detect distro ID
pub fn distro_id() -> String {
    let mut distro_id = String::new();
    // Check if /etc/lsb-release exists and contains DISTRIB_ID
    if let Ok(file) = read_to_string("/etc/lsb-release") {
        for line in file.lines() {
            if line.starts_with("DISTRIB_ID=") {
                distro_id = line.split('=').nth(1).unwrap()
                                                  .to_lowercase()
                                                  .trim_matches('"')
                                                  .to_string();
                break;
            }
        }
    }

    // If /etc/lsb-release check fails, check if /etc/os-release exists and contains ID
    if distro_id.is_empty() {
        if let Ok(file) = read_to_string("/etc/os-release") {
            for line in file.lines() {
                if line.starts_with("ID=") {
                    distro_id = line.split('=').nth(1).unwrap()
                                                      .to_lowercase()
                                                      .trim_matches('"')
                                                      .to_string();
                    break;
                }
            }
        }
    }

    // If both checks fail, loop through all files in /etc/ and check for -release files
    if distro_id.is_empty() {
        for entry in read_dir("/etc").unwrap() {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && path.to_str().unwrap().ends_with("-release") {
                    distro_id = path.file_stem().unwrap()
                                                .to_str().unwrap()
                                                         .to_lowercase().to_owned().to_string()
                                                                                   .split('-')
                                                                                   .nth(0)
                                                                                   .unwrap()
                                                                                   .to_string();
                    break;
                }
            }
        }
    }
    distro_id
}

// Detect distro name
pub fn distro_name() -> String {
    let mut distro_name = String::new();
    // If /etc/lsb-release check fails, check if /etc/os-release exists and contains ID
    if let Ok(file) = read_to_string("/etc/os-release") {
        for line in file.lines() {
            if line.starts_with("NAME=") {
                distro_name = line.split('=').nth(1).unwrap().trim_matches('"').to_string();
                break;
            }
        }
    }

    // Check if /etc/lsb-release exists and contains DISTRIB_NAME
    if distro_name.is_empty() {
        if let Ok(file) = read_to_string("/etc/lsb-release") {
            for line in file.lines() {
                if line.starts_with("DISTRIB_DESCRIPTION=") {
                    distro_name = line.split('=').nth(1).unwrap().split(' ').nth(0).unwrap().trim_matches('"').to_string();
                    break;
                }
            }
        }
    }

    // If both checks fail, loop through all files in /etc/ and check for -release files
    if distro_name.is_empty() {
        for entry in read_dir("/etc").unwrap() {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && path.to_str().unwrap().ends_with("-release") {
                    let distro_name_from_path = path.file_stem().unwrap()
                                                                .to_str().unwrap()
                                                                         .to_lowercase().to_owned().to_string()
                                                                                                   .split('-')
                                                                                                   .nth(0)
                                                                                                   .unwrap()
                                                                                                   .to_string();
                    distro_name = capitalize(&distro_name_from_path);
                    break;
                }
            }
        }
    }
    distro_name
}
