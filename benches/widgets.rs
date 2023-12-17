use criterion::{criterion_group, criterion_main, Criterion};
use zjstatus::config::ZellijState;
use std::collections::BTreeMap;

use zjstatus::widgets;
use zjstatus::widgets::widget::Widget;

fn bench_widget_datetime(c: &mut Criterion) {
    let config = BTreeMap::new();

    let wid = widgets::datetime::DateTimeWidget::new(config);

    let state = ZellijState::default();

    c.bench_function("widgets::DateTime", |b| {
        b.iter(|| {
            wid.process("", state.clone());
        })
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    bench_widget_datetime(c);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
