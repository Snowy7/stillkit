//! A `Waker` ends the UI thread's idle wait from another thread, coalesces, and leaves no wake
//! behind once drained. Needs a real event queue on the main thread (AppKit, Win32), hence
//! `harness = false`; Linux needs a compositor and is skipped.
fn main() {
    #[cfg(any(target_os = "macos", windows))]
    run();
}

#[cfg(any(target_os = "macos", windows))]
fn run() {
    use std::time::{Duration, Instant};
    use stillkit::platform::current::host::{wait, wait_for};
    use stillkit::{Kind, Role, Surface, next_event, waker};

    // A surface launches the platform's queue; it is never shown.
    let _surface = Surface::new(Kind::Popup { activate: false }, "waker", Role::Window).unwrap();
    while next_event().is_some() {}
    let waker = waker();

    // A wake from another thread ends an otherwise endless wait.
    let started = Instant::now();
    let remote = waker.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        for _ in 0..50 {
            remote.wake(); // coalesced into one native message
        }
    });
    wait();
    let waited = started.elapsed();
    thread.join().unwrap();
    assert!(waited >= Duration::from_millis(90), "woke before the wake: {waited:?}");
    assert!(waited < Duration::from_secs(5), "the wake never ended the wait: {waited:?}");

    // The coalesced wakes left at most one message; after it, a wait sleeps its full limit.
    wait_for(Duration::from_millis(10));
    let started = Instant::now();
    wait_for(Duration::from_millis(200));
    assert!(started.elapsed() >= Duration::from_millis(150), "a stale wake ended the wait: {:?}", started.elapsed());

    // A wake sent while the app is busy (between waits) still ends the next wait.
    waker.wake();
    let started = Instant::now();
    wait_for(Duration::from_secs(5));
    assert!(started.elapsed() < Duration::from_secs(1), "a wake between waits was lost");
    println!("waker: ok");
}
