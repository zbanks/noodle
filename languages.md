Language Choice
===============

I originally wrote the Noodle "core" in C, but as an exercise I have done a proof-of-concept port to Zig and then Rust. 

I wanted to collect my thoughts into a retrospective of sorts, even though the project is still in development.

This document is *not* meant to be a head-to-head comparison of C, Zig, and Rust.
Each language absolutely has its pros & cons, and this one project is not representative of the wide variety of contexts programming can exist in.

**Note:** This document is still a work-in-progress; and is not complete. 

Goals & Assumptions
-------------------

Noodle is a tool that I've wanted for a while for puzzle hunts, and is heavily inspired by https://tools.qhex.org .
I originally started work on Noodle towards the end of 2020, and wanted to have something useable for the 2021 Mystery Hunt.

Roughly in order of importance, here are the goals, considerations, and assumptions that factor into Noodle's implementation:

- Noodle needs to be *fast*. Although it uses a lot of heuristics, it's ultimately trying to solve `O(n^k)`-ish problems
    - There's a huge usability difference between returning results in 200ms vs. 20s
    - Should be able to return results *incrementally*, time-to-first-result is also important
- The core library should have a reasonable API, so it can be used directly from e.g. Python
    - If Noodle is missing a certain feature, maybe it can be scripted around
- For the first implementation, I wanted something my teammates & I could use (i.e. not public)
    - Security/crashes was not important -- could tolerate crashes & restarts if needed
    - For hunt, I ended up hosting Noodle on a dedicated VM behind a simple auth wall
    - For "production", I care a *bit* more: but it still is a stateless & low-permission app

First Pass: C
-------------

As a background note, I've been writing primarily C code professionally for the last several years, and I absolutely feel comfortable using C to hack together prototypes.

### What went well

#### Python Bindings

