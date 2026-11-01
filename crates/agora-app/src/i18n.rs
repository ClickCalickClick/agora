//! gettext helpers with `{}` placeholders filled in order.

use gettextrs::{gettext, ngettext};

fn fill(mut s: String, args: &[&str]) -> String {
    for arg in args {
        if let Some(i) = s.find("{}") {
            s.replace_range(i..i + 2, arg);
        }
    }
    s
}

/// `gettext` with placeholders.
pub fn i18n_f(msgid: &str, args: &[&str]) -> String {
    fill(gettext(msgid), args)
}

/// `ngettext` with placeholders.
pub fn ni18n_f(single: &str, plural: &str, n: u32, args: &[&str]) -> String {
    fill(ngettext(single, plural, n), args)
}
