//! Request lanes and pacing (SPEC §4.6). No clock, no WebKit: every method
//! takes the current time as a [`Duration`] since a fixed origin, so the tests
//! below assert each value in the spec with made-up time.

use std::collections::VecDeque;
use std::time::Duration;

/// Minimum gap between requests.
pub const MIN_GAP: Duration = Duration::from_millis(1000);
/// Random jitter added to each gap is uniform in `0..=MAX_JITTER`.
pub const MAX_JITTER: Duration = Duration::from_millis(500);
/// Gap inside a burst after a user action.
pub const BURST_GAP: Duration = Duration::from_millis(250);
/// First back-off after pushback.
pub const BACKOFF_START: Duration = Duration::from_secs(5 * 60);
/// Back-off doubles up to this.
pub const BACKOFF_CAP: Duration = Duration::from_secs(60 * 60);
/// A quiet time this long since the last pushback starts the back-off over.
pub const BACKOFF_RESET: Duration = Duration::from_secs(60 * 60);

/// Which queue a request waits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// Caused by a user action.
    Interactive,
    /// Inbox checks and the self-test: wait while anything interactive is queued.
    Background,
}

/// What on screen a request is for. The list pane and the post pane are both
/// visible in the wide layout, so leaving a view means replacing it in its own
/// pane; a new feed also closes the post pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    /// Not tied to a view; never dropped.
    None,
    /// A feed in the list pane.
    Feed(String),
    /// A post in the post pane.
    Thread(String),
}

/// What to do next.
#[derive(Debug, PartialEq, Eq)]
pub enum Next<T> {
    /// Send this request now. Call [`Scheduler::finished`] when it ends.
    Send(T),
    /// Ask again after this long.
    Wait(Duration),
    /// Nothing can happen until a request is queued or finishes.
    Idle,
}

struct Queued<T> {
    view: View,
    item: T,
}

/// The queues and the timing rules. `T` is whatever wakes the waiting request.
pub struct Scheduler<T> {
    interactive: VecDeque<Queued<T>>,
    background: VecDeque<Queued<T>>,
    in_flight: bool,
    /// When the last request finished, and the gap drawn for the next one.
    last: Option<(Duration, Duration)>,
    feed: Option<String>,
    thread: Option<String>,
    /// The next interactive send starts a burst.
    burst_pending: bool,
    /// The send after the current one may use [`BURST_GAP`].
    burst_second: bool,
    blocked_until: Option<Duration>,
    last_backoff: Option<(Duration, Duration)>,
    jitter: Box<dyn FnMut() -> Duration>,
}

impl<T> Scheduler<T> {
    /// A scheduler drawing each gap's jitter from `jitter`.
    pub fn new(jitter: impl FnMut() -> Duration + 'static) -> Self {
        Self {
            interactive: VecDeque::new(),
            background: VecDeque::new(),
            in_flight: false,
            last: None,
            feed: None,
            thread: None,
            burst_pending: false,
            burst_second: false,
            blocked_until: None,
            last_backoff: None,
            jitter: Box::new(jitter),
        }
    }

    /// Queues a request. Returns the queued requests this one replaces, which
    /// are never sent (SPEC §4.6): the caller fails them with `Cancelled`.
    pub fn enqueue(&mut self, lane: Lane, view: View, item: T) -> Vec<T> {
        let mut dropped = Vec::new();
        if lane == Lane::Interactive {
            match &view {
                View::None => {}
                View::Feed(id) => {
                    if self.feed.as_ref() != Some(id) {
                        self.burst_pending = true;
                    }
                    self.feed = Some(id.clone());
                    self.thread = None;
                    dropped = self.remove_interactive(|v| match v {
                        View::None => false,
                        View::Feed(other) => other != id,
                        View::Thread(_) => true,
                    });
                }
                View::Thread(id) => {
                    if self.thread.as_ref() != Some(id) {
                        self.burst_pending = true;
                    }
                    self.thread = Some(id.clone());
                    dropped = self.remove_interactive(|v| matches!(v, View::Thread(other) if other != id));
                }
            }
        }
        let queue = match lane {
            Lane::Interactive => &mut self.interactive,
            Lane::Background => &mut self.background,
        };
        queue.push_back(Queued { view, item });
        dropped
    }

