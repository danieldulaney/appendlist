//! An append-only list that preserves references to its elements
//!
//! Just about everything is in the `AppendList` struct.

use std::iter::FromIterator;
use std::ops::Index;

// Must be a power of 2
const FIRST_CHUNK_SIZE: usize = 16;

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

    /// Append an item to the end
    ///
    /// Note that this does not require `mut`.
    pub fn push(&self, item: T) {
        // Unsafe code alert!
        //
        // Preserve the following invariants:
        // - Only the last chunk may be modified
        // - A chunk cannot ever be reallocated
        // - len must reflect the length
        let self_mut: &mut Self = unsafe { &mut *(self as *const _ as *mut _) };

        let new_index = self.len;
        let chunk_id = index_chunk(new_index);

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

            let mut new_chunk = Vec::with_capacity(chunk_size(chunk_id));
            debug_assert!(new_chunk.capacity() >= chunk_size(chunk_id));

            new_chunk.push(item);

            self_mut.chunks.push(new_chunk);
        }

        self_mut.len += 1;
    }

    /// Get the length of the list
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

    /// Get an item from the list, if it is in bounds
    ///
    /// Returns `None` if the `index` is out-of-bounds. Note that you can also
    /// index with `[]`, which will panic on out-of-bounds.
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }

        let chunk_id = index_chunk(index);
        let chunk_start = chunk_start(chunk_id);

        return Some(&self.chunks[chunk_id][index - chunk_start]);
    }

    /// Get an iterator over the list
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        AppendListIter {
            list: &self,
            index: 0,
        }
    }
}

const fn chunk_size(chunk_id: usize) -> usize {
    // First chunk is FIRST_CHUNK_SIZE, subsequent chunks double each time
    FIRST_CHUNK_SIZE << chunk_id
}

const fn chunk_start(chunk_id: usize) -> usize {
    // This looks like magic, but I promise it works
    // Essentially, each chunk is the size of the sum of all chunks before
    // it. Except that the first chunk is different: it "should" be preceded
    // by a whole list of chunks that sum to its size, but it's not. Therefore,
    // there's a "missing" set of chunks the size of the first chunk, so
    // later chunks need to be updated.
    chunk_size(chunk_id) - FIRST_CHUNK_SIZE
}

const fn index_chunk(index: usize) -> usize {
    // This *is* magic
    floor_log2(index + FIRST_CHUNK_SIZE) - floor_log2(FIRST_CHUNK_SIZE)
}

#[inline]
const fn floor_log2(x: usize) -> usize {
    const BITS_PER_BYTE: usize = 8;

    BITS_PER_BYTE * std::mem::size_of::<usize>() - (x.leading_zeros() as usize) - 1
}

impl<T> Index<usize> for AppendList<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index)
            .expect("AppendList indexed beyond its length")
    }
}

impl<T> FromIterator<T> for AppendList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let list = Self::new();

        for item in iter {
            list.push(item);
        }

        list
    }
}

struct AppendListIter<'l, T> {
    list: &'l AppendList<T>,
    index: usize,
}

impl<'l, T> Iterator for AppendListIter<'l, T> {
    type Item = &'l T;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.list.get(self.index);

        self.index += 1;

        item
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.list.len() - self.index;

        (remaining, Some(remaining))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn log2(x: usize) -> f64 {
        (x as f64).log2()
    }

    #[test]
    fn from_iterator() {
        let l: AppendList<i32> = (0..100).collect();

        for i in 0..100 {
            assert_eq!(l[i], i as i32);
        }
    }

    #[test]
    fn iterator() {
        let l: AppendList<i32> = (0..100).collect();
        let mut i = l.iter();

        for item in 0..100 {
            assert_eq!(i.next(), Some(&item));
        }

        assert_eq!(i.next(), None);
    }

    #[test]
    fn iterator_size_hint() {
        let l: AppendList<i32> = AppendList::new();
        let mut i = l.iter();
        assert_eq!(i.size_hint(), (0, Some(0)));

        l.push(1);
        assert_eq!(i.size_hint(), (1, Some(1)));

        l.push(2);
        assert_eq!(i.size_hint(), (2, Some(2)));

        i.next();
        assert_eq!(i.size_hint(), (1, Some(1)));

        l.push(3);
        assert_eq!(i.size_hint(), (2, Some(2)));

        i.next();
        assert_eq!(i.size_hint(), (1, Some(1)));

        i.next();
        assert_eq!(i.size_hint(), (0, Some(0)));
    }

    #[test]
    fn first_chunk_size_is_power_of_2() {
        assert_eq!(floor_log2(FIRST_CHUNK_SIZE) as f64, log2(FIRST_CHUNK_SIZE));
    }

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
