//! Optional per-phase wall-clock timers. With the `phase-timing` feature off, `ph!` is just its
//! expression and nothing is measured.

#[cfg(feature = "phase-timing")]
thread_local! {
    static TOTALS: std::cell::RefCell<std::collections::BTreeMap<&'static str, u128>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

#[cfg(feature = "phase-timing")]
pub fn record(name: &'static str, nanos: u128) {
    TOTALS.with(|totals| *totals.borrow_mut().entry(name).or_default() += nanos);
}

/// Take and clear the accumulated nanoseconds per phase on this thread.
#[cfg(feature = "phase-timing")]
pub fn take() -> Vec<(&'static str, u128)> {
    TOTALS.with(|totals| std::mem::take(&mut *totals.borrow_mut()).into_iter().collect())
}

#[cfg(not(feature = "phase-timing"))]
pub fn take() -> Vec<(&'static str, u128)> {
    Vec::new()
}

#[cfg(feature = "phase-timing")]
pub type Started = std::time::Instant;
#[cfg(not(feature = "phase-timing"))]
pub type Started = ();

#[inline(always)]
pub fn start() -> Started {
    #[cfg(feature = "phase-timing")]
    return std::time::Instant::now();
}

#[inline(always)]
#[allow(unused_variables)]
pub fn stop(name: &'static str, started: Started) {
    #[cfg(feature = "phase-timing")]
    record(name, started.elapsed().as_nanos());
}

/// Run an expression, recording its wall time under `name` when `phase-timing` is on.
#[macro_export]
macro_rules! ph {
    ($name:expr, $body:expr) => {{
        let started = $crate::prof::start();
        let out = $body;
        $crate::prof::stop($name, started);
        out
    }};
}
