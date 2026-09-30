use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use regex::Regex;
use tglink::ParseTgLink;

const SIZE: usize = 1 << 20; // 1 MiB

const PLAIN: &str =
    "The quick brown fox jumps over the lazy dog, then takes the total to the station. ";

const LINKS: &str =
    "hello.t.me https://t.me/durov @durov tg://user?id=42 telegram.me/durov t.me/-100123456 ";

fn repeat(s: &str) -> String {
    s.repeat(SIZE / s.len() + 1)
}

fn sparse() -> String {
    let block = PLAIN.repeat(12);
    let mut out = String::with_capacity(SIZE + block.len());

    while out.len() < SIZE {
        out.push_str(&block);
        out.push_str("https://juzocode.t.me/1 ");
    }

    out
}

fn bench(c: &mut Criterion) {
    // RU:  Примерный аналог на Regex
    // ENG: A rough Regex equivalent
    let re = Regex::new(
        r"(?i)(?:@|\bt\.me/|\btelegram\.(?:me|dog)/)[a-z][a-z0-9_]*|\b[a-z][a-z0-9_]*\.t\.me\b|tg://(?:user\?id=|resolve\?domain=|openmessage\?user_id=)[a-z0-9_]+",
    )
    .unwrap();

    let cases = [
        ("plain (no links)", repeat(PLAIN)),
        ("sparse (1 link / KiB)", sparse()),
        ("dense (links only)", repeat(LINKS)),
    ];

    for (name, text) in &cases {
        let text: &str = text.as_str();

        let mut g = c.benchmark_group(*name);
        g.throughput(Throughput::Bytes(text.len() as u64));

        g.bench_function("tglink", |b| {
            b.iter(|| ParseTgLink::all(black_box(text)).count())
        });

        g.bench_function("regex", |b| {
            b.iter(|| {
                re.find_iter(black_box(text))
                    .count()
            })
        });

        g.finish();
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
