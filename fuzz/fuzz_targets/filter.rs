#![no_main]

use libfuzzer_sys::fuzz_target;
use oxpdf_fuzz::fuzz_filter;

fuzz_target!(|data: &[u8]| {
    fuzz_filter(data);
});