    /// Queues a later step of an operation already running for `view`. If the
    /// user has left that view, `item` comes back and nothing is queued.
    pub fn enqueue_step(&mut self, lane: Lane, view: View, item: T) -> Result<(), T> {
        let current = match &view {
            View::None => true,
            View::Feed(id) => self.feed.as_ref() == Some(id),
            View::Thread(id) => self.thread.as_ref() == Some(id),
        };
        if !current && lane == Lane::Interactive {
            return Err(item);
        }
        let queue = match lane {
            Lane::Interactive => &mut self.interactive,
            Lane::Background => &mut self.background,
        };
        queue.push_back(Queued { view, item });
        Ok(())
    }

    fn remove_interactive(&mut self, drop_view: impl Fn(&View) -> bool) -> Vec<T> {
        let mut kept = VecDeque::new();
        let mut dropped = Vec::new();
        for q in self.interactive.drain(..) {
            if drop_view(&q.view) {
                dropped.push(q.item);
            } else {
                kept.push_back(q);
            }
        }
        self.interactive = kept;
        dropped
    }

    /// The next step at time `now`. One request is in flight at a time.
    pub fn next(&mut self, now: Duration) -> Next<T> {
        if self.in_flight {
            return Next::Idle;
        }
        let interactive = !self.interactive.is_empty();
        if !interactive && self.background.is_empty() {
            return Next::Idle;
        }
        if let Some(until) = self.blocked_until {
            if now < until {
                return Next::Wait(until - now);
            }
            self.blocked_until = None;
        }
        if let Some((finished, gap)) = self.last {
            let gap = if self.burst_second && interactive {
                BURST_GAP
            } else {
                gap
            };
            let earliest = finished + gap;
            if now < earliest {
                return Next::Wait(earliest - now);
            }
        }
        let queued = if interactive {
            self.interactive.pop_front()
        } else {
            self.background.pop_front()
        };
        let queued = queued.expect("checked above");
        self.burst_second = interactive && self.burst_pending;
        if interactive {
            self.burst_pending = false;
        }
        self.in_flight = true;
        Next::Send(queued.item)
    }

    /// The request sent last ended at `now`.
    pub fn finished(&mut self, now: Duration) {
        self.in_flight = false;
        let jitter = (self.jitter)().min(MAX_JITTER);
        self.last = Some((now, MIN_GAP + jitter));
    }

    /// Reddit pushed back at `now`: stops all traffic and returns how long for,
    /// with every queued request, which the caller fails. Doubles on each
    /// pushback until a quiet [`BACKOFF_RESET`] has passed.
    pub fn pushback(&mut self, now: Duration) -> (Duration, Vec<T>) {
        let backoff = match self.last_backoff {
            Some((at, previous)) if now.saturating_sub(at) < previous + BACKOFF_RESET => {
                (previous * 2).min(BACKOFF_CAP)
            }
            _ => BACKOFF_START,
        };
        self.last_backoff = Some((now, backoff));
        self.blocked_until = Some(now + backoff);
        let dropped = self
            .interactive
            .drain(..)
            .chain(self.background.drain(..))
            .map(|q| q.item)
            .collect();
        (backoff, dropped)
    }

    /// Restores a back-off saved before the app last quit: `remaining` left of
    /// a back-off of length `backoff`.
    pub fn restore_backoff(&mut self, now: Duration, remaining: Duration, backoff: Duration) {
        self.last_backoff = Some((now, backoff));
        self.blocked_until = Some(now + remaining);
    }

    /// How long traffic is still stopped.
    pub fn blocked_for(&self, now: Duration) -> Option<Duration> {
        self.blocked_until.filter(|until| now < *until).map(|until| until - now)
    }

    /// The length of the back-off now running, for saving it.
    pub fn current_backoff(&self) -> Option<Duration> {
        self.last_backoff.map(|(_, b)| b)
    }

    /// Forgets the views, for sign-out: nothing queued belongs to anyone.
    pub fn drain(&mut self) -> Vec<T> {
        self.feed = None;
        self.thread = None;
        self.interactive
            .drain(..)
            .chain(self.background.drain(..))
            .map(|q| q.item)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn scheduler() -> Scheduler<&'static str> {
        Scheduler::new(|| Duration::ZERO)
    }

    fn feed(id: &str) -> View {
        View::Feed(id.into())
    }

    fn thread(id: &str) -> View {
        View::Thread(id.into())
    }

