use std::error::Error;
use std::fmt::{Display, Formatter};
use std::hint::black_box;
use std::time::{Duration, Instant};

const BENCHMARK_MESSAGE: &str = "  benchmark message  ";
const EXPECTED_MESSAGE: &str = "benchmark message";
const WARMUP_ITERATIONS: usize = 20_000;
const MEASURED_ITERATIONS: usize = 300_000;
const MEASURED_ROUNDS: usize = 7;

#[derive(Debug, PartialEq, Eq)]
struct DiagnosticEntry {
    sequence: u64,
    message: String,
}

#[derive(Debug, PartialEq, Eq)]
enum DiagnosticsError {
    EmptyMessage,
    SequenceExhausted,
    VerificationFailed(&'static str),
}

impl Display for DiagnosticsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMessage => formatter.write_str("diagnostic message must not be empty"),
            Self::SequenceExhausted => formatter.write_str("diagnostic sequence is exhausted"),
            Self::VerificationFailed(reason) => {
                write!(formatter, "benchmark verification failed: {reason}")
            }
        }
    }
}

impl Error for DiagnosticsError {}

trait MessageNormalizer: Send + Sync {
    fn normalize(&self, message: &str) -> Result<String, DiagnosticsError>;
}

impl<T> MessageNormalizer for Box<T>
where
    T: MessageNormalizer + ?Sized,
{
    fn normalize(&self, message: &str) -> Result<String, DiagnosticsError> {
        (**self).normalize(message)
    }
}

struct TrimNormalizer;

impl MessageNormalizer for TrimNormalizer {
    fn normalize(&self, message: &str) -> Result<String, DiagnosticsError> {
        normalize_message(message)
    }
}

struct AlternateTrimNormalizer;

impl MessageNormalizer for AlternateTrimNormalizer {
    fn normalize(&self, message: &str) -> Result<String, DiagnosticsError> {
        normalize_message(message)
    }
}

fn normalize_message(message: &str) -> Result<String, DiagnosticsError> {
    let normalized = message.trim();
    if normalized.is_empty() {
        return Err(DiagnosticsError::EmptyMessage);
    }
    Ok(normalized.to_owned())
}

trait EntryStore: Send + Sync {
    fn append(&mut self, entry: DiagnosticEntry) -> Result<(), DiagnosticsError>;
    fn snapshot(&self) -> Vec<DiagnosticEntry>;
}

impl<T> EntryStore for Box<T>
where
    T: EntryStore + ?Sized,
{
    fn append(&mut self, entry: DiagnosticEntry) -> Result<(), DiagnosticsError> {
        (**self).append(entry)
    }

    fn snapshot(&self) -> Vec<DiagnosticEntry> {
        (**self).snapshot()
    }
}

struct VecEntryStore {
    entries: Vec<DiagnosticEntry>,
}

impl VecEntryStore {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }
}

impl EntryStore for VecEntryStore {
    fn append(&mut self, entry: DiagnosticEntry) -> Result<(), DiagnosticsError> {
        self.entries.push(entry);
        Ok(())
    }

    fn snapshot(&self) -> Vec<DiagnosticEntry> {
        copy_entries(&self.entries)
    }
}

struct AlternateVecEntryStore {
    entries: Vec<DiagnosticEntry>,
}

impl AlternateVecEntryStore {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }
}

impl EntryStore for AlternateVecEntryStore {
    fn append(&mut self, entry: DiagnosticEntry) -> Result<(), DiagnosticsError> {
        self.entries.push(entry);
        Ok(())
    }

    fn snapshot(&self) -> Vec<DiagnosticEntry> {
        copy_entries(&self.entries)
    }
}

fn copy_entries(entries: &[DiagnosticEntry]) -> Vec<DiagnosticEntry> {
    entries
        .iter()
        .map(|entry| DiagnosticEntry {
            sequence: entry.sequence,
            message: entry.message.to_owned(),
        })
        .collect()
}

struct ReportingModule<N>
where
    N: MessageNormalizer,
{
    normalizer: N,
}

impl<N> ReportingModule<N>
where
    N: MessageNormalizer,
{
    fn new(normalizer: N) -> Self {
        Self { normalizer }
    }

    fn prepare(&self, message: &str) -> Result<String, DiagnosticsError> {
        self.normalizer.normalize(message)
    }
}

