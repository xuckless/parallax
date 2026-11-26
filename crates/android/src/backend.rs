//! Which GPU backend the renderer tries first, decided from one device fact: `ro.hardware` (Java's
//! `Build.HARDWARE`). Pure, so it is tested on any host; `renderer.rs` reads the property (once) and maps [`Backend`] to wgpu
//! (`impl From<Backend> for wgpu::Backends`).

/// A GPU backend `gpui_wgpu` can draw with on Android.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backend {
    Vulkan,
    Gl,
}

/// The kind of device the app runs on, as far as the renderer cares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Device {
    /// A virtual device: the Android emulator, whose Vulkan (gfxstream) draws black or drops glyphs (API 33) while its
    /// GL draws correctly, or another one whose guest Vulkan is commonly broken or missing (Cuttlefish, Genymotion).
    Emulator,
    /// Anything else: a phone, tablet or other real hardware.
    Real,
}

/// `ro.hardware` values of virtual devices. init copies it from `androidboot.hardware`; Cuttlefish's
/// `vsoc_x86_64`/`vsoc_arm64` are its `ro.product.device`, not this. To recognise another virtual device, add its value.
const EMULATOR_HARDWARE: [&str; 5] = [
    "ranchu",   // Android emulator, API 25+
    "goldfish", // Android emulator, older system images
    "cutf_cvm", // Cuttlefish
    "vsoc",     // Cuttlefish, before cutf_cvm
    "vbox86",   // Genymotion (VirtualBox)
];

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
                "cuttlefish_gl_first",
                "cutf_cvm",
                [Backend::Gl, Backend::Vulkan],
            ),
            (
                "old_cuttlefish_gl_first",
                "vsoc",
                [Backend::Gl, Backend::Vulkan],
            ),
            (
                "genymotion_gl_first",
                "vbox86",
                [Backend::Gl, Backend::Vulkan],
            ),
            (
                "cuttlefish_product_name_is_not_hardware",
                "vsoc_x86_64",
                [Backend::Vulkan, Backend::Gl],
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
