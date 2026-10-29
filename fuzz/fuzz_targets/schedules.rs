//! Legal typed schedules, algebra, boolean rewrites, value and literal folds, field order, literal
//! alternatives, item order and inherent `impl` partitions group apart from changed neighbors.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// The checks, one per input, chosen by the input's last byte so the leading bytes keep their meaning.
const CHECKS: [fn(&[u8]); 7] = [
    echoes_fuzz::check_dependency_schedules,
    echoes_fuzz::check_boolean_schedules,
    echoes_fuzz::check_value_schedules,
    echoes_fuzz::check_helper_schedules,
    echoes_fuzz::check_structure_schedules,
    echoes_fuzz::check_item_schedules,
    echoes_fuzz::check_concat_literals,
];

fuzz_target!(|data: &[u8]| {
    let check = CHECKS[usize::from(data.last().copied().unwrap_or(0)) % CHECKS.len()];
    let data = data.to_vec();
    echoes_fuzz::on_large_stack(move || check(&data));
});