struct LoggingModule<S>
where
    S: EntryStore,
{
    store: S,
    next_sequence: u64,
}

impl<S> LoggingModule<S>
where
    S: EntryStore,
{
    fn new(store: S) -> Self {
        Self {
            store,
            next_sequence: 1,
        }
    }

    fn record(&mut self, message: String) -> Result<(), DiagnosticsError> {
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(DiagnosticsError::SequenceExhausted)?;
        self.store.append(DiagnosticEntry { sequence, message })
    }

    fn snapshot(&self) -> Vec<DiagnosticEntry> {
        self.store.snapshot()
    }
}

struct DiagnosticsSystem<N, S>
where
    N: MessageNormalizer,
    S: EntryStore,
{
    reporting: ReportingModule<N>,
    logging: LoggingModule<S>,
}

impl<N, S> DiagnosticsSystem<N, S>
where
    N: MessageNormalizer,
    S: EntryStore,
{
    fn new(normalizer: N, store: S) -> Self {
        Self {
            reporting: ReportingModule::new(normalizer),
            logging: LoggingModule::new(store),
        }
    }

    fn report(&mut self, message: &str) -> Result<(), DiagnosticsError> {
        let normalized = self.reporting.prepare(message)?;
        self.logging.record(normalized)
    }

    fn entries(&self) -> Vec<DiagnosticEntry> {
        self.logging.snapshot()
    }
}

type StaticDiagnostics = DiagnosticsSystem<TrimNormalizer, VecEntryStore>;
type DynamicDiagnostics = DiagnosticsSystem<Box<dyn MessageNormalizer>, Box<dyn EntryStore>>;

fn new_static_diagnostics(capacity: usize) -> StaticDiagnostics {
    DiagnosticsSystem::new(TrimNormalizer, VecEntryStore::with_capacity(capacity))
}

#[inline(never)]
fn select_normalizer(primary: bool) -> Box<dyn MessageNormalizer> {
    if primary {
        Box::new(TrimNormalizer)
    } else {
        Box::new(AlternateTrimNormalizer)
    }
}

#[inline(never)]
fn select_store(primary: bool, capacity: usize) -> Box<dyn EntryStore> {
    if primary {
        Box::new(VecEntryStore::with_capacity(capacity))
    } else {
        Box::new(AlternateVecEntryStore::with_capacity(capacity))
    }
}

fn new_dynamic_diagnostics(capacity: usize) -> DynamicDiagnostics {
    let runtime_choice = black_box(true);
    DiagnosticsSystem::new(
        select_normalizer(runtime_choice),
        select_store(runtime_choice, capacity),
    )
}

fn run_static(iterations: usize) -> Result<Duration, DiagnosticsError> {
    let mut diagnostics = new_static_diagnostics(iterations);
    let started = Instant::now();
    for _ in 0..iterations {
        diagnostics.report(black_box(BENCHMARK_MESSAGE))?;
    }
    let elapsed = started.elapsed();
    verify_entries(&diagnostics.entries(), iterations)?;
    Ok(elapsed)
}

fn run_dynamic(iterations: usize) -> Result<Duration, DiagnosticsError> {
    let mut diagnostics = new_dynamic_diagnostics(iterations);
    let started = Instant::now();
    for _ in 0..iterations {
        diagnostics.report(black_box(BENCHMARK_MESSAGE))?;
    }
    let elapsed = started.elapsed();
    verify_entries(&diagnostics.entries(), iterations)?;
    Ok(elapsed)
}

fn run_flat(iterations: usize) -> Result<Duration, DiagnosticsError> {
    let mut entries = Vec::with_capacity(iterations);
    let mut next_sequence = 1_u64;
    let started = Instant::now();
    for _ in 0..iterations {
        let normalized = normalize_message(black_box(BENCHMARK_MESSAGE))?;
        let sequence = next_sequence;
        next_sequence = next_sequence
            .checked_add(1)
            .ok_or(DiagnosticsError::SequenceExhausted)?;
        entries.push(DiagnosticEntry {
            sequence,
            message: normalized,
        });
    }
    let elapsed = started.elapsed();
    verify_entries(&copy_entries(&entries), iterations)?;
    Ok(elapsed)
}

