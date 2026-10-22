//! The book's listings, each a test, so a page shows code that compiles and runs.
//!
//! A page includes a listing by the anchor around it; a chapter's listings are in the module named
//! for it, and the cookbook's in `cookbook`, which takes each recipe's structure from its example
//! or from `tests/cookbook/`, as the loom models in `tests/model.rs` do. Loom's atomics exist only
//! inside a model, so nothing here builds under loom.

#![feature(const_trait_impl)]

#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod arrays_and_element_places;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod cookbook;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod deriving_atom;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod fields;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod getting_started;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod orderings;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod ranged_integers;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod references_and_function_pointers;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod tagged_pointers;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod tuples_and_core_values;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
#[cfg(wide)]
mod two_words;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod values_and_places;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod values_and_their_reprs;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod what_each_operation_costs;
#[cfg(test)]
#[cfg(all(not(loom), feature = "derive"))]
mod working_with_other_crates;
