//! Agora: an unofficial Reddit client for GNOME.

mod application;
mod config;
mod i18n;
mod render;
mod stores;
mod ui;
mod util;

use gettextrs::LocaleCategory;
use gtk::prelude::*;
use gtk::{gio, glib};

use self::application::AgoraApplication;

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_env("AGORA_LOG").unwrap_or_else(|_| "warn".into()))
        .init();

    // SAFETY: called first in main, before any other thread exists.
    unsafe { gettextrs::setlocale(LocaleCategory::LcAll, "") };
    let _ = gettextrs::bindtextdomain(config::GETTEXT_PACKAGE, config::LOCALEDIR);
    let _ = gettextrs::bind_textdomain_codeset(config::GETTEXT_PACKAGE, "UTF-8");
    let _ = gettextrs::textdomain(config::GETTEXT_PACKAGE);

    // Source-tree runs use the schema build.rs compiled. This must happen
    // before anything touches GSettings, which caches the schema sources.
    if config::PKGDATADIR.is_empty() {
        let dirs = match std::env::var("GSETTINGS_SCHEMA_DIR") {
            Ok(existing) if !existing.is_empty() => format!("{}:{existing}", config::DEV_SCHEMA_DIR),
            _ => config::DEV_SCHEMA_DIR.to_string(),
        };
        // SAFETY: still single-threaded; GTK has not been initialized yet.
        unsafe { std::env::set_var("GSETTINGS_SCHEMA_DIR", dirs) };
    }

    gio::resources_register_include!("agora.gresource").expect("could not register resources");
    glib::set_application_name("Agora");

    AgoraApplication::new().run()
}
