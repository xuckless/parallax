//! Display scale as Windows' display settings set it, on plain values (tested on any host). The DisplayConfig
//! packets `main.rs` sends (undocumented types -3 get, -4 set, as the SetDPI tool, github.com/imniko/SetDPI, uses
//! them) name a scale as steps from the display's recommended one, which sits `-min` steps above 100 %.

/// The scale steps Windows offers, in order. To extend, add a step where Windows adds one.
const STEPS: [u32; 12] = [100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500];

/// The relative step that sets `percent`, given the packet's `min` (the lowest step, relative).
pub fn relative_step(percent: u32, min: i32) -> Option<i32> {
    let index = STEPS.iter().position(|step| *step == percent)?;
    Some(index as i32 + min)
}

/// The percent of relative step `current`, given `min`.
pub fn percent(current: i32, min: i32) -> Option<u32> {
    STEPS.get(usize::try_from(current - min).ok()?).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_steps() {
        // (name, percent, min, relative step): the CI runner's 1920 × 1080 display recommends 100 % (min 0).
        let rows = [
            ("100 % recommended", 150, 0, Some(2)),
            ("150 % recommended", 100, -2, Some(-2)),
            ("150 % recommended, 150 %", 150, -2, Some(0)),
            ("not a step", 110, 0, None),
        ];
        for (name, want_percent, min, step) in rows {
            assert_eq!(relative_step(want_percent, min), step, "{name}");
            if let Some(step) = step {
                assert_eq!(percent(step, min), Some(want_percent), "{name}: back");
            }
        }
        assert_eq!(percent(-3, -2), None, "below the lowest step");
    }
}
