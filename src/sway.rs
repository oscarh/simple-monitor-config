use crate::types::{Mode, OutputInfo};
use swayipc::Connection;

pub fn get_outputs() -> Result<Vec<OutputInfo>, String> {
    let mut conn = Connection::new().map_err(|e| e.to_string())?;
    let outputs = conn.get_outputs().map_err(|e| e.to_string())?;

    let mut result: Vec<OutputInfo> = outputs
        .into_iter()
        .filter(|o| o.active)
        .map(|o| {
            let current_mode = o
                .current_mode
                .map(|m| Mode {
                    width: m.width,
                    height: m.height,
                    refresh: m.refresh,
                })
                .unwrap_or(Mode {
                    width: 0,
                    height: 0,
                    refresh: 0,
                });

            OutputInfo {
                name: o.name,
                make: o.make,
                model: o.model,
                serial: o.serial,
                x: o.rect.x,
                y: o.rect.y,
                width: o.rect.width,
                height: o.rect.height,
                modes: dedup_modes(
                    o.modes
                        .into_iter()
                        .map(|m| Mode {
                            width: m.width,
                            height: m.height,
                            refresh: m.refresh,
                        })
                        .collect(),
                    &current_mode,
                ),
                current_mode,
                scale: o.scale.unwrap_or(1.0),
                active: o.active,
            }
        })
        .collect();

    result.sort_by_key(|o| o.x);
    Ok(result)
}

// Sway lists CEA variants of a mode separately (differing only in picture
// aspect ratio or 59.94 vs 60 Hz). We show and apply refresh rates rounded to
// whole Hz, so those variants are indistinguishable and only one is kept. The
// current mode wins its group so the pick list can still match it.
fn dedup_modes(modes: Vec<Mode>, current: &Mode) -> Vec<Mode> {
    let key = |m: &Mode| (m.width, m.height, m.refresh_hz());
    let mut result: Vec<Mode> = Vec::with_capacity(modes.len());
    for mode in modes {
        match result.iter_mut().find(|m| key(m) == key(&mode)) {
            Some(existing) if mode == *current => *existing = mode,
            Some(_) => {}
            None => result.push(mode),
        }
    }
    result
}

// Sway identifies an output either by its connector name (DP-1) or by the
// exact "make model serial" string from its EDID. The connector is per-slot
// and changes when a monitor is plugged elsewhere, so prefer the identifier
// to keep configs stable across machines and ports.
fn output_identifier(output: &OutputInfo) -> String {
    if output.make.is_empty() && output.model.is_empty() && output.serial.is_empty() {
        return output.name.clone();
    }
    format!("'{} {} {}'", output.make, output.model, output.serial)
}

pub fn apply_config(outputs: &[OutputInfo], mirror: bool) -> Result<(), String> {
    let mut conn = Connection::new().map_err(|e| e.to_string())?;

    for output in outputs {
        let (x, y) = if mirror { (0, 0) } else { (output.x, output.y) };
        let cmd = format!(
            "output {} mode {}x{}@{}Hz pos {} {}",
            output_identifier(output),
            output.current_mode.width,
            output.current_mode.height,
            output.current_mode.refresh_hz(),
            x,
            y
        );
        let results = conn.run_command(&cmd).map_err(|e| e.to_string())?;
        for result in results {
            result.map_err(|e| format!("{}: {}", output.name, e))?;
        }
    }

    // Refocus our window since output changes may move the cursor to another screen
    let _ = conn.run_command(format!("[app_id={}] focus", crate::APP_ID));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::test_output;

    #[test]
    fn identifier_uses_make_model_serial() {
        let mut o = test_output("DP-1", 0, 0, 1920, 1080);
        o.make = "Dell Inc.".to_string();
        o.model = "DELL C3422WE".to_string();
        o.serial = "G4BYWN3".to_string();
        assert_eq!(output_identifier(&o), "'Dell Inc. DELL C3422WE G4BYWN3'");
    }

    #[test]
    fn identifier_falls_back_to_connector_when_edid_empty() {
        let o = test_output("DP-1", 0, 0, 1920, 1080);
        assert_eq!(output_identifier(&o), "DP-1");
    }

    fn mode(width: i32, height: i32, refresh: i32) -> Mode {
        Mode {
            width,
            height,
            refresh,
        }
    }

    #[test]
    fn dedup_removes_same_rounded_refresh() {
        let modes = vec![
            mode(1920, 1080, 60000),
            mode(1920, 1080, 60000),
            mode(1920, 1080, 59940),
            mode(1920, 1080, 50000),
            mode(1280, 720, 60000),
        ];
        let current = mode(2560, 1440, 59951);
        assert_eq!(
            dedup_modes(modes, &current),
            vec![
                mode(1920, 1080, 60000),
                mode(1920, 1080, 50000),
                mode(1280, 720, 60000),
            ]
        );
    }

    #[test]
    fn dedup_keeps_current_mode_in_its_group() {
        let modes = vec![
            mode(1920, 1080, 60000),
            mode(1920, 1080, 59940),
            mode(1280, 720, 60000),
        ];
        let current = mode(1920, 1080, 59940);
        assert_eq!(
            dedup_modes(modes, &current),
            vec![mode(1920, 1080, 59940), mode(1280, 720, 60000)]
        );
    }
}
