//! Build-time configuration, set by Meson through `build.rs`.

/// Application ID (`.Devel` suffix for development builds).
pub const APP_ID: &str = env!("AGORA_APP_ID");
/// `development` or `default`.
pub const PROFILE: &str = env!("AGORA_PROFILE");
/// Version from Cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// gettext domain.
pub const GETTEXT_PACKAGE: &str = "agora";
/// Where translations are installed.
pub const LOCALEDIR: &str = env!("AGORA_LOCALEDIR");
/// Where Meson installed the data files; empty for source-tree runs.
pub const PKGDATADIR: &str = env!("AGORA_PKGDATADIR");
/// The GSettings schema compiled by `build.rs`, for source-tree runs.
pub const DEV_SCHEMA_DIR: &str = env!("AGORA_DEV_SCHEMA_DIR");
/// GResource base path.
pub const RESOURCE_BASE: &str = "/io/github/ClickCalickClick/Agora";
/// Schema ID (the same for the Devel build, SPEC §13).
pub const SCHEMA_ID: &str = "io.github.ClickCalickClick.Agora";
/// Repository, shown in About.
pub const REPO_URL: &str = "https://github.com/ClickCalickClick/agora";

/// Whether this is a development build.
pub fn is_devel() -> bool {
    PROFILE == "development"
}
