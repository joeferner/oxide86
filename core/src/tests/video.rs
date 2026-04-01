use crate::{
    tests::{assert_screen, create_computer, run_assert_screen_key_press_run_test, run_test},
    video::VideoCardType,
};

/// Prints all 256 ASCII characters (0x00-0xFF) using the 8x16 VGA font
/// in text mode 3 (80x25), displayed in a 16x16 grid.
#[test_log::test]
pub(crate) fn print_chars_8x16() {
    run_assert_screen_key_press_run_test(
        "video/print_chars_8x16",
        make_computer!(video_card_type: VideoCardType::VGA),
    );
}

/// Prints all 256 ASCII characters (0x00-0xFF) using the 8x8 CGA font
/// in CGA graphics mode 04h (320x200 4-color), displayed in a 16x16 grid.
#[test_log::test]
pub(crate) fn print_chars_8x8() {
    run_assert_screen_key_press_run_test("video/print_chars_8x8", create_computer());
}

#[test_log::test]
pub(crate) fn mode_13h_vga_320x200x256() {
    run_assert_screen_key_press_run_test(
        "video/mode_13h_vga_320x200x256",
        make_computer!(video_card_type: VideoCardType::VGA),
    );
}

#[test_log::test]
pub(crate) fn mode_06h_cga_640x200x2() {
    run_assert_screen_key_press_run_test("video/mode_06h_cga_640x200x2", create_computer());
}

/// Tests that port 0x3DA bit 0 (horizontal retrace) toggles, allowing programs
/// that use the classic CGA snow-avoidance write pattern to proceed.
/// Without bit 0 toggling the double-poll loop spins forever and nothing renders.
#[test_log::test]
pub(crate) fn cga_snow_avoidance() {
    run_assert_screen_key_press_run_test(
        "video/cga_snow_avoidance",
        make_computer!(video_card_type: VideoCardType::CGA),
    );
}

/// Tests mode 06h with a custom VGA DAC palette programmed via INT 10h after the mode set.
/// Replicates observed real-program behavior: after switching to mode 06h the program
/// reprograms DAC registers 0-15 (including DAC[15] = greenish RGB(35,54,6)).
/// Mode 06h foreground uses vga_dac_palette[cga_bg=15] = DAC[15], so the foreground
/// should appear in the custom greenish color, not the default white.
#[test_log::test]
pub(crate) fn mode_06h_vga_custom_dac() {
    run_assert_screen_key_press_run_test(
        "video/mode_06h_vga_custom_dac",
        make_computer!(video_card_type: VideoCardType::VGA),
    );
}

#[test_log::test]
pub(crate) fn mode_04h_cga_320x200x4() {
    run_assert_screen_key_press_run_test("video/mode_04h_cga_320x200x4", create_computer());
}

