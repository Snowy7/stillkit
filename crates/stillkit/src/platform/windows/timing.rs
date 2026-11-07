use std::time::{Duration, Instant};
use anyhow::{Context, Result};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};
use windows::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CancelWaitableTimer, CreateWaitableTimerExW,
    SetWaitableTimerEx, TIMER_ALL_ACCESS,
};

/// Wakes the UI loop once per display frame while something moves, and never otherwise. Wait
/// on [`handle`](Self::handle) with the window messages; call [`schedule`](Self::schedule)
/// after each pass of the loop.
pub struct Ticker {
    timer: HANDLE,
    frame: Duration,
    /// When the armed tick is due; a due time in the past has fired.
    due: Option<Instant>,
}

impl Ticker {
    pub fn new() -> Result<Self> {
        // SAFETY: creates an unnamed timer owned by this struct.
        let timer = unsafe {
            CreateWaitableTimerExW(
                None,
                None,
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )
        }
        .context("high-resolution waitable timer")?;
        Ok(Self {
            timer,
            frame: display_frame(),
            due: None,
        })
    }

    /// Signalled when a frame is due (an auto-reset timer: a satisfied wait resets it).
    pub fn handle(&self) -> HANDLE {
        self.timer
    }

    /// While `moving`, makes sure a tick is pending (one frame from now, unless one already
    /// is: extra loop passes never push it back); otherwise disarms the timer.
    pub fn schedule(&mut self, moving: bool) {
        let now = Instant::now();
        // SAFETY: our own timer; the due time is a local.
        unsafe {
            if moving {
                if self.due.is_some_and(|due| due > now) {
                    return;
                }
                // Negative: relative, in 100 ns units.
                let relative = -(self.frame.as_nanos() as i64 / 100);
                let armed = SetWaitableTimerEx(self.timer, &relative, 0, None, None, None, 0);
                self.due = armed.is_ok().then_some(now + self.frame);
            } else if self.due.take().is_some() {
                let _ = CancelWaitableTimer(self.timer);
            }
        }
    }

    /// Is a tick pending (or just fired and not yet followed by `schedule`)?
    pub fn armed(&self) -> bool {
        self.due.is_some()
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        // SAFETY: our own handle, closed once.
        let _ = unsafe { CloseHandle(self.timer) };
    }
}

/// One refresh of the display, from DWM (60 Hz when it can't say).
pub fn display_frame() -> Duration {
    let mut timing = DWM_TIMING_INFO {
        cbSize: size_of::<DWM_TIMING_INFO>() as u32,
        ..Default::default()
    };
    // SAFETY: the out struct is a valid local with its size set.
    let ok = unsafe { DwmGetCompositionTimingInfo(HWND::default(), &mut timing) }.is_ok();
    let rate = timing.rateRefresh;
    if ok && rate.uiNumerator > 0 && rate.uiDenominator > 0 {
        Duration::from_secs_f64(f64::from(rate.uiDenominator) / f64::from(rate.uiNumerator))
    } else {
        Duration::from_secs_f64(1.0 / 60.0)
    }
}

