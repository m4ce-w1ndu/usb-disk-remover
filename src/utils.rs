/// Returns true if the bit in position `bit` for `value` is set.
pub fn is_bit_set(value: u32, bit: u8) -> bool {
    (value & (1 << bit)) != 0
}
