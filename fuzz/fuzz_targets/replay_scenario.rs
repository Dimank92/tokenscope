#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    tokenscope_fuzz_harness::replay_scenario(data);
});
