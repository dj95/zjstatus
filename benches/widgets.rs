use chrono::{Duration, Local};
use criterion::{criterion_group, criterion_main, Criterion};
use std::{collections::BTreeMap, ops::Sub};
use zellij_tile::prelude::*;

use zjstatus::{
    config::ZellijState,
    widgets::{self, widget::Widget},
};

fn bench_widget_command(c: &mut Criterion) {
    let config = BTreeMap::from([
        (
            "command_test_format".to_owned(),
            "#[fg=#9399B2,bg=#181825,bold,italic] {exit_code} {stdout} ".to_owned(),
        ),
        ("command_test_interval".to_owned(), "100".to_owned()),
    ]);

    let wid = widgets::command::CommandWidget::new(&config);

    let ts = Local::now().sub(Duration::try_seconds(1).unwrap());

    let state = ZellijState {
        command_results: BTreeMap::from([(
            "command_test".to_owned(),
            widgets::command::CommandResult {
                exit_code: Some(0),
                stdout: "test".to_owned(),
                stderr: "".to_owned(),
                context: BTreeMap::from([(
                    "timestamp".to_owned(),
                    ts.format(widgets::command::TIMESTAMP_FORMAT).to_string(),
                )]),
            },
        )]),
        ..Default::default()
    };

    c.bench_function("widgets::CommandWidget (static)", |b| {
        b.iter(|| {
            wid.process("command_test", &state);
        })
    });
}

fn bench_widget_command_dynamic(c: &mut Criterion) {
    let config = BTreeMap::from([
        (
            "command_test_format".to_owned(),
            "#[fg=#9399B2,bg=#181825,bold,italic] {exit_code} {stdout} ".to_owned(),
        ),
        ("command_test_interval".to_owned(), "100".to_owned()),
        ("command_test_rendermode".to_owned(), "dynamic".to_owned()),
    ]);

    let wid = widgets::command::CommandWidget::new(&config);

    let ts = Local::now().sub(Duration::try_seconds(1).unwrap());

    let state = ZellijState {
        command_results: BTreeMap::from([(
            "command_test".to_owned(),
            widgets::command::CommandResult {
                exit_code: Some(0),
                stdout: "#[fg=#9399B2,bg=#181825,bold,italic] test #[fg=#9399B2,bg=#181825,bold,italic] test".to_owned(),
                stderr: "".to_owned(),
                context: BTreeMap::from([(
                    "timestamp".to_owned(),
                    ts.format(widgets::command::TIMESTAMP_FORMAT).to_string(),
                )]),
            },
        )]),
        ..Default::default()
    };

    c.bench_function("widgets::CommandWidget (dynamic)", |b| {
        b.iter(|| {
            wid.process("command_test", &state);
        })
    });
}

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
    bench_widget_command(c);
    bench_widget_command_dynamic(c);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
