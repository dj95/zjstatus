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
            "tab_normal".to_string(),
            "#[fg=#6C7086,bg=#181825] {index} {name} ".to_string(),
        ),
        (
            "tab_normal_fullscreen".to_string(),
            "#[fg=#6C7086,bg=#181825] {index} {name} ".to_string(),
        ),
        (
            "tab_active".to_string(),
            "#[fg=#9399B2,bg=#181825,bold,italic] {index} {name} ".to_string(),
        ),
    ]);

    let wid = widgets::tabs::TabsWidget::new(config);

    let mut tab_state = TabInfo::default();
    tab_state.name = "test".to_string();
    tab_state.active = true;

    let mut state = ZellijState::default();
    state.tabs = vec![tab_state];

    c.bench_function("widgets::TabsWidget", |b| {
        b.iter(|| {
            wid.process("tabs", state.clone());
        })
    });
}

fn bench_widget_mod(c: &mut Criterion) {
    let config = BTreeMap::from([(
        "mode_normal".to_string(),
        "#[bg=blue] #[bg=yellow] ".to_string(),
    )]);

    let wid = widgets::mode::ModeWidget::new(config);

    let mut state = ZellijState::default();
    state.mode = ModeInfo::default();

    c.bench_function("widgets::ModeWidget", |b| {
        b.iter(|| {
            wid.process("mode", state.clone());
        })
    });
}

fn bench_widget_datetime(c: &mut Criterion) {
    let config = BTreeMap::new();

    let wid = widgets::datetime::DateTimeWidget::new(config);

    let state = ZellijState::default();

    c.bench_function("widgets::DateTimeWidget", |b| {
        b.iter(|| {
            wid.process("datetime", state.clone());
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
