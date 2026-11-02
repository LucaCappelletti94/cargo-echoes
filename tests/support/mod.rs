//! Helpers shared by the integration test modules.

#[cfg(feature = "cli")]
pub(crate) mod cli;
#[cfg(feature = "cli")]
pub(crate) mod context_scan;
pub(crate) mod dependency;
pub(crate) mod dependency_value_oracle;
pub(crate) mod doc_blocks;
pub(crate) mod failing;
pub(crate) mod fixture_group;
pub(crate) mod form;
pub(crate) mod function;
pub(crate) mod function_ids;
pub(crate) mod group_members;
pub(crate) mod grouped_file;
pub(crate) mod impl_item;
pub(crate) mod method_form;
pub(crate) mod permutations;
pub(crate) mod public_sites;
pub(crate) mod put;
pub(crate) mod same;
pub(crate) mod separate;
pub(crate) mod site_file;
pub(crate) mod site_src;
pub(crate) mod spans;
pub(crate) mod trait_forms;
pub(crate) mod witnesses;
