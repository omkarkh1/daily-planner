/// macOS integration via built-in system tools only.
/// No third-party services. No network. No accounts.

pub fn notify(title: &str, message: &str) {
    let script = format!(
        "display notification {:?} with title {:?}",
        message, title
    );
    let _ = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output();
}

pub fn is_dark_mode() -> Option<bool> {
    let output = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Some(stdout.trim().to_lowercase() == "dark")
}

pub fn battery_percent() -> Option<u8> {
    let output = std::process::Command::new("pmset")
        .args(["-g", "batt"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    // Look for something like "97%" in the output
    for word in text.split_whitespace() {
        if let Some(pct) = word.strip_suffix('%') {
            if let Ok(n) = pct.replace(';', "").parse::<u8>() {
                return Some(n);
            }
        }
    }
    None
}

pub fn copy_to_clipboard(text: &str) {
    let mut child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .ok();
    if let Some(ref mut c) = child {
        use std::io::Write;
        if let Some(stdin) = c.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = c.wait();
    }
}

pub fn paste_from_clipboard() -> Option<String> {
    let output = std::process::Command::new("pbpaste").output().ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn open_file(path: &str) {
    let _ = std::process::Command::new("open").arg(path).spawn();
}

pub fn say(text: &str) {
    let _ = std::process::Command::new("say").arg(text).spawn();
}

pub fn play_sound(file: &str) {
    let _ = std::process::Command::new("afplay").arg(file).spawn();
}
