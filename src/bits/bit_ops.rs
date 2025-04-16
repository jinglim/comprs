// Safe, non overflowing bit operations.

/// Shift left a value up to 64 bits.
#[inline]
pub fn shift_left(data: u64, bits: u32) -> u64 {
    // (An alternative is to use unbounded_shift_left.)
    if bits == 64 {
        0
    } else {
        data << bits
    }
}

/// Shift right a value up to 64 bits.
#[inline]
pub fn shift_right(data: u64, bits: u32) -> u64 {
    // (An alternative is to use unbounded_shift_right.)
    if bits == 64 {
        0
    } else {
        data >> bits
    }
}
