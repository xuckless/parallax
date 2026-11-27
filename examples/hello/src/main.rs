//! Desktop entry point (macOS, Windows, Linux). Android and the web start from `lib.rs`.

use parallax::gpui::Application;
use parallax::parallax_platform::{Error, current_platform};

fn main() -> Result<(), Error> {
    Application::with_platform(current_platform(false)?).run(|cx| {
        // Flutter's gesture feel for this platform (docs/behaviour.md); android_main! does this on Android.
        parallax::set_platform_gesture_tuning(cx);
        hello::launch(cx)
    });
    Ok(())
}
