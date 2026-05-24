//! AC vs. battery detection via /sys/class/power_supply (no daemons, no polling
//! cost beyond a couple of tiny file reads). Drives battery-first interval scaling.

use std::fs;

/// True if running on battery (an AC adapter exists and none is online).
/// Desktops with no AC adapter at all are treated as "on AC".
pub fn on_battery() -> bool {
    let Ok(entries) = fs::read_dir("/sys/class/power_supply") else {
        return false;
    };
    let mut saw_ac = false;
    for entry in entries.flatten() {
        let p = entry.path();
        let kind = fs::read_to_string(p.join("type")).unwrap_or_default();
        if kind.trim() == "Mains" {
            saw_ac = true;
            if fs::read_to_string(p.join("online"))
                .unwrap_or_default()
                .trim()
                == "1"
            {
                return false; // plugged in
            }
        }
    }
    saw_ac // AC present but none online -> battery; no AC at all -> false
}
