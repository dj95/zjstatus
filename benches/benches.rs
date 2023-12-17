use criterion::{criterion_group, criterion_main, Criterion};
use std::collections::BTreeMap;

use zjstatus::{config::ModuleConfig, render::FormattedPart};

fn bench_formattedpart_from_format_string(c: &mut Criterion) {
    c.bench_function("FormattedPart::from_format_string", |b| {
        b.iter(|| {
            FormattedPart::from_format_string(
                "#[fg=#9399B2,bg=#181825,bold,italic] {index} {name} [] ".to_string(),
            )
        })
    });
}

fn bench_moduleconfig_new(c: &mut Criterion) {
    let mut config = BTreeMap::new();

    config.insert("format_left".to_string(), "{mode} #[fg=#89B4FA,bg=#181825,bold]{session} {tabs} {command_1} {command_git_branch} {command_3}".to_string());
    config.insert("format_right".to_string(), "{datetime}".to_string());
    config.insert("format_space".to_string(), "#[bg=#181825]".to_string());

    c.bench_function("ModuleConfig::new", |b| {
        b.iter(|| ModuleConfig::new(config.clone()))
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    bench_formattedpart_from_format_string(c);
    bench_moduleconfig_new(c);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
