pub fn set_bit(bitmap: &mut [u64], index: usize) {
    let (word, bit) = (index / 64, index % 64);
    bitmap[word] |= 1 << bit;
}

pub fn clear_bit(bitmap: &mut [u64], index: usize) {
    let (word, bit) = (index / 64, index % 64);
    bitmap[word] &= !(1 << bit);
}

pub fn is_bit_set(bitmap: &[u64], index: usize) -> bool {
    let (word, bit) = (index / 64, index % 64);
    (bitmap[word] & (1 << bit)) != 0
}