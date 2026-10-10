use super::*;
use chrono::Days;

/// A first pass places every note of a space, which for a heavy writer
/// is many thousands. Only a gate in release, as the other timing tests,
/// and a tenth of the notes in a debug build, which is many times slower:
/// `cargo test --release placing_is_fast -- --nocapture`.
#[test]
fn placing_is_fast_enough_for_ten_thousand_notes() {
    const DIMS: usize = 768;
    let count: u64 = if cfg!(debug_assertions) {
        1_000
    } else {
        10_000
    };
    let mut state: u32 = 0x9e37_79b9;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32 - 0.5
    };
    let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let mut vectors = Vectors::new("m", DIMS);
    let mut when = HashMap::new();
    // Fourteen notes a day for two years, about nothing in common.
    for i in 0..count {
        let id = format!("{i:05}");
        let vector: Vec<f32> = (0..DIMS).map(|_| next()).collect();
        vectors.insert(id.clone(), "h".into(), vector).unwrap();
        let date = start.checked_add_days(Days::new(i / 14)).unwrap();
        when.insert(
            id,
            When {
                date,
                time: format!("{:02}:00", i % 14 + 8),
            },
        );
    }

    let started = std::time::Instant::now();
    let mut threads = Threads::default();
    threads.reconcile(&vectors, &when, &Edits::default());
    let took = started.elapsed();
    eprintln!("placed {count} notes in {took:?}");
    if !cfg!(debug_assertions) {
        assert!(took.as_secs_f32() < 5.0, "took {took:?}");
    }
}
