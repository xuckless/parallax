// Parallax's own test (not ported). See docs/android-tests.md.
package io.github.xuckless.parallax;

import static org.junit.Assert.assertEquals;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.junit.Test;

/**
 * Rust sends each GPUI haptic as a `HapticFeedback.vibrate` argument, listed once, in the match of
 * crates/android/src/haptics.rs `flutter_argument`. This test reads that match (build.gradle passes
 * the file as `parallax.hapticsRs`) instead of copying it, so the list stays in one place.
 */
public class HapticArgumentsTest {
  // One arm of the match: `Haptic::Kind => None,` or `Haptic::Kind => Some("argument"),`.
  private static final Pattern ARM =
      Pattern.compile("Haptic::(\\w+) => (?:None|Some\\(\"([^\"]+)\"\\)),");

  @Test
  public void everyRustArgumentResolvesToItsOwnType() throws IOException, NoSuchFieldException {
    String source =
        new String(Files.readAllBytes(Paths.get(System.getProperty("parallax.hapticsRs"))), "UTF-8");
    Matcher arm = ARM.matcher(source);
    List<String> kinds = new ArrayList<>();
    Set<PlatformChannel.HapticFeedbackType> types =
        EnumSet.noneOf(PlatformChannel.HapticFeedbackType.class);
    while (arm.find()) {
      kinds.add(arm.group(1));
      // fromValue throws for an argument Java does not know; a null one is vibrate()'s STANDARD.
      types.add(PlatformChannel.HapticFeedbackType.fromValue(arm.group(2)));
    }
    assertEquals(
        "every Android type is some Rust arm's: " + kinds,
        EnumSet.allOf(PlatformChannel.HapticFeedbackType.class),
        types);
    assertEquals("no two Rust arms send the same type: " + kinds, types.size(), kinds.size());
  }
}
