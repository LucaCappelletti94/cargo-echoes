//! Identical bodies at different source sites group together.

#![no_main]

use echoes::{DocTest, group};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(code) = std::str::from_utf8(data) else {
        return;
    };
    let code = code.to_owned();
    echoes_fuzz::on_large_stack(move || {
        let site = |item: &str| DocTest {
            file: format!("src/{item}.rs"),
            line: 1,
            end: None,
            item: format!("c::{item}"),
            info: Vec::new(),
            code: code.clone(),
            allow: false,
            self_type: None,
            public: false,
        };
        let report = group(&[site("first"), site("second")], 1, 0);
        let mut members: Vec<_> = report
            .groups
            .iter()
            .map(|group| {
                let mut sites: Vec<_> = group.sites.iter().map(|site| site.item.as_str()).collect();
                sites.sort_unstable();
                sites
            })
            .collect();
        members.sort_unstable();
        assert_eq!(members, [vec!["c::first", "c::second"]]);
    });
});
