use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

pub use usrvclock::AsyncClient as ClockReceiver;
pub use usrvclock::ClockOverlay;
use usrvclock::SafeClock;

use crate::common::*;
use crate::util::real_time_box_channel::{self, RealTimeBoxReceiver};
pub type RealTimeClockReceiver = RealTimeBoxReceiver<Option<ClockOverlay>>;

/// High-precision clock (nanoseconds)
pub type FineClock = u64;

/// Signed version of the high-precision clock. For clock deltas.
pub type FineClockDiff = i64;

//#[derive(Clone)]
pub struct MediaClock {
  overlay: Option<ClockOverlay>,
}

#[inline(always)]
fn timestamp_to_clock_value(ts: clock_steering::Timestamp) -> FineClock {
  (ts.seconds as FineClock).wrapping_mul(1_000_000_000).wrapping_add(ts.nanos as FineClock)
}

impl MediaClock {
  pub fn new(use_safe_clock: bool) -> Self {
    assert!(!use_safe_clock);
    Self { overlay: None }
  }
  pub fn is_ready(&self) -> bool {
    self.overlay.is_some()
  }
  pub fn get_overlay(&self) -> &Option<ClockOverlay> {
    &self.overlay
  }
  pub fn update_overlay(&mut self, overlay: ClockOverlay) {
    /* if let Some(cur_overlay) = self.overlay {
      let cur_ovl_time = cur_overlay.now_ns();
      let new_ovl_time = overlay.now_ns();
      let diff = (new_ovl_time as ClockDiff).wrapping_sub(cur_ovl_time as ClockDiff);
      /* if diff.abs() > 10_000_000 {
        error!("clock is trying to jump dangerously by {diff} ns, ignoring update");
        return;
      } */
    } */
    self.overlay = Some(overlay);
  }
  #[inline(always)]
  pub fn now_ns(&self) -> Option<FineClock> {
    self.overlay.map(|overlay| overlay.now_ns() as FineClock)
  }
  #[inline(always)]
  pub fn now_in_timebase(&self, timebase_hz: u64) -> Option<LongClock> {
    self.now_ns().map(|ns| {
      // TODO it will jump when underlying wraps
      ((ns as u128) * (timebase_hz as u128) / 1_000_000_000u128) as LongClock
    })
  }
  #[inline(always)]
  pub fn wrapping_now_in_timebase(&self, timebase_hz: u64) -> Option<Clock> {
    self.now_in_timebase(timebase_hz).map(|x| x as Clock)
  }
  pub fn system_clock_duration_until(
    &mut self,
    timestamp: LongClock,
    timebase_hz: u64,
  ) -> Option<std::time::Duration> {
    let now_ns = self.now_ns()?;
    let until_ns = (timestamp as u128 * 1_000_000_000u128 / timebase_hz as u128) as FineClock;
    let remaining = (until_ns as FineClockDiff).wrapping_sub(now_ns as FineClockDiff);
    let corr = (remaining as f64 * self.overlay?.freq_scale) as FineClockDiff;
    let duration = remaining - corr; // FIXME it should be * 1/(1+freq_scale) but should be good enough for low correction values
    if duration > 0 {
      Some(std::time::Duration::from_nanos(duration as u64))
    } else {
      Some(std::time::Duration::from_secs(0))
    }
  }
  pub fn system_clock_duration_from_until(
    &mut self,
    from: Clock,
    until: Clock,
    timebase_hz: u64,
  ) -> Option<std::time::Duration> {
    let duration_in_tb = wrapped_diff(until, from);
    let duration_ns = (duration_in_tb as i64 * 1_000_000_000i64 / timebase_hz as i64) as FineClockDiff;
    let corr = (duration_ns as f64 * self.overlay?.freq_scale) as FineClockDiff;
    let duration = duration_ns - corr; // FIXME it should be * 1/(1+freq_scale) but should be good enough for low correction values
    if duration > 0 {
      Some(std::time::Duration::from_nanos(duration as u64))
    } else {
      Some(std::time::Duration::from_secs(0))
    }
  }
}

pub fn start_clock_receiver(path: Option<PathBuf>) -> ClockReceiver {
  let socket_path = path.unwrap_or(usrvclock::DEFAULT_SERVER_SOCKET_PATH.into());
  info!(clock_socket = ?socket_path, "connecting to clock receiver");
  ClockReceiver::start(
    socket_path,
    Box::new(|e| warn!("clock receive error: {e:?}")),
  )
}

pub async fn make_shared_media_clock(
  receiver: &ClockReceiver,
  use_safe_clock: bool,
) -> Arc<RwLock<MediaClock>> {
  let mut rx = receiver.subscribe();
  let media_clock = MediaClock::new(use_safe_clock);
  /* loop {
    match rx.recv().await {
      Ok(overlay) => {
        media_clock.update_overlay(overlay);
        break;
      }
      Err(broadcast::error::RecvError::Closed) => {
        panic!("ClockReceiver channel closed during initial await");
      },
      Err(e) => {
        warn!("clock receive error {e:?}");
      }
    }
  } */
  // initial await makes e.g. Audacity freeze when starting when Statime is not running. TODO figure it out
  let media_clock = Arc::new(RwLock::new(media_clock));
  let media_clock1 = media_clock.clone();
  // Explicit detach: spawned task continues updating clock overlay independently
  tokio::spawn(async move {
    loop {
      let overlay_opt = rx.borrow_and_update().clone();
      if let Some(overlay) = overlay_opt {
        match media_clock.write() {
          Ok(mut mc) => mc.update_overlay(overlay),
          Err(e) => error!("failed to acquire write lock on media_clock: {}", e),
        }
      }
      if rx.changed().await.is_err() {
        debug!("clock update task: channel closed, exiting");
        break;
      }
    }
  });
  media_clock1
}

pub fn async_clock_receiver_to_realtime(
  mut receiver: tokio::sync::watch::Receiver<Option<ClockOverlay>>,
  initial: Option<ClockOverlay>,
) -> RealTimeBoxReceiver<Option<ClockOverlay>> {
  let (rt_sender, rt_recv) = real_time_box_channel::channel(Box::new(initial));
  // Explicit detach: spawned task bridges async clock receiver to real-time channel
  tokio::spawn(async move {
    loop {
      let overlay_opt = receiver.borrow_and_update().clone();
      if let Some(overlay) = overlay_opt {
        rt_sender.send(Box::new(Some(overlay)));
      }
      if receiver.changed().await.is_err() {
        debug!("realtime clock adapter task: channel closed, exiting");
        break;
      }
    }
  });
  rt_recv
}
