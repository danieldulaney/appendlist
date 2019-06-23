# AppendList: An append-only list data structure

[![Build Status](https://travis-ci.com/danieldulaney/appendlist.svg?branch=master)](https://travis-ci.com/danieldulaney/appendlist)

This list lets you add stuff to it, even if you're holding a reference to some
of its elements.

When should you use it?

- You need to insert into a list while its other elements are borrowed
- You have so much data that `Vec` reallocations are significant

When shouldn't you use it?

- You are storing indices into a list, rather than actual references
- `Vec` reallocations don't matter very much (normally the case!)

What are some features?

- A `push(&self, item: T)` method (you'd expect `push(&mut self, item: T)`)
- Non-amortized constant-time insertions and indexes
- You can hold onto references in the list
