use super::{PENDING_FAILURE_CAPACITY, PendingFailureEnqueue, PendingFailureQueue};
use crate::core::{Errc, Error};
use std::sync::{Arc, Barrier};
use std::thread;

#[test]
fn queue_is_fifo_bounded_and_surfaces_overflow_after_real_failures() {
    let queue = PendingFailureQueue::new();
    let source = queue.source();
    for index in 0..PENDING_FAILURE_CAPACITY {
        assert_eq!(
            source.enqueue(Error::new(Errc::PlatformError, format!("failure-{index}"))),
            PendingFailureEnqueue::Queued
        );
    }
    assert_eq!(
        source.enqueue(Error::new(Errc::PlatformError, "overflow")),
        PendingFailureEnqueue::Overflowed
    );

    for index in 0..PENDING_FAILURE_CAPACITY {
        let Some(error) = source.take() else {
            panic!("queued failure must be retained");
        };
        assert_eq!(error.message(), format!("failure-{index}"));
    }
    let Some(overflow) = source.take() else {
        panic!("overflow must become typed failure");
    };
    assert_eq!(overflow.code(), Errc::InsufficientResources);
    assert!(overflow.message().contains("dropped 1 failure"));
    assert!(source.take().is_none());
}

#[test]
fn sources_are_isolated_and_drop_closes_stale_callbacks() {
    let queue = PendingFailureQueue::new();
    let first = queue.source();
    let second = queue.source();
    first.enqueue(Error::new(Errc::IoError, "first"));
    second.enqueue(Error::new(Errc::Timeout, "second"));

    let Some(first_error) = first.take() else {
        panic!("first source");
    };
    let Some(second_error) = second.take() else {
        panic!("second source");
    };
    assert_eq!(first_error.code(), Errc::IoError);
    assert_eq!(second_error.code(), Errc::Timeout);

    let callback = first.clone();
    drop(first);
    callback.enqueue(Error::new(Errc::PlatformError, "late"));
    drop(callback);
    assert!(second.take().is_none());
}

#[test]
fn concurrent_callback_producers_preserve_capacity_and_one_overflow_signal() {
    const PRODUCER_COUNT: usize = 4;
    const ATTEMPTS_PER_PRODUCER: usize = PENDING_FAILURE_CAPACITY / 2;

    assert_eq!(PENDING_FAILURE_CAPACITY, 64);
    let queue = PendingFailureQueue::new();
    let source = queue.source();
    let start = Arc::new(Barrier::new(PRODUCER_COUNT));
    let mut producers = Vec::with_capacity(PRODUCER_COUNT);

    for producer_id in 0..PRODUCER_COUNT {
        let source = source.clone();
        let start = Arc::clone(&start);
        producers.push(thread::spawn(move || {
            start.wait();
            let mut queued = 0;
            let mut overflowed = 0;
            for attempt in 0..ATTEMPTS_PER_PRODUCER {
                let result = source.enqueue(Error::new(
                    Errc::PlatformError,
                    format!("producer-{producer_id}-{attempt}"),
                ));
                match result {
                    PendingFailureEnqueue::Queued => queued += 1,
                    PendingFailureEnqueue::Overflowed => overflowed += 1,
                    PendingFailureEnqueue::Closed => {
                        panic!("the active source must not close during producer pressure")
                    }
                }
            }
            (queued, overflowed)
        }));
    }

    let (queued, overflowed) = producers
        .into_iter()
        .map(|producer| {
            producer
                .join()
                .unwrap_or_else(|_| panic!("callback producer must not panic"))
        })
        .fold(
            (0, 0),
            |(queued, overflowed), (producer_queued, producer_overflowed)| {
                (queued + producer_queued, overflowed + producer_overflowed)
            },
        );
    assert_eq!(queued, PENDING_FAILURE_CAPACITY);
    assert_eq!(overflowed, PRODUCER_COUNT * ATTEMPTS_PER_PRODUCER - queued);

    // 所有生产者只入队。只有 owner 线程排空源，并且必须先看到真实失败，
    // 随后才是一条预算信号。
    let mut real_failures = 0;
    let mut overflow_signals = 0;
    while let Some(error) = source.take() {
        if error.code() == Errc::InsufficientResources {
            overflow_signals += 1;
        } else {
            assert_eq!(error.code(), Errc::PlatformError);
            real_failures += 1;
        }
    }
    assert_eq!(real_failures, PENDING_FAILURE_CAPACITY);
    assert_eq!(overflow_signals, 1);
    assert!(source.take().is_none());
}

#[test]
fn closed_generation_releases_capacity_before_replacement_pressure() {
    let queue = PendingFailureQueue::new();
    let old_generation = queue.source();
    let late_callback = old_generation.clone();
    old_generation.enqueue(Error::new(Errc::PlatformError, "old generation"));
    old_generation.close();

    let replacement = queue.source();
    assert_eq!(
        late_callback.enqueue(Error::new(Errc::PlatformError, "late old generation")),
        PendingFailureEnqueue::Closed
    );
    for index in 0..PENDING_FAILURE_CAPACITY {
        assert_eq!(
            replacement.enqueue(Error::new(
                Errc::PlatformError,
                format!("replacement-{index}"),
            )),
            PendingFailureEnqueue::Queued
        );
    }

    let mut replacement_failures = 0;
    while replacement.take().is_some() {
        replacement_failures += 1;
    }
    assert_eq!(replacement_failures, PENDING_FAILURE_CAPACITY);
    assert!(old_generation.take().is_none());
}
