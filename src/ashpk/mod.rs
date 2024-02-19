cfg_if::cfg_if! {
    //if #[cfg(feature = "apk")] { // TODO
        // APK package manager
        // pub mod apk
    // } else
    if #[cfg(feature = "apt")] {
        // APT package manager
        pub mod apt;
    } else if #[cfg(feature = "dnf")] {
        // DNF package manager
        pub mod dnf;
    } else if #[cfg(feature = "pacman")] {
        pub mod pacman;
    //} else if #[cfg(feature = "pkgtool")] { // TODO
        // PKGTOOL
        // pub mod pkgtool
    //} else if #[cfg(feature = "portage")] { // TODO
        // Portage package manager
        // pub mod portage
    //}
        // Pacman package manager
    //} else if #[cfg(feature = "xbps")] { // TODO
        // XBPS package manager
        // pub mod xbps
    //}
    }
}