    /// Sends the next request at `now` and finishes it immediately.
    fn send(s: &mut Scheduler<&'static str>, now: Duration) -> &'static str {
        match s.next(now) {
            Next::Send(item) => {
                s.finished(now);
                item
            }
            other => panic!("expected a send at {now:?}, got {other:?}"),
        }
    }

    #[test]
    fn constants_match_the_spec() {
        assert_eq!(MIN_GAP, ms(1000));
        assert_eq!(MAX_JITTER, ms(500));
        assert_eq!(BURST_GAP, ms(250));
        assert_eq!(BACKOFF_START, Duration::from_secs(300));
        assert_eq!(BACKOFF_CAP, Duration::from_secs(3600));
    }

    #[test]
    fn gap_is_at_least_one_second_after_the_last_request_ended() {
        let mut s = scheduler();
        s.enqueue(Lane::Background, View::None, "a");
        s.enqueue(Lane::Background, View::None, "b");
        assert_eq!(send(&mut s, ms(0)), "a");
        assert_eq!(s.next(ms(999)), Next::Wait(ms(1)));
        assert_eq!(send(&mut s, ms(1000)), "b");
    }

    #[test]
    fn jitter_is_added_and_capped_at_half_a_second() {
        let mut s: Scheduler<&str> = Scheduler::new(|| ms(9000));
        s.enqueue(Lane::Background, View::None, "a");
        s.enqueue(Lane::Background, View::None, "b");
        assert_eq!(send(&mut s, ms(0)), "a");
        assert_eq!(s.next(ms(1000)), Next::Wait(ms(500)));
        assert_eq!(send(&mut s, ms(1500)), "b");

        let mut s: Scheduler<&str> = Scheduler::new(|| ms(123));
        s.enqueue(Lane::Background, View::None, "a");
        s.enqueue(Lane::Background, View::None, "b");
        assert_eq!(send(&mut s, ms(0)), "a");
        assert_eq!(s.next(ms(1000)), Next::Wait(ms(123)));
    }

    #[test]
    fn one_request_is_in_flight_across_both_lanes() {
        let mut s = scheduler();
        s.enqueue(Lane::Background, View::None, "bg");
        s.enqueue(Lane::Interactive, View::None, "ui");
        assert_eq!(s.next(ms(0)), Next::Send("ui"));
        assert_eq!(s.next(ms(60_000)), Next::Idle);
        s.finished(ms(60_000));
        assert_eq!(s.next(ms(61_000)), Next::Send("bg"));
    }

    #[test]
    fn background_waits_while_anything_interactive_is_queued() {
        let mut s = scheduler();
        s.enqueue(Lane::Background, View::None, "bg");
        s.enqueue(Lane::Interactive, View::None, "ui1");
        s.enqueue(Lane::Interactive, View::None, "ui2");
        assert_eq!(send(&mut s, ms(0)), "ui1");
        assert_eq!(send(&mut s, ms(1000)), "ui2");
        assert_eq!(send(&mut s, ms(2000)), "bg");
    }

    #[test]
    fn leaving_a_view_drops_its_queued_requests() {
        let mut s = scheduler();
        s.enqueue(Lane::Interactive, thread("a"), "a1");
        s.enqueue(Lane::Interactive, thread("a"), "a2");
        assert_eq!(s.enqueue(Lane::Interactive, thread("b"), "b"), vec!["a1", "a2"]);
        assert_eq!(send(&mut s, ms(0)), "b");
        assert_eq!(s.next(ms(100_000)), Next::Idle);
    }

    #[test]
    fn opening_a_post_keeps_the_feed_requests_next_to_it() {
        let mut s = scheduler();
        s.enqueue(Lane::Interactive, feed("gnome"), "info");
        assert!(s.enqueue(Lane::Interactive, thread("a"), "post").is_empty());
        assert_eq!(send(&mut s, ms(0)), "info");
        assert_eq!(send(&mut s, ms(1000)), "post");
    }

    #[test]
    fn a_new_feed_drops_the_old_feed_and_the_post() {
        let mut s = scheduler();
        s.enqueue(Lane::Interactive, feed("gnome"), "page");
        s.enqueue(Lane::Interactive, thread("a"), "post");
        s.enqueue(Lane::Interactive, View::None, "whoami");
        let dropped = s.enqueue(Lane::Interactive, feed("rust"), "new");
        assert_eq!(dropped, vec!["page", "post"]);
        assert_eq!(send(&mut s, ms(0)), "whoami");
        assert_eq!(send(&mut s, ms(1000)), "new");
    }

    #[test]
    fn background_is_never_dropped_by_a_view_change() {
        let mut s = scheduler();
        s.enqueue(Lane::Background, feed("gnome"), "bg");
        assert!(s.enqueue(Lane::Interactive, feed("rust"), "ui").is_empty());
        assert_eq!(send(&mut s, ms(0)), "ui");
        assert_eq!(send(&mut s, ms(1000)), "bg");
    }

    #[test]
    fn a_later_step_for_a_view_the_user_left_is_refused() {
        let mut s = scheduler();
        s.enqueue(Lane::Interactive, thread("a"), "a1");
        assert_eq!(send(&mut s, ms(0)), "a1");
        s.enqueue(Lane::Interactive, thread("b"), "b1");
        assert_eq!(s.enqueue_step(Lane::Interactive, thread("a"), "a2"), Err("a2"));
        assert_eq!(s.enqueue_step(Lane::Interactive, thread("b"), "b2"), Ok(()));
    }

    #[test]
    fn a_new_feed_closes_the_post_for_later_steps() {
        let mut s = scheduler();
        s.enqueue(Lane::Interactive, thread("a"), "a1");
        s.enqueue(Lane::Interactive, feed("rust"), "page");
        assert_eq!(s.enqueue_step(Lane::Interactive, thread("a"), "a2"), Err("a2"));
    }

    #[test]
    fn burst_after_a_user_action_is_two_requests_250_ms_apart() {
        let mut s = scheduler();
        s.enqueue(Lane::Interactive, thread("a"), "post");
        s.enqueue_step(Lane::Interactive, thread("a"), "comments").unwrap();
        s.enqueue_step(Lane::Interactive, thread("a"), "third").unwrap();
        assert_eq!(send(&mut s, ms(0)), "post");
        assert_eq!(s.next(ms(249)), Next::Wait(ms(1)));
        assert_eq!(send(&mut s, ms(250)), "comments");
        assert_eq!(s.next(ms(1249)), Next::Wait(ms(1)));
        assert_eq!(send(&mut s, ms(1250)), "third");
    }

    #[test]
    fn staying_in_a_view_gives_no_new_burst() {
        let mut s = scheduler();
        s.enqueue(Lane::Interactive, feed("gnome"), "page1");
        assert_eq!(send(&mut s, ms(0)), "page1");
        s.enqueue(Lane::Interactive, feed("gnome"), "info");
        assert_eq!(send(&mut s, ms(250)), "info");
        s.enqueue(Lane::Interactive, feed("gnome"), "page2");
        s.enqueue(Lane::Interactive, feed("gnome"), "page3");
        assert_eq!(s.next(ms(1000)), Next::Wait(ms(250)));
        assert_eq!(send(&mut s, ms(1250)), "page2");
        assert_eq!(s.next(ms(1500)), Next::Wait(ms(750)));
    }

    #[test]
    fn background_never_gets_the_burst_gap() {
        let mut s = scheduler();
        s.enqueue(Lane::Interactive, thread("a"), "post");
        s.enqueue(Lane::Background, View::None, "bg");
        assert_eq!(send(&mut s, ms(0)), "post");
        assert_eq!(s.next(ms(250)), Next::Wait(ms(750)));
    }

    #[test]
    fn pushback_stops_everything_and_doubles_up_to_the_cap() {
        let mut s = scheduler();
        s.enqueue(Lane::Interactive, View::None, "a");
        s.enqueue(Lane::Background, View::None, "b");
        let mut now = ms(0);
        let (first, dropped) = s.pushback(now);
        assert_eq!(first, Duration::from_secs(300));
        assert_eq!(dropped, vec!["a", "b"]);

        s.enqueue(Lane::Interactive, View::None, "c");
        assert_eq!(s.next(now), Next::Wait(first));
        assert_eq!(s.blocked_for(now), Some(first));
        now += first;
        assert_eq!(s.blocked_for(now), None);
        assert_eq!(send(&mut s, now), "c");

        let expected = [600, 1200, 2400, 3600, 3600];
        for secs in expected {
            let (backoff, _) = s.pushback(now);
            assert_eq!(backoff, Duration::from_secs(secs));
            now += backoff;
        }
    }

    #[test]
    fn a_quiet_hour_starts_the_backoff_over() {
        let mut s = scheduler();
        let (first, _) = s.pushback(ms(0));
        let after = first + BACKOFF_RESET;
        let (again, _) = s.pushback(after);
        assert_eq!(again, BACKOFF_START);
    }

    #[test]
    fn a_saved_backoff_still_holds_after_restart() {
        let mut s = scheduler();
        s.restore_backoff(ms(0), Duration::from_secs(120), Duration::from_secs(300));
        s.enqueue(Lane::Interactive, View::None, "a");
        assert_eq!(s.next(ms(0)), Next::Wait(Duration::from_secs(120)));
        let (next, _) = s.pushback(Duration::from_secs(120));
        assert_eq!(next, Duration::from_secs(600));
    }
}
