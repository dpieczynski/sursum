// SPDX-License-Identifier: GPL-3.0-or-later
//! The few pages the window shows instead of the remote, in Polish (the program's only UI text
//! outside the remote). Plain HTML with inline styles: no server is running to serve anything.

use std::path::Path;

/// Another copy of the program is running (its window may be hidden behind others).
pub fn already_running() -> String {
    page(
        "Sursum jest już uruchomiony",
        "Program działa w innym oknie, które może być schowane za innymi oknami albo \
         zminimalizowane. Zamknij to okno i przejdź do tamtego.",
    )
}

/// The server did not start; the details are in the log file, if there is one.
pub fn could_not_start(log: Option<&Path>) -> String {
    let details = match log {
        Some(path) => format!("Szczegóły są w pliku: {}", path.display()),
        None => "Nie udało się też zapisać szczegółów do pliku.".to_owned(),
    };
    page("Nie udało się uruchomić Sursum", &format!("Spróbuj uruchomić program ponownie. {details}"))
}

fn page(title: &str, text: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pl\"><head><meta charset=\"utf-8\"><title>{title}</title></head>\
<body style=\"margin:0;min-height:100vh;display:grid;place-items:center;background:#111;color:#eee;\
font-family:system-ui,sans-serif\"><main style=\"max-width:36rem;padding:2rem;line-height:1.5\">\
<h1 style=\"font-size:1.5rem\">{title}</h1><p style=\"user-select:text\">{text}</p></main></body></html>",
        title = escape(title),
        text = escape(text),
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_in_the_message_cannot_break_the_page() {
        let html = could_not_start(Some(Path::new("C:\\Users\\<ks.>\\sursum.log")));
        assert!(html.contains("&lt;ks.&gt;") && !html.contains("<ks.>"));
    }
}
