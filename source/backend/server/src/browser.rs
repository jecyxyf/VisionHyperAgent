/// Opens the local UI in the user's default browser.
pub fn open(url: &str) -> Result<(), String> {
    webbrowser::open(url).map_err(|error| format!("failed to open {url}: {error}"))
}
