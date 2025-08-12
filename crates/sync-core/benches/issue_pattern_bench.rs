use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use sync_utils::issue_pattern::find_issue_number;

fn bench_find_issue_number(c: &mut Criterion) {
  let mut group = c.benchmark_group("find_issue_number");

  // Typical subject lines
  let samples = vec![
    "ABC-123 Fix the bug",
    "[tag] XYZ-999: title",
    "feat(api): JIRA-42 handle edge",
    "chore: no issue here",
    "Fix JIRA-456 in code",
    "ABC-123 in subject\nDEF-456 in body",
    "prefixABC-123",
    "A-1 minimal",
    "[threading] IJPL-163558: Fix observability",
    "feat(auth): DEF-456 add login",
  ];

  for s in &samples {
    group.bench_with_input(BenchmarkId::new("single", s), s, |b, input| {
      b.iter(|| {
        let res = find_issue_number(black_box(input));
        black_box(res);
      })
    });
  }

  // Random-ish long inputs to simulate worst-case scanning
  let long_without = "a".repeat(1024) + " something";
  let long_with = "[category] fix: ".to_string() + &"A".repeat(20) + "-" + &"1".repeat(10) + " rest";

  group.bench_with_input(BenchmarkId::new("long", "no_issue"), &long_without, |b, input| {
    b.iter(|| {
      let res = find_issue_number(black_box(input));
      black_box(res);
    })
  });

  group.bench_with_input(BenchmarkId::new("long", "with_issue"), &long_with, |b, input| {
    b.iter(|| {
      let res = find_issue_number(black_box(input));
      black_box(res);
    })
  });

  // Throughput style: run over a batch of inputs per iteration
  let corpus: Vec<String> = (0..1000)
    .map(|i| if i % 3 == 0 { format!("feat: ABC-{i} work") } else { format!("random text {i}") })
    .collect();
  group.throughput(Throughput::Elements(corpus.len() as u64));
  group.bench_function("corpus_scan", |b| {
    b.iter_batched(
      || corpus.clone(),
      |inputs| {
        let mut count = 0u64;
        for s in inputs {
          if find_issue_number(&s).is_some() {
            count += 1;
          }
        }
        black_box(count)
      },
      BatchSize::SmallInput,
    )
  });

  group.finish();
}

criterion_group!(benches, bench_find_issue_number);
criterion_main!(benches);
