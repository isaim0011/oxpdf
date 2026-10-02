#![no_main]

use libfuzzer_sys::fuzz_target;
use oxpdf_fuzz::fuzz_font;

fuzz_target!(|data: &[u8]| {
    fuzz_font(data);
});
