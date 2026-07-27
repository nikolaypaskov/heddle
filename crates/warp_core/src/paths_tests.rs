use dirs::home_dir;

use super::*;

#[test]
fn test_data_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    // ChannelState, by default, is configured for Channel::Oss.
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(data_dir(), home_dir.join(".heddle-oss"));
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            assert_eq!(data_dir(), home_dir.join(".local/share/heddle"));
        } else if #[cfg(windows)] {
            assert_eq!(data_dir(), home_dir.join("AppData\\Roaming\\heddle\\Heddle\\data"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_config_local_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    // ChannelState, by default, is configured for Channel::Oss.
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(config_local_dir(), home_dir.join(".heddle-oss"));
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            assert_eq!(config_local_dir(), home_dir.join(".config/heddle"));
        } else if #[cfg(windows)] {
            assert_eq!(config_local_dir(), home_dir.join("AppData\\Local\\heddle\\Heddle\\config"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_gui_app_id_maps_oss_tui_to_oss_gui() {
    let gui_app_id = gui_app_id_for_channel(Channel::Oss, AppId::new("dev", "warp", "WarpTui"));

    assert_eq!(gui_app_id.to_string(), "dev.heddle.Heddle");
}

#[test]
fn test_gui_config_and_mcp_paths_resolve_explicit_sources() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    let gui_config_dir = gui_config_local_dir().expect("GUI config path should resolve");

    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(gui_config_dir, home_dir.join(".heddle-oss"));
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            assert_eq!(gui_config_dir, home_dir.join(".config/heddle"));
        } else if #[cfg(windows)] {
            assert_eq!(
                gui_config_dir,
                home_dir.join("AppData\\Local\\heddle\\Heddle\\config")
            );
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }

    assert_eq!(gui_mcp_config_file_path(), warp_home_mcp_config_file_path());
}
#[test]
fn test_warp_home_config_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    let expected_dir_name = match ChannelState::data_profile() {
        Some(data_profile) => format!(".heddle-oss-{data_profile}"),
        None => ".heddle-oss".to_string(),
    };

    assert_eq!(
        warp_home_config_dir(),
        Some(home_dir.join(expected_dir_name))
    );
}

#[test]
fn test_warp_home_skills_and_mcp_paths() {
    let Some(config_dir) = warp_home_config_dir() else {
        panic!("Should be able to compute Warp home config directory");
    };

    assert_eq!(warp_home_skills_dir(), Some(config_dir.join("skills")));
    assert_eq!(
        warp_home_mcp_config_file_path(),
        Some(config_dir.join(".mcp.json"))
    );
}

#[test]
fn test_tui_mcp_config_path_is_separate_from_gui() {
    let tui_mcp_path = tui_mcp_config_file_path();

    assert_eq!(tui_mcp_path, tui_config_local_dir().join(".mcp.json"));
    assert_ne!(
        Some(tui_mcp_path),
        warp_home_mcp_config_file_path(),
        "GUI and TUI MCP configuration must remain isolated"
    );
}
#[test]
fn test_cache_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    // ChannelState, by default, is configured for Channel::Oss.
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(cache_dir(), home_dir.join("Library/Application Support/dev.heddle.Heddle"));
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            assert_eq!(cache_dir(), home_dir.join(".cache/heddle"));
        } else if #[cfg(windows)] {
            assert_eq!(cache_dir(), home_dir.join("AppData\\Local\\heddle\\Heddle\\cache"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_state_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    cfg_if::cfg_if! {
        // ChannelState, by default, is configured for Channel::Oss.
        if #[cfg(target_os = "macos")] {
            assert_eq!(state_dir(), home_dir.join("Library/Application Support/dev.heddle.Heddle"));
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            assert_eq!(state_dir(), home_dir.join(".local/state/heddle"));
        } else if #[cfg(windows)] {
            assert_eq!(state_dir(), home_dir.join("AppData\\Local\\heddle\\Heddle\\data"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_tui_state_dir_is_tui_subdir_of_gui_state_base() {
    let tui_dir = tui_state_dir();
    assert_eq!(tui_dir.file_name(), Some(std::ffi::OsStr::new("tui")));

    // The TUI state dir must be a direct `tui` child of the same base
    // directory that holds the GUI's SQLite database (the secure state dir
    // when available, otherwise the plain state dir), so the two front-ends
    // keep sibling — never shared — databases.
    let gui_state_base = secure_state_dir().unwrap_or_else(state_dir);
    assert_eq!(tui_dir.parent(), Some(gui_state_base.as_path()));
}

#[test]
fn test_project_path_for_warp_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "warp", "Warp"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.warp.Warp");
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            assert_eq!(project_dirs.project_path(), "warp-terminal");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "warp\\Warp");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_project_path_for_warp_dev_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "warp", "WarpDev"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.warp.WarpDev");
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            assert_eq!(project_dirs.project_path(), "warp-terminal-dev");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "warp\\WarpDev");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_project_path_for_oss_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "heddle", "Heddle"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.heddle.Heddle");
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            // Linux ignores the qualifier and organization entirely: the project path is just
            // the lowercased application name. `AppId::new("dev", "heddle", "Heddle")` therefore
            // yields "heddle", not the pre-rename "warp-oss".
            assert_eq!(project_dirs.project_path(), "heddle");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "heddle\\Heddle");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

/// Heddle must store state in its OWN directory, whether or not Warp is installed.
///
/// Upstream nests state inside the app group container. On this fork that resolved only when Warp
/// happened to be installed -- the container `2BBY89MBSN.dev.warp` exists on those machines and is
/// owned by the user, so the path was usable even though this bundle no longer requests the
/// entitlement. The consequence was that one build stored data in two different places depending
/// on whether a *different product* was present, and the location could move under the user if
/// they installed or uninstalled Warp later, taking their history with it.
///
/// This asserts the property that makes all four of those cases identical: there is no secure
/// state directory, so every caller falls through to `state_dir()`.
#[test]
fn state_never_lives_in_another_vendors_container() {
    assert!(
        secure_state_dir().is_none(),
        "secure_state_dir must be None so state cannot land in Warp's app group container"
    );

    let dir = state_dir();
    assert!(
        !dir.to_string_lossy().contains("Group Containers"),
        "state_dir must not resolve inside an app group container, got {}",
        dir.display()
    );
    assert!(
        !dir.to_string_lossy().to_lowercase().contains("dev.warp"),
        "state_dir must not sit under a Warp-owned path, got {}",
        dir.display()
    );
}

/// The TUI's state directory inherits the same property, since it is derived from the GUI's.
#[test]
fn tui_state_also_avoids_the_vendor_container() {
    let dir = tui_state_dir();
    assert!(
        !dir.to_string_lossy().contains("Group Containers"),
        "tui_state_dir must not resolve inside an app group container, got {}",
        dir.display()
    );
}
