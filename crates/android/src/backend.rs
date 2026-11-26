//! Which GPU backend the renderer tries first, decided from one device fact: `ro.hardware` (Java's
//! `Build.HARDWARE`). Pure, so it is tested on any host; `renderer.rs` reads the property and maps [`Backend`] to wgpu.

/// A GPU backend `gpui_wgpu` can draw with on Android.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backend {
    Vulkan,
    Gl,
}

/// The kind of device the app runs on, as far as the renderer cares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Device {
    /// The Android emulator: its Vulkan (gfxstream) draws black or drops glyphs (API 33), its GL draws correctly.
    Emulator,
    /// Anything else: a phone, tablet or other real hardware.
    Real,
}

/// `ro.hardware` values of the Android emulator. To recognise another emulator, add its value.
const EMULATOR_HARDWARE: [&str; 2] = ["ranchu", "goldfish"];

impl Device {
    /// The device whose `ro.hardware` is `hardware`.
    pub fn from_hardware(hardware: &str) -> Self {
        if EMULATOR_HARDWARE.contains(&hardware) {
            Device::Emulator
        } else {
            Device::Real
        }
    }

    /// The backend table: per device, backends in order of preference. The first that can drive the surface wins,
    /// at startup and after device loss. To change an order, edit its row; a new [`Device`] must add one.
    pub fn backends(self) -> [Backend; 2] {
        match self {
            Device::Real => [Backend::Vulkan, Backend::Gl],
            Device::Emulator => [Backend::Gl, Backend::Vulkan],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backends_by_hardware() {
        // (row name, ro.hardware, expected order)
        let cases = [
            (
                "emulator_ranchu_gl_first",
                "ranchu",
                [Backend::Gl, Backend::Vulkan],
            ),
            (
                "emulator_goldfish_gl_first",
                "goldfish",
                [Backend::Gl, Backend::Vulkan],
            ),
            (
                "pixel_vulkan_first",
                "tensor",
                [Backend::Vulkan, Backend::Gl],
            ),
            (
                "qualcomm_vulkan_first",
                "qcom",
                [Backend::Vulkan, Backend::Gl],
            ),
            (
                "unknown_empty_vulkan_first",
                "",
                [Backend::Vulkan, Backend::Gl],
            ),
            (
                "prefix_is_not_emulator",
                "ranchu2",
                [Backend::Vulkan, Backend::Gl],
            ),
        ];
        for (name, hardware, expected) in cases {
            assert_eq!(
                Device::from_hardware(hardware).backends(),
                expected,
                "{name}"
            );
        }
    }
}
