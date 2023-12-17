use criterion::{criterion_group, criterion_main, Criterion};
use std::{collections::BTreeMap, sync::Arc};
use zellij_tile::prelude::{ModeInfo, TabInfo};

use zjstatus::{
    config::{ModuleConfig, ZellijState},
    render::FormattedPart,
    widgets::{datetime::DateTimeWidget, mode::ModeWidget, session::SessionWidget, widget::Widget},
};

fn bench_formattedpart_format_string_with_widgets(c: &mut Criterion) {
    let format = FormattedPart::from_format_string(
        "#[fg=#9399B2,bg=#181825,bold,italic] {mode} {datetime} {session} [] ".to_string(),
    );

    let mut widgets: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::new();

    widgets.insert(
        "mode".to_string(),
        Arc::new(ModeWidget::new(BTreeMap::from([(
            "mode_normal".to_string(),
            "#[bg=blue] #[bg=yellow] ".to_string(),
        )]))),
    );

    widgets.insert(
        "datetime".to_string(),
        Arc::new(DateTimeWidget::new(BTreeMap::from([(
            "datetime".to_string(),
            "#[fg=#6C7086,bg=#181825] {index} {name} ".to_string(),
        )]))),
    );

    widgets.insert(
        "session".to_string(),
        Arc::new(SessionWidget::new(BTreeMap::from([]))),
    );

    let state = ZellijState {
        mode: ModeInfo::default(),
        tabs: vec![TabInfo {
            name: "test".to_string(),
            active: true,
            ..Default::default()
        }],
        ..Default::default()
    };

    c.bench_function("FormattedPart::format_string_with_widgets", |b| {
        b.iter(|| format.format_string_with_widgets(&widgets, &state))
    });
}

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
    bench_formattedpart_format_string_with_widgets(c);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
