/// A list that can be appended to while elements are borrowed
///
/// This looks like a fairly bare-bones list API, except that it has a `push`
/// method that works on non-`mut` lists. It is safe to hold references to
/// values inside this list and push a new value onto the end.
///
/// Additionally, the list has O(1) index and O(1) push (not amortized!).
///
/// For example, this would be illegal with a `Vec`:
///
/// ```
/// use appendlist::AppendList;
///
/// let list = AppendList::new();
///
/// list.push(1);
/// let first_item = &list[0];
/// list.push(2);
/// let second_item = &list[1];
///
/// assert_eq!(*first_item, list[0]);
/// assert_eq!(*second_item, list[1]);
/// ```
///
/// # Implementation details
///
/// This section is not necessary to use the API, it just describes the underlying
/// allocation and indexing strategies.
///
/// The list is a `Vec` of *chunks*. Each chunk is itself a `Vec<T>`. The list
/// will fill up a chunk, then allocate a new chunk with its full capacity.
/// Because the capacity of a given chunk never changes, the underlying `Vec<T>`
/// never reallocates, so references to that chunk are never invalidated. Each
/// chunk is twice the size of the previous chunk, so there will never be more
/// than O(log(n)) chunks.
///
/// Constant-time indexing is achieved because the chunk ID of a particular index
/// can be quickly calculated: if the first chunk has size c, index i will be
/// located in chunk floor(log2(i + c) - log2(c)). If c is a power of 2, this
/// is equivalent to floor(log2(i + c)) - floor(log2(c)), and a very fast floor
/// log2 algorithm can be derived from `usize::leading_zeros()`.
use std::ops::Index;

// Must be a power of 2
const FIRST_CHUNK_SIZE: usize = 16;

pub struct AppendList<T> {
    chunks: Vec<Vec<T>>,
    len: usize,
}

impl<T> AppendList<T> {
    /// Create a new `AppendList`
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            len: 0,
        }
    }

    pub fn push(&self, item: T) {
        // Unsafe code alert!
        //
        // Preserve the following invariants:
        // - Only the last chunk may be modified
        // - A chunk cannot ever be reallocated
        // - len must reflect the length
        let self_mut: &mut Self = unsafe { &mut *(self as *const _ as *mut _) };

        let new_index = self.len;
        let chunk_id = Self::index_chunk(new_index);

        if chunk_id < self.chunks.len() {
            // We should always be inserting into the last chunk
            debug_assert_eq!(chunk_id, self.chunks.len() - 1);

            // Insert into the appropriate chunk
            let chunk = &mut self_mut.chunks[chunk_id];

            // The chunk must not be reallocated! Save the pre-insertion capacity
            // so we can check it later (debug builds only)
            #[cfg(debug)]
            let prev_capacity = chunk.capacity();

            // Do the insertion
            chunk.push(item);

            // Check that the capacity didn't change (debug builds only)
            #[cfg(debug)]
            assert_eq!(prev_capacity, chunk.capacity());
        } else {
            // Need to allocate a new chunk

            // New chunk should be the immediate next chunk
            debug_assert_eq!(chunk_id, self.chunks.len());

            let mut new_chunk = Vec::with_capacity(Self::chunk_size(chunk_id));
            debug_assert!(new_chunk.capacity() >= Self::chunk_size(chunk_id));

            new_chunk.push(item);

            self_mut.chunks.push(new_chunk);
        }

        self_mut.len += 1;
    }

    pub fn len(&self) -> usize {
        // Check that all chunks are correct (debug builds only)
        #[cfg(debug)]
        {
            if self.len > 0 {
                // Correct number of chunks
                assert_eq!(Self::index_chunk(self.len - 1), self.chunks.len() - 1);

                // Every chunk holds enough items
                for chunk_id in 0..self.chunks.len() {
                    assert!(Self::chunk_size(chunk_id) <= self.chunks[chunk_id].capacity());
                }

                // Intermediate chunks are full
                for chunk_id in 0..self.chunks.len() - 1 {
                    assert_eq!(Self::chunk_size(chunk_id), self.chunks[chunk_id].len());
                }

                // Last chunk is correct length
                assert_eq!(
                    self.chunks[self.chunks.len() - 1].len() - 1,
                    self.len - Self::chunk_start(self.chunks.len() - 1)
                );
            } else {
                // No chunks
                assert_eq!(0, self.chunks.len());
            }
        }

        self.len
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }

        let chunk_id = Self::index_chunk(index);
        let chunk_start = Self::chunk_start(chunk_id);

        return Some(&self.chunks[chunk_id][index - chunk_start]);
    }

    fn chunk_size(chunk_id: usize) -> usize {
        FIRST_CHUNK_SIZE << chunk_id
    }

    fn chunk_start(chunk_id: usize) -> usize {
        AppendList::<()>::chunk_size(chunk_id) - FIRST_CHUNK_SIZE
    }

    fn index_chunk(index: usize) -> usize {
        Self::floor_log2(index + FIRST_CHUNK_SIZE) - Self::floor_log2(FIRST_CHUNK_SIZE)
    }

    #[inline]
    const fn floor_log2(x: usize) -> usize {
        const BITS_PER_BYTE: usize = 8;

        BITS_PER_BYTE * std::mem::size_of::<usize>() - (x.leading_zeros() as usize) - 1
    }
}

impl<T> Index<usize> for AppendList<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index)
            .expect("AppendList indexed beyond its length")
    }
}

#[cfg(test)]
mod test {
    use super::{AppendList, FIRST_CHUNK_SIZE};

    #[test]
    fn chunk_sizes_make_sense() {
        assert_eq!(AppendList::<()>::chunk_size(0), FIRST_CHUNK_SIZE);

        let mut index = 0;

        for chunk in 0..20 {
            // Each chunk starts where the last one ends
            assert_eq!(AppendList::<()>::chunk_start(chunk), index);
            index += AppendList::<()>::chunk_size(chunk);
        }
    }

    #[test]
    fn index_chunk_matches_up() {
        for index in 0..1_000_000 {
            let chunk_id = AppendList::<()>::index_chunk(index);

            assert!(index >= AppendList::<()>::chunk_start(chunk_id));
            assert!(
                index
                    < AppendList::<()>::chunk_start(chunk_id)
                        + AppendList::<()>::chunk_size(chunk_id)
            );
        }
    }

    #[test]
    fn empty_list() {
        let l: AppendList<usize> = AppendList::new();

        assert_eq!(l.len(), 0);
        assert_eq!(l.get(0), None);
    }

    #[test]
    fn thousand_item_list() {
        test_big_list(1_000);
    }

    #[test]
    fn million_item_list() {
        test_big_list(1_000_000);
    }

    fn test_big_list(size: usize) {
        let l = AppendList::new();
        let mut refs = Vec::new();

        for i in 0..size {
            assert_eq!(l.len(), i);

            l.push(i);
            refs.push(l[i]);

            assert_eq!(l.len(), i + 1);
        }

        for i in 0..size {
            assert_eq!(Some(&refs[i]), l.get(i));
        }
    }
}
