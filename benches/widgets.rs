use criterion::{criterion_group, criterion_main, Criterion};
use std::collections::BTreeMap;
use zellij_tile::prelude::*;

use zjstatus::{
    config::ZellijState,
    widgets::{self, widget::Widget},
};

fn bench_widget_tabs(c: &mut Criterion) {
    let config = BTreeMap::from([
        (
            "tab_normal".to_owned(),
            "#[fg=#6C7086,bg=#181825] {index} {name} ".to_owned(),
        ),
        (
            "tab_normal_fullscreen".to_owned(),
            "#[fg=#6C7086,bg=#181825] {index} {name} ".to_owned(),
        ),
        (
            "tab_active".to_owned(),
            "#[fg=#9399B2,bg=#181825,bold,italic] {index} {name} ".to_owned(),
        ),
    ]);

    let wid = widgets::tabs::TabsWidget::new(&config);

    let state = ZellijState {
        tabs: vec![TabInfo {
            name: "test".to_owned(),
            active: true,
            ..Default::default()
        }],
        ..Default::default()
    };

    c.bench_function("widgets::TabsWidget", |b| {
        b.iter(|| {
            wid.process("tabs", &state);
        })
    });
}

fn bench_widget_mod(c: &mut Criterion) {
    let config = BTreeMap::from([(
        "mode_normal".to_owned(),
        "#[bg=blue] #[bg=yellow] ".to_owned(),
    )]);

    let wid = widgets::mode::ModeWidget::new(&config);

    let state = ZellijState {
        mode: ModeInfo::default(),
        ..Default::default()
    };

    c.bench_function("widgets::ModeWidget", |b| {
        b.iter(|| {
            wid.process("mode", &state);
        })
    });
}

fn bench_widget_datetime(c: &mut Criterion) {
    let config = BTreeMap::new();

    let wid = widgets::datetime::DateTimeWidget::new(&config);

    let state = ZellijState::default();

    c.bench_function("widgets::DateTimeWidget", |b| {
        b.iter(|| {
            wid.process("datetime", &state);
        })
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    bench_widget_datetime(c);
    bench_widget_mod(c);
    bench_widget_tabs(c);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
