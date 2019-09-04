// Must be a power of 2
const FIRST_CHUNK_SIZE: usize = 16;


pub const fn chunk_size(chunk_id: usize) -> usize {
    // First chunk is FIRST_CHUNK_SIZE, subsequent chunks double each time
    FIRST_CHUNK_SIZE << chunk_id
}

pub const fn chunk_start(chunk_id: usize) -> usize {
    // This looks like magic, but I promise it works
    // Essentially, each chunk is the size of the sum of all chunks before
    // it. Except that the first chunk is different: it "should" be preceded
    // by a whole list of chunks that sum to its size, but it's not. Therefore,
    // there's a "missing" set of chunks the size of the first chunk, so
    // later chunks need to be updated.
    chunk_size(chunk_id) - FIRST_CHUNK_SIZE
}

pub const fn index_chunk(index: usize) -> usize {
    // This *is* magic
    floor_log2(index + FIRST_CHUNK_SIZE) - floor_log2(FIRST_CHUNK_SIZE)
}

#[inline]
pub const fn floor_log2(x: usize) -> usize {
    const BITS_PER_BYTE: usize = 8;

    BITS_PER_BYTE * std::mem::size_of::<usize>() - (x.leading_zeros() as usize) - 1
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn chunk_sizes_make_sense() {
        assert_eq!(chunk_size(0), FIRST_CHUNK_SIZE);

        let mut index = 0;

        for chunk in 0..20 {
            // Each chunk starts just after the previous one ends
            assert_eq!(chunk_start(chunk), index);
            index += chunk_size(chunk);
        }
    }

    #[test]
    fn index_chunk_matches_up() {
        for index in 0..1_000_000 {
            let chunk_id = index_chunk(index);

            // Each index happens after its chunk start and before its chunk end
            assert!(index >= chunk_start(chunk_id));
            assert!(index < chunk_start(chunk_id) + chunk_size(chunk_id));
        }
    }
}