fn verify_entries(entries: &[DiagnosticEntry], iterations: usize) -> Result<(), DiagnosticsError> {
    if entries.len() != iterations {
        return Err(DiagnosticsError::VerificationFailed(
            "unexpected entry count",
        ));
    }
    let Some(last_entry) = entries.last() else {
        return Err(DiagnosticsError::VerificationFailed("missing final entry"));
    };
    if last_entry.sequence != iterations as u64 || last_entry.message != EXPECTED_MESSAGE {
        return Err(DiagnosticsError::VerificationFailed(
            "observable behavior differs from the expected contract",
        ));
    }
    Ok(())
}

fn median(samples: &mut [Duration]) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn operations_per_second(iterations: usize, elapsed: Duration) -> f64 {
    iterations as f64 / elapsed.as_secs_f64()
}

fn main() -> Result<(), Box<dyn Error>> {
    black_box(run_static(WARMUP_ITERATIONS)?);
    black_box(run_dynamic(WARMUP_ITERATIONS)?);
    black_box(run_flat(WARMUP_ITERATIONS)?);

    let mut static_samples = Vec::with_capacity(MEASURED_ROUNDS);
    let mut dynamic_samples = Vec::with_capacity(MEASURED_ROUNDS);
    let mut flat_samples = Vec::with_capacity(MEASURED_ROUNDS);

    for round in 0..MEASURED_ROUNDS {
        match round % 3 {
            0 => {
                static_samples.push(run_static(MEASURED_ITERATIONS)?);
                dynamic_samples.push(run_dynamic(MEASURED_ITERATIONS)?);
                flat_samples.push(run_flat(MEASURED_ITERATIONS)?);
            }
            1 => {
                dynamic_samples.push(run_dynamic(MEASURED_ITERATIONS)?);
                flat_samples.push(run_flat(MEASURED_ITERATIONS)?);
                static_samples.push(run_static(MEASURED_ITERATIONS)?);
            }
            _ => {
                flat_samples.push(run_flat(MEASURED_ITERATIONS)?);
                static_samples.push(run_static(MEASURED_ITERATIONS)?);
                dynamic_samples.push(run_dynamic(MEASURED_ITERATIONS)?);
            }
        }
    }

    let static_median = median(&mut static_samples);
    let dynamic_median = median(&mut dynamic_samples);
    let flat_median = median(&mut flat_samples);

    print_result("static", static_median, flat_median);
    print_result("dynamic", dynamic_median, flat_median);
    print_result("flat", flat_median, flat_median);
    Ok(())
}

fn print_result(name: &str, elapsed: Duration, flat_elapsed: Duration) {
    let ratio = elapsed.as_secs_f64() / flat_elapsed.as_secs_f64();
    println!("implementation={name}");
    println!("iterations_per_round={MEASURED_ITERATIONS}");
    println!("rounds={MEASURED_ROUNDS}");
    println!("median_ns={}", elapsed.as_nanos());
    println!(
        "operations_per_second={:.0}",
        operations_per_second(MEASURED_ITERATIONS, elapsed)
    );
    println!("ratio_to_flat={ratio:.3}");
    println!("overhead_percent={:.1}", (ratio - 1.0) * 100.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_and_dynamic_composition_preserve_the_same_contract() -> Result<(), DiagnosticsError> {
        let mut static_diagnostics = new_static_diagnostics(2);
        let mut dynamic_diagnostics = new_dynamic_diagnostics(2);

        for diagnostics_message in ["  first  ", "second"] {
            static_diagnostics.report(diagnostics_message)?;
            dynamic_diagnostics.report(diagnostics_message)?;
        }

        assert_eq!(static_diagnostics.entries(), dynamic_diagnostics.entries());
        Ok(())
    }

    #[test]
    fn failed_report_does_not_mutate_state() -> Result<(), DiagnosticsError> {
        let mut diagnostics = new_static_diagnostics(1);

        assert_eq!(
            diagnostics.report("   "),
            Err(DiagnosticsError::EmptyMessage)
        );
        assert!(diagnostics.entries().is_empty());
        Ok(())
    }
}
