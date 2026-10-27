# The Book

The book's source, which mdBook builds into `book/`. Every page is a Markdown
file under `src/`, listed in `src/SUMMARY.md`: a page missing from it is never
published, and the checks fail on it.

- `just check-mdbook` lints the book, builds it, and runs its Rust examples.
- `mdbook serve` in this directory serves it, rebuilt on every change.

The book's own look is in `theme/`: the page width, collapsible definitions, and
a `$` prompt on `console` blocks.

Every listing is included from code the tests build, under
`crates/atomix/tests/` or among the examples, so it compiles against the crates
as they are. The book's own tests, under `crates/atomix/tests/book/`, run with:

```sh
cargo test -p atomix --all-features --test book
```

A page links an item of the API to the facade's page, `api/atomix/…`, in a
reference definition at its foot, since a path would resolve to the crate that
defines the item, `atomix_core`. The links preprocessor builds the API into the
book, with every feature on, and lychee checks each link and its fragment.