(This isn't a totally fair, because I have not yet done these steps for Zig/Rust)

I generated Python bindings for the core Noodle library using `cffi`, and overall it was a pretty smooth experience.

I did end up writing a [sketchy script](https://github.com/zbanks/noodle/blob/c/build_cffi.py) for fixing up the header files to extract the relevant exported declarations.
This step was mostly a kludge -- the `libnoodle` interfaces do *not* have a good ABI boundary: if I did the refactoring to smooth it out so it could be included in other C projects, it would be a lot easier to include from Python.

I also did have to write the [Pythonic wrappers](https://github.com/zbanks/noodle/blob/c/noodle.py), which did feel a bit meh.
Overall there's a bit of fence-sitting there about how "Pythonic" vs. C-like the API should be.
If the project were a larger, or had a less stable API, I think it could become a pain to keep this in sync.
And, failing to update the Python wrapper would usually become a runtime error, rather than something that could be determined at compile-time, by `pylint`, or on module import.

#### Generic Bitset

This pro is only relative to Rust -- but Rust without const generics makes it difficult to implement compile-time fixed-length bitsets.
(It can be done with macros, though!)

On the other hand, in C it ends up being a pretty vanilla use of CPP macros.

#### Ownership 

I like using convention that C structs can provide *either* `foo_init(&foo)`/`foo_term(&foo)` or `foo = foo_create()`/`foo_destroy(foo)` constructors/destructors: init/term leaves the space allocation up to the parent (e.g. can be put on the stack), whereas create/destroy handles allocation.

I used the init/term pattern for all the small/"common" C structs.
Theoretically there can be performance benefits from avoiding multiple small allocations and/or copies.

This works pretty well in C code, but oddly enough it did make the Python wrappers a bit harder to write: they had to do a surprising amount of memory management.

#### "Stupid" Low-level Optimizations

C makes it incredibly straightforward to implement other "stupid" low-level optimizations & bit-packing (which in practice, are often premature).

A good example is my "short strings" optimization in [`struct word`](https://github.com/zbanks/noodle/blob/c/src/word.h#L4-L16) -- where strings that are less than 15 bytes are encoded without an additional allocation/pointer.

Although my ultimate optimization goal was solely wall-clock time (not memory usage, etc.) -- in theory, using memory efficiently should have better cache performance, leading to faster execution times.

#### Debugging / Profiling

Debugging with GDB was very straightforward. I used a [small Python library](https://github.com/zbanks/noodle/blob/c/noodle-gdb.py) to add pretty-printers for my custom data structures. 

I feel that this fills a similar niche to writing custom struct formatters in Zig/Rust, but is obviously only available to the debugger.
I don't feel that this capability is *unique* to C -- but I only needed to do it for C because of my "stupid" optimizations.


### What went poorly

#### Hashset

Most of the needed data structures are simple, easily done in C (vectors, bitsets).

But, there was one spot where a hashset would have been better (see Zig/Rust), in `struct nx_combo_cache` for the `classes` array.
This array is used to determine which words have equivalent transition tables.

To process `n` words, we want to do `O(n)` "get-or-insert index for key" operations so that later we can do `O(n^k)` "get key for index" operations (which are trivially `O(1)`).

I used a simple array instead of a hashset because I figured having `O(n^2)` in the first phase instead of `O(n)` wasn't a big deal compared to the added code complexity.
But, after porting to Zig, I realized changing out this array for a hash lookup lead to some nice performance improvements.

It's not that adding a hashset here would have been insurmountable; but it was sufficiently high activation energy that I didn't try it!
I think it's sort of an interesting edge case: I would have reached for a hashset sooner if the problem more obviously benefited from the data structure (e.g. more lookups by key) or if I needed one elsewhere in the library.

#### Wordlists, Wordsets, and Words

A `struct word` represents either an input or output string (so, despite the name, it may actually be a phrase), along with a "weight" value.

I had previously experimented with "word tuples" (phrases), which were a union variant of `struct word` that contained an array of pointers to its component words.
The original intention was to preserve all of the metadata for the individual words in an output phrase.
(The only metadata that exists now is "weight" -- but it would be nice in the future to connect to definitions, synonyms, etc.)
This concept was scrapped, although `word_tuple_init(...)` exists to populate `struct word` with a phrase.

A `struct wordset` represents a list of *unowned* words, as `struct word *` *pointers*.
Under the hood, it's a pretty basic vector-like.
This is the primary type used for input/output by other components in the library.

A `struct wordlist` represents a list of *owned* words, as `struct word` *values*.
It exposes a `self_set`, the `struct wordset` with references to the same list of words, so it can easily be used with other library components.

Together, these structs form a very primitive parent/owner-like system: with `struct wordlist` acts as an arena for allocating/storing `struct word`s.
The `struct word` has a one-bit "owned" flag, which can be used to ensure that all words/phrases returned by a matcher have an owner without extraneous copies.
But, given that `struct word`s are typically very small (16 bytes) and rarely allocated, this may have been very premature.

Overall I don't think this is more of a problem of premature optimization: it was almost too easy to (locally) build these "optimized" abstractions, even though the "simple" approach would have saved complexity long-term.

Second Pass: Zig
----------------

For background, this is the first time I've written more than a few dozen lines of Zig.
I've been following it from the sidelines for a few years now, was interested in it, but never had a great project to try it out.

### What went well

#### Performance

The Zig implementation ended up being the fastest: the equivalent Rust implementation was about ~50% slower, and the C implementation was about 400% slower (but wasn't equivalent, it didn't have the hashset).

#### GeneralPurposeAllocator

I like that the GPA is sort of like having Valgrind built-in.

Even though Valgrind is absolutely useful on C programs, I think there's something very powerful about making it so accessible (and loud!).
In C, I frequently encounter libraries that raise mostly-benign Valgrind warnings (e.g. they allocate and never free some object on startup) 

#### Comptime

I really like comptime. I think it's a really enjoyable approach to generics/metaprogramming, and it felt very straightforward to use (coming from C/CPP).

(I do wonder how it'd scale with a big project?)

#### Error handling, control flow

Zig's approach to error handling is excellent. 
In C, I've wished the standard library provided a way to extend the global `errno` with user/library-defined error codes -- and Zig feels like it's doing this and more.

Zig's approach *is* opinionated though -- it's more entrenched in the language that Rust's approach; but, it is still relatively straightforward and easy to use.
I really enjoy that working with errors (or non-error values) usually doesn't require casting.
That, plus inferred error unions make it incredibly lightweight to adopt even while prototyping (or translating C).

Likewise, `errdefer`/`defer` are nice syntactic sugar.

#### (Optional) Duck typing (?)

Zig's comptime ends up being an enabling surprisingly weak typing *at compile time*.

A great example of this is `format`: if a struct has a `format(...)` method, then it can be invoked from `print("...", .{...})` calls!
The `print` method is able to use comptime introspection (`@hasDecl`) and behave differently if the method exists or not.

I really like that this is something that you *can* do -- but it's not the default behavior.
This may be a continuation for my general praise of comptime: leveraging comptime doesn't involve necronomicon incantations, which makes the language feel like it can take advantage of the pros of both strong & weak typing without suffering too much from the cons.

It's also really great that it's really easy to use introspection to do careful validation: you can assert not just that the struct has a declaration "format", but you can assert that it is a function, taking the correct parameters, etc.

Also all struct fields are always public, but declarations can be private? Not sure if this is a pro/con, just an interesting observation, not sure where to put it.

### What went poorly

#### Documentation

Zig's compile-time capabilities are really interesting, but I don't envy the challenge they create around documentation.

Although `zig doc` exists, there currently isn't a great strategy for handling generics or functions that return types, etc. 
Right now, most functions which take or return non-global types are listed as `var`.

Overall the generated docs are a good starting point, but I found I had to look at the source directly to actually make use of most of the stdlib.
It's frankly a bit hard/unfair to compare `zig doc` to `cargo doc`, which does a much better job of organizing documentation (but is much more mature).

Although this isn't a dealbreaker, it is the kind of thing that makes me hesitant to consider Zig for larger or more collaborative projects.

This is also one area I'd like to contribute to.

#### Error messages

I frequently encountered some pretty baffling error messages, and I think there's two main problems:

1. Type inference, error set inference, etc. lead to some incredibly long type names.
2. Type checking seems to be be done "backwards" from C compilers and the way I'm used to: I feel like I got error messages that my function had the wrong return type at the declaration, rather than having its use be marked as an error?

These weren't irresolvable, and Zig is absolutely a younger compiler than `rustc` or `gcc`.

#### Unreferenced code unchecked

Something that was a bit surprising to me was the way that unreferenced code isn't fully compiled/type-checked.
It's possible to write code in `foo.zig`, compile a working binary, then change `bar.zig` and now `foo.zig` no longer compiles!

The story is that the author should be writing tests - and tests should ensure that the code is referenced (& therefore compiled/checked).

#### Async

Async feels not 100% done yet, or maybe it didn't perfectly fit my mental model. 

It seems like Zig has (mostly) the right primitives, but could benefit from some helper functions to implement things like iterators and cancellables.

I wasn't able to really make use of async in a helpful way.

#### Iterators 

I wish there were just _slightly_ more syntactic sugar around iterators.

I don't think this has to conflict with Zig's goal of being explicit, and TBH I think C-style for loops would help a lot here. 

#### Missing minor features (lambdas, ranges)

I had two minor complaints for what seemed like deliberately missing features (both which have workarounds). 

A few times I wanted a lambda -- no capturing or anything fancy, just something basic. Although you can't create an inline function, you *can* create an inline struct with a declared function!

```zig
const fn = struct {
        fn addOne(x: usize) usize {
            x + 1
        }
    }.addOne;

fn(2);
```

I also wanted something like `range(n)` in Python a few times, or even like `for (...)` in C. I understand it's *usually* not the right answer, but there were a few places in Noodle I think it would have been more appropriate.

The two options are:

```zig
var i: usize = 0;
while (i <= 10) : (i += 1) { ... }

// or 

for ([_]u0{0} ** 10) |_, i| { ... }
```


Third Pass: Rust
----------------

### What went well

#### Crate ecosystem

I was confident enough in the Rust ecosystem to grab a parser generator library, rather than roll my own parser as I had done for C/Zig.

### What went poorly

#### Errors, combinators

Although I think the Rust `Option<T>` and `Result<T, E>` datatypes are good, I'm not a huge fan of the proliferation of combinators (e.g. `.and_then(...)`).

Every time I start writing Rust again, it takes a lot to remember what all the options are (though I'm sure this would get better if I wrote Rust more frequently). 
I sort of wish it were easier to search through `Option`, `Result`, and `Vec` by type signature (like Hoogle for Haskell) -- or if there were a cheatsheet for this?

I get that these "zero-cost abstractions" get compiled out, and it does lead to *shorter* code, but as a casual Rustacean it is harder to review or write compared to `if`s or loops.

#### Iterators

Writing custom iterators always feels like such a pain.
(I think I'm just spoiled by Python...)

#### Hard to Prototype?

There's a certain quality to writing C, Zig, or Python code, that I feel like I can "smush" the code into working just enough to get a prototype together.
It's not ship a product, I mean stubbing out one area of the code so that another area can get tested: temporarily breaking down some abstractions just to get things moving/compiling.

I'm really bad at doing this in Rust. I feel like there's a few rules I stub my toe on, even though I agree they're great:

- The rule that prevents you from implementing a nonlocal trait on a nonlocal struct. (Yes there are workarounds, but it's annoying to drag out the boilerplate to run a quick proof-of-concept)
- Error handling, Error types, etc. There is the `unwrap()` escape hatch but it's annoying that it's easier to destroy information than pass through errors.

Other Notes
-----------

### Profiling

I used `perf` for profiling.

I found myself comparing the profiles of the C, Zig, and Rust implementations, trying to determine which parts got slower/faster.
Trying to compare the profile results for two different implementations in different languages for the same algorithm is kind of funny -- it feels like it should be easier to do automatically, but realistically I don't think there would be any good heuristics to perform this comparison automatically.

The `nx_match_transitions`/`Expression.matchTransitions` function was the closest thing that was easiest to compare:
due to the structure of the code, it was the smallest function that wouldn't get inlined by the C/Zig compilers.

On the other hand, I care a lot about the performance of the bitsets, and that was harder to directly compare across different implementations due to heavy inlining.

