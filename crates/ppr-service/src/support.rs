//! The support page, opened in the user's browser by every desktop front end.

use std::process::{Command, Stdio};

use crate::ServiceError;

/// The support page. The address is fixed here, so no front end can be made to open others.
pub const SUPPORT_URL: &str = "https://ko-fi.com/techneut92";

/// Opens the support page in the default browser. Blocks until an opener has started it.
pub fn open_support_page() -> Result<(), ServiceError> {
    open_url(SUPPORT_URL)
}

/// The commands that can open `url` in a browser, tried in turn until one succeeds.
fn openers(url: &str) -> Vec<Command> {
    #[cfg(target_os = "windows")]
    {
        let mut c = Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        vec![c]
    }
    #[cfg(target_os = "macos")]
    {
        let mut c = Command::new("open");
        c.arg(url);
        vec![c]
    }
    // `xdg-open` is the usual opener. Where it finds no browser (Flatpak, containers) the desktop portal can open
    // one on the host, and `gio` covers systems without `xdg-open`.
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let mut xdg = Command::new("xdg-open");
        xdg.arg(url);
        let mut portal = Command::new("gdbus");
        portal.args(["call", "--session", "--dest", "org.freedesktop.portal.Desktop"]);
        portal.args(["--object-path", "/org/freedesktop/portal/desktop"]);
        portal.args(["--method", "org.freedesktop.portal.OpenURI.OpenURI", "", url, "{}"]);
        let mut gio = Command::new("gio");
        gio.args(["open", url]);
        vec![xdg, portal, gio]
    }
}

/// Opens `url` with the first opener that succeeds.
fn open_url(url: &str) -> Result<(), ServiceError> {
    let mut last = String::from("no opener");
    for mut cmd in openers(url) {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        match cmd.status() {
            Ok(s) if s.success() => return Ok(()),
            Ok(s) => last = s.to_string(),
            Err(e) => last = e.to_string(),
        }
    }
    Err(ServiceError::Browser {
        url: url.to_owned(),
        reason: last,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_opener_gets_the_address() {
        let commands = openers(SUPPORT_URL);
        assert!(!commands.is_empty());
        for c in &commands {
            assert!(
                c.get_args().any(|a| a == SUPPORT_URL),
                "{:?} doesn't pass the address",
                c.get_program()
            );
        }
    }
}
