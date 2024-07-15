use criterion::{criterion_group, criterion_main, Criterion};
use std::{collections::BTreeMap, sync::Arc};
use zellij_tile::prelude::{ModeInfo, TabInfo};

use zjstatus::{
    config::{ModuleConfig, ZellijState},
    render::{
        formatted_part_from_string_cached, formatted_parts_from_string_cached, FormattedPart,
    },
    widgets::{datetime::DateTimeWidget, mode::ModeWidget, session::SessionWidget, widget::Widget},
};

fn bench_moduleconfig_render_bar(c: &mut Criterion) {
    let config = BTreeMap::from([
        ("format_left".to_owned(), "{mode} #[fg=#89B4FA,bg=#181825,bold]{session} {tabs} {command_1} {command_git_branch} {command_3}".to_owned()),
        ("format_right".to_owned(), "{datetime}".to_owned()),
        ("format_space".to_owned(), "#[bg=#181825]".to_owned()),
    ]);

    let mut module_config = ModuleConfig::new(&config).unwrap();

    let mut widgets: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::new();

    widgets.insert(
        "mode".to_owned(),
        Arc::new(ModeWidget::new(&BTreeMap::from([(
            "mode_normal".to_owned(),
            "#[bg=blue] #[bg=yellow] ".to_owned(),
        )]))),
    );

    widgets.insert(
        "datetime".to_owned(),
        Arc::new(DateTimeWidget::new(&BTreeMap::from([(
            "datetime".to_owned(),
            "#[fg=#6C7086,bg=#181825] {index} {name} ".to_owned(),
        )]))),
    );

    widgets.insert(
        "session".to_owned(),
        Arc::new(SessionWidget::new(&BTreeMap::from([]))),
    );

    let state = ZellijState {
        mode: ModeInfo::default(),
        tabs: vec![TabInfo {
            name: "test".to_owned(),
            active: true,
            ..Default::default()
        }],
        ..Default::default()
    };

    c.bench_function("ModuleConfig::render_bar", |b| {
        b.iter(|| module_config.render_bar(state.clone(), widgets.clone()))
    });
}

fn bench_formattedpart_format_string_with_widgets(c: &mut Criterion) {
    let mut format = FormattedPart::from_format_string(
        "#[fg=#9399B2,bg=#181825,bold,italic] {mode} {datetime} {session} [] ",
        &BTreeMap::new(),
    );

    let mut widgets: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::new();

    widgets.insert(
        "mode".to_owned(),
        Arc::new(ModeWidget::new(&BTreeMap::from([(
            "mode_normal".to_owned(),
            "#[bg=blue] #[bg=yellow] ".to_owned(),
        )]))),
    );

    widgets.insert(
        "datetime".to_owned(),
        Arc::new(DateTimeWidget::new(&BTreeMap::from([(
            "datetime".to_owned(),
            "#[fg=#6C7086,bg=#181825] {index} {name} ".to_owned(),
        )]))),
    );

    widgets.insert(
        "session".to_owned(),
        Arc::new(SessionWidget::new(&BTreeMap::from([]))),
    );

    let state = ZellijState {
        mode: ModeInfo::default(),
        tabs: vec![TabInfo {
            name: "test".to_owned(),
            active: true,
            ..Default::default()
        }],
        cache_mask: 0b00000011,
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
                "#[fg=#9399B2,bg=#181825,bold,italic] {index} {name} [] ",
                &BTreeMap::new(),
            )
        })
    });
}

fn bench_formattedpart_from_format_string_cached(c: &mut Criterion) {
    c.bench_function("formatted_part_from_string_cached", |b| {
        b.iter(|| {
            formatted_part_from_string_cached(
                "#[fg=#9399B2,bg=#181825,bold,italic] {index} {name} [] ",
                &BTreeMap::new(),
            )
        })
    });
}

fn bench_formattedpart_multiple_from_format_string(c: &mut Criterion) {
    c.bench_function("FormattedPart::multiple_from_format_string", |b| {
        b.iter(|| {
            FormattedPart::multiple_from_format_string(
                "#[fg=#9399B2,bg=#181825,bold,italic] {index} {name} [] #[fg=#9399B2,bg=#181825,bold,italic] {index} {name} [] ",
                &BTreeMap::new(),
            )
        })
    });
}

fn bench_formattedparts_from_format_string_cached(c: &mut Criterion) {
    c.bench_function("formatted_parts_from_string_cached", |b| {
        b.iter(|| {
            formatted_parts_from_string_cached(
                "#[fg=#9399B2,bg=#181825,bold,italic] {index} {name} [] #[fg=#9399B2,bg=#181825,bold,italic] {index} {name} [] ",
                &BTreeMap::new(),
            )
        })
    });
}

fn bench_moduleconfig_new(c: &mut Criterion) {
    let mut config = BTreeMap::new();

    config.insert("format_left".to_owned(), "{mode} #[fg=#89B4FA,bg=#181825,bold]{session} {tabs} {command_1} {command_git_branch} {command_3}".to_owned());
    config.insert("format_right".to_owned(), "{datetime}".to_owned());
    config.insert("format_space".to_owned(), "#[bg=#181825]".to_owned());

    c.bench_function("ModuleConfig::new", |b| {
        b.iter(|| ModuleConfig::new(&config))
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    bench_formattedpart_from_format_string(c);
    bench_formattedpart_from_format_string_cached(c);
    bench_formattedpart_multiple_from_format_string(c);
    bench_formattedparts_from_format_string_cached(c);
    bench_formattedpart_format_string_with_widgets(c);
    bench_moduleconfig_new(c);
    bench_moduleconfig_render_bar(c);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
