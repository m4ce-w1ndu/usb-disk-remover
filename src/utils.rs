use windows::core::PCWSTR;

/// Returns true if the bit in position `bit` for `value` is set.
pub fn is_bit_set(value: u32, bit: u8) -> bool {
    (value & (1 << bit)) != 0
}

/// Converts a string literal to an UTF16 word array
pub fn str_to_utf16vec(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0u16]).collect::<Vec<u16>>()
}