/// Tests that EGA card + CGA mode 04h uses EGA 6-bit color codes for the DAC,
/// not the VGA grayscale ramp.
///
/// Mirrors CheckIt's Graphics Grid Test palette setup:
///   AC[0..3] = [0, 19, 21, 23]  (set via INT 10h AH=10h)
///   AC[3] = 23 = 0x17 = R=42,G=63,B=42 (bright green in EGA 6-bit encoding)
///
/// Without fix: DAC[23] = [24,24,24] = dark gray (nearly invisible)
/// With fix:    DAC[23] = [42,63,42] = bright green (clearly visible)
#[test_log::test]
pub(crate) fn mode_04h_ega_ac_palette() {
    run_assert_screen_key_press_run_test(
        "video/mode_04h_ega_ac_palette",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}

#[test_log::test]
pub(crate) fn mode_0dh_ega_320x200x16() {
    run_assert_screen_key_press_run_test(
        "video/mode_0dh_ega_320x200x16",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}

#[test_log::test]
pub(crate) fn mode_0eh_ega_640x200x16() {
    run_assert_screen_key_press_run_test(
        "video/mode_0eh_ega_640x200x16",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}

#[test_log::test]
pub(crate) fn mode_0fh_ega_640x350x4() {
    run_assert_screen_key_press_run_test(
        "video/mode_0fh_ega_640x350x4",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}

/// Tests that CRTC start address registers 0x0C/0x0D correctly offset the
/// display viewport in EGA mode 0x0D. Page 0 (offset 0) is white; page 1
/// (offset 8000 = 0x1F40) is blue. Flipping the CRTC start address must
/// show the correct page.
#[test_log::test]
pub(crate) fn ega_crtc_start_address() {
    run_test(
        "video/ega_crtc_start_address",
        make_computer!(video_card_type: VideoCardType::EGA),
        |computer, video_buffer| {
            computer.run();
            assert_screen("video/ega_crtc_start_address_page0", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ega_crtc_start_address_page1", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

/// Tests INT 10h/AH=11h/AL=30h font info queries and direct EGA VRAM glyph rendering,
/// mirroring the technique SimCity uses for menu text (BH=02h ROM 8x14 path).
#[test_log::test]
pub(crate) fn font_direct_render() {
    run_assert_screen_key_press_run_test(
        "video/font_direct_render",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}

#[test_log::test]
pub(crate) fn print_chars_8x14() {
    run_assert_screen_key_press_run_test(
        "video/print_chars_8x14",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}

#[test_log::test]
pub(crate) fn mode_10h_ega_640x350x16() {
    run_assert_screen_key_press_run_test(
        "video/mode_10h_ega_640x350x16",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}

#[test_log::test]
pub(crate) fn mode_12h_vga_640x480x16() {
    run_assert_screen_key_press_run_test(
        "video/mode_12h_vga_640x480x16",
        make_computer!(video_card_type: VideoCardType::VGA),
    );
}

#[test_log::test]
pub(crate) fn cga_composite_trans() {
    run_assert_screen_key_press_run_test("video/cga_composite/trans", create_computer());
}

#[test_log::test]
pub(crate) fn ct755r_vhw_detect_cga() {
    run_test(
        "video/ct755r/vhw_detect",
        make_computer!(video_card_type: VideoCardType::CGA),
        |computer, video_buffer| {
            computer.run();
            assert_screen("video/ct755r/vhw_detect_cga", video_buffer);
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

/// Verifies INT 10h AH=09h XorInverted behavior in CGA mode 04h.
///
/// When ch has bit 7 set AND attr has bit 7 set (XOR mode), the glyph looked
/// up must be (ch & 0x7F), not ch itself. With ch=0x80 this gives glyph 0x00
/// (blank / all zeros). Inverted: all ones → full-block XOR mask → every pixel
/// in the cell toggles. Applied twice it must restore the original character.
///
/// Step 1 screenshot: 'Y' in white with a full-block XOR cursor over it
///   (inverted Y: black Y on white background).
/// Step 2 screenshot: 'Y' restored to white on black.
#[test_log::test]
pub(crate) fn int10_write_char_xor_inverted() {
    run_test(
        "video/int10_write_char_xor_inverted",
        create_computer(),
        |computer, video_buffer| {
            computer.run();
            assert_screen(
                "video/int10_write_char_xor_inverted_step1",
                video_buffer.clone(),
            );
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen(
                "video/int10_write_char_xor_inverted_step2",
                video_buffer.clone(),
            );
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

/// Verifies INT 10h AH=09h XOR cursor blink in EGA mode 0Dh (320x200x16).
///
/// In EGA mode, bit 7 of AL is NOT an invert flag: ch=0xDB (full block, all
/// pixels on) in XOR mode (BL bit 7) must XOR glyph 0xDB directly onto the
/// screen, inverting every pixel in the cell. Applied twice it restores the
/// original character. This differs from CGA XorInverted behavior where
/// ch=0x80 uses glyph (0x80 & 0x7F)=0x00 inverted as the XOR mask.
///
/// Step 1 screenshot: 'Y' with XOR cursor over it (full cell inverted).
/// Step 2 screenshot: 'Y' restored (second XOR undoes the first).
#[test_log::test]
pub(crate) fn int10_write_char_xor_ega_0dh() {
    run_test(
        "video/int10_write_char_xor_ega_0dh",
        make_computer!(video_card_type: VideoCardType::EGA),
        |computer, video_buffer| {
            computer.run();
            assert_screen(
                "video/int10_write_char_xor_ega_0dh_step1",
                video_buffer.clone(),
            );
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen(
                "video/int10_write_char_xor_ega_0dh_step2",
                video_buffer.clone(),
            );
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

/// Tests blinking text attributes and cursor positioning in CGA text mode 3.
///
/// Screen 1 (blink_off): Blink disabled (intensity mode) — bit 7 gives bright
///   background instead of blinking.
/// Screen 2 (blink_on_visible): Blink enabled, visible phase — blinking chars
///   rendered with foreground visible (blink_phase = false).
/// Screen 2 (blink_on_blanked): Same VRAM, blanked phase — blinking chars
///   rendered as background-only (blink_phase = true).
/// Screen 3 (cursor): Cursor visible at row 12, col 40 with a label above.
#[test_log::test]
pub(crate) fn blink_and_cursor() {
    run_test(
        "video/blink_and_cursor",
        make_computer!(video_card_type: VideoCardType::VGA),
        |computer, video_buffer| {
            // Screen 1: intensity mode (blink disabled)
            computer.run();
            assert_screen("video/blink_and_cursor_01_blink_off", video_buffer.clone());

            // Screen 2: blink enabled
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            // Visible phase (blink_phase = false, default)
            assert_screen("video/blink_and_cursor_02_blink_on_visible", video_buffer.clone());
            // Blanked phase: toggle blink_phase so blinking chars show background only
            {
                let mut vb = video_buffer.write().unwrap();
                vb.set_blink_phase(true);
            }
            assert_screen("video/blink_and_cursor_03_blink_on_blanked", video_buffer.clone());
            // Reset blink phase
            {
                let mut vb = video_buffer.write().unwrap();
                vb.set_blink_phase(false);
            }

            // Screen 3: cursor positioning
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            // Cursor visible phase
            assert_screen("video/blink_and_cursor_04_cursor_visible", video_buffer.clone());
            // Cursor blanked phase: blink_phase hides the cursor
            {
                let mut vb = video_buffer.write().unwrap();
                vb.set_blink_phase(true);
            }
            assert_screen("video/blink_and_cursor_05_cursor_blanked", video_buffer.clone());
            // Reset blink phase
            {
                let mut vb = video_buffer.write().unwrap();
                vb.set_blink_phase(false);
            }

            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

/// Verifies AC palette routing in CGA mode 04h on EGA/VGA.
///
/// The full color chain on VGA is:
///   2-bit pixel value → AC palette register[pixel] → DAC index → RGB
///
/// This test fills four 50-row horizontal bands with pixel values 0-3, then
/// programs the AC palette (AL=02h) to route pixel i → DAC i, and sets DAC
/// registers 0-3 to black/cyan/purple/white.  The expected screen is four
/// distinct color bands from top to bottom.
#[test_log::test]
pub(crate) fn int10_cga_ac_palette() {
    run_assert_screen_key_press_run_test(
        "video/int10_cga_ac_palette",
        make_computer!(video_card_type: VideoCardType::VGA),
    );
}

#[test_log::test]
pub(crate) fn ct755r_vhw_detect_mda() {
    run_test(
        "video/ct755r/vhw_detect",
        make_computer!(video_card_type: VideoCardType::MDA),
        |computer, video_buffer| {
            computer.run();
            assert_screen("video/ct755r/vhw_detect_mda", video_buffer);
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn ct755r_vhw_detect_hgc() {
    run_test(
        "video/ct755r/vhw_detect",
        make_computer!(video_card_type: VideoCardType::HGC),
        |computer, video_buffer| {
            computer.run();
            assert_screen("video/ct755r/vhw_detect_hgc", video_buffer);
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn ct755r_ntsc_out() {
    run_test(
        "video/ct755r/ntsc_out",
        create_computer(),
        |computer, video_buffer| {
            computer.run();
            assert_screen("video/ct755r/ntsc_out_m4_p0", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/ntsc_out_m4_p0h", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/ntsc_out_m4_p1", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/ntsc_out_m4_p1h", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/ntsc_out_m6", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/ntsc_out_m6h", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn ct755r_mode0123() {
    run_test(
        "video/ct755r/mode0123",
        create_computer(),
        |computer, video_buffer| {
            computer.run();
            assert_screen("video/ct755r/mode0123_m0_chars", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode0123_m0_colors", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode0123_m1_chars", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode0123_m1_colors", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode0123_m2_chars", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode0123_m2_colors", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode0123_m3_chars", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode0123_m3_colors", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn ct755r_mode45() {
    run_test(
        "video/ct755r/mode45",
        create_computer(),
        |computer, video_buffer| {
            // Mode 4: 16 screens
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s01", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s02", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s03", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s04", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s05", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s06", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s07", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s08", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s09", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s10", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s11", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s12", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s13", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s14", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s15", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m4_s16", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            // Mode 5: 16 screens
            assert_screen("video/ct755r/mode45_m5_s01", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s02", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s03", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s04", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s05", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s06", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s07", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s08", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s09", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s10", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s11", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s12", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s13", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s14", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s15", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode45_m5_s16", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn ct755r_mode6() {
    run_test(
        "video/ct755r/mode6",
        create_computer(),
        |computer, video_buffer| {
            computer.run();
            assert_screen("video/ct755r/mode6_s01", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode6_s02", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode6_s03", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode6_s04", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode6_s05", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode6_s06", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn ct755r_mode7() {
    run_test(
        "video/ct755r/mode7",
        make_computer!(video_card_type: VideoCardType::MDA),
        |computer, video_buffer| {
            computer.run();
            assert_screen("video/ct755r/mode7_s01", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
            assert_screen("video/ct755r/mode7_s02", video_buffer.clone());
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn ct755r_c160_100() {
    run_assert_screen_key_press_run_test("video/ct755r/c160_100", create_computer());
}

#[test_log::test]
pub(crate) fn ct755r_vhw_detect_ega_256k() {
    run_test(
        "video/ct755r/vhw_detect",
        make_computer!(video_card_type: VideoCardType::EGA),
        |computer, video_buffer| {
            computer.run();
            assert_screen("video/ct755r/vhw_detect_ega_256k", video_buffer);
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}
