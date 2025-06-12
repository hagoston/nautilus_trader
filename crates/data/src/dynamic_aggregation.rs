use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use nautilus_core::{UnixNanos, datetime::NANOS_IN_SECOND};
use nautilus_common::clock::Clock;

use nautilus_model::data::{
    TradeTick, Bar, BarType, BarSpecification, PriceType, MintSpecificBar,
};
use nautilus_model::identifiers::InstrumentId;
use nautilus_model::enums::{AggregationSource, BarAggregation, BarIntervalType};

// Assuming these are in super::aggregation, which is crate::aggregation from this file's perspective
use super::aggregation::{
    BarAggregator, TickBarAggregator, TimeBarAggregator, VolumeBarAggregator, ValueBarAggregator,
    // NewBarCallback is part of TimeBarAggregator::start, which is complex for dynamic dispatch here
};

/// Manages a collection of bar aggregators, dynamically creating them based on
/// observed `mint_address` values in incoming `TradeTick`s.
///
/// Each unique `mint_address` (or `None`) will have its own dedicated bar aggregator,
/// configured according to the `base_bar_spec` derived from the initial `base_bar_type`.
/// Aggregators are removed if they do not receive any trades within the `inactivity_timeout_ns`.
pub struct DynamicAggregatorManager {
    instrument_id: InstrumentId,
    base_bar_spec: BarSpecification,
    inactivity_timeout_ns: u64,
    // Using Option<[u8; 32]> as key for HashMap to represent mint address (Some) or general (None)
    aggregators: HashMap<Option<[u8; 32]>, Box<dyn BarAggregator>>,
    last_activity: HashMap<Option<[u8; 32]>, UnixNanos>,
    // Using Rc<RefCell<Box<...>>> to allow shared mutable access to the handler
    bar_handler: Rc<RefCell<Box<dyn FnMut(MintSpecificBar) + Send>>>,
    clock: Rc<RefCell<dyn Clock>>,
    price_precision: u8,
    size_precision: u8,

    // Default settings for TimeBarAggregators
    time_bar_build_with_no_updates: bool,
    time_bar_timestamp_on_close: bool,
    time_bar_interval_type: BarIntervalType,
}

impl DynamicAggregatorManager {
    /// Creates a new `DynamicAggregatorManager`.
    ///
    /// # Parameters
    ///
    /// - `instrument_id`: The primary instrument ID these aggregators are for.
    /// - `base_bar_type`: The template `BarType` used to configure new aggregators.
    ///   The `instrument_id` from this `base_bar_type` will be overridden by the manager's `instrument_id`.
    ///   Its `AggregationSource` will be overridden to `Internal`. The `BarSpecification` from this
    ///   type determines the kind of sub-aggregators created (e.g., Tick, Volume, Time).
    /// - `inactivity_timeout_ns`: Nanoseconds of inactivity after which an aggregator for a specific mint is removed.
    /// - `bar_handler`: A shared, mutable callback that is invoked with `MintSpecificBar` when any aggregator produces a bar.
    ///   It's wrapped in `Rc<RefCell<Box<...>>>` to allow shared mutable access from multiple sub-aggregators.
    /// - `clock`: A shared clock source.
    /// - `price_precision`: Price precision for the instrument, used when creating new aggregators.
    /// - `size_precision`: Size precision for the instrument, used when creating new aggregators.
    /// - `time_bar_build_with_no_updates`: Optional default for `TimeBarAggregator`s: if `Some(true)`, generate empty bars. Defaults to `true`.
    /// - `time_bar_timestamp_on_close`: Optional default for `TimeBarAggregator`s: if `Some(true)`, bar timestamp is at close. Defaults to `true`.
    /// - `time_bar_interval_type`: Optional default for `TimeBarAggregator`s: e.g., `BarIntervalType::LeftOpen`. Defaults to `LeftOpen`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instrument_id: InstrumentId,
        base_bar_type: BarType,
        inactivity_timeout_ns: u64,
        bar_handler: Rc<RefCell<Box<dyn FnMut(MintSpecificBar) + Send>>>,
        clock: Rc<RefCell<dyn Clock>>,
        price_precision: u8,
        size_precision: u8,
        time_bar_build_with_no_updates: Option<bool>,
        time_bar_timestamp_on_close: Option<bool>,
        time_bar_interval_type: Option<BarIntervalType>,
    ) -> Self {
        let base_bar_spec = base_bar_type.spec(); // Extract spec for reuse

        Self {
            instrument_id,
            base_bar_spec,
            inactivity_timeout_ns,
            aggregators: HashMap::new(),
            last_activity: HashMap::new(),
            bar_handler,
            clock,
            price_precision,
            size_precision,
            time_bar_build_with_no_updates: time_bar_build_with_no_updates.unwrap_or(true),
            time_bar_timestamp_on_close: time_bar_timestamp_on_close.unwrap_or(true),
            time_bar_interval_type: time_bar_interval_type.unwrap_or(BarIntervalType::LeftOpen),
        }
    }

    /// Processes an incoming `TradeTick`.
    ///
    /// If an aggregator for the trade's `mint_address` (or for `None` if the trade has no mint address)
    /// does not exist, a new one is created based on `base_bar_spec`. The trade is then forwarded
    /// to the appropriate aggregator. The last activity time for this mint address is updated.
    pub fn handle_trade_tick(&mut self, trade: TradeTick) {
        let current_mint_address_key = trade.mint_address;
        let now = self.clock.borrow().now();

        self.last_activity.insert(current_mint_address_key, now);

        if !self.aggregators.contains_key(&current_mint_address_key) {
            let new_agg_bar_type = BarType::new(
                self.instrument_id,
                self.base_bar_spec,
                AggregationSource::Internal,
            );

            let main_bar_handler_rc = self.bar_handler.clone();
            let mint_for_closure = current_mint_address_key;

            let sub_handler = Box::new(move |bar: Bar| {
                let mint_specific_bar = MintSpecificBar::new(bar, mint_for_closure);
                main_bar_handler_rc.borrow_mut()(mint_specific_bar);
            });

            let new_aggregator_result: Result<Box<dyn BarAggregator>, String> = match self.base_bar_spec.aggregation {
                BarAggregation::Tick => Ok(Box::new(TickBarAggregator::new(
                    new_agg_bar_type, self.price_precision, self.size_precision,
                    sub_handler, false, current_mint_address_key,
                ))),
                BarAggregation::Volume => Ok(Box::new(VolumeBarAggregator::new(
                    new_agg_bar_type, self.price_precision, self.size_precision,
                    sub_handler, false, current_mint_address_key,
                ))),
                BarAggregation::Value => Ok(Box::new(ValueBarAggregator::new(
                    new_agg_bar_type, self.price_precision, self.size_precision,
                    sub_handler, false, current_mint_address_key,
                ))),
                BarAggregation::Millisecond | BarAggregation::Second | BarAggregation::Minute |
                BarAggregation::Hour | BarAggregation::Day | BarAggregation::Week | BarAggregation::Month => {
                    let time_agg_clock = self.clock.clone();
                    let mut time_agg = TimeBarAggregator::new(
                        new_agg_bar_type, self.price_precision, self.size_precision,
                        time_agg_clock, sub_handler, false,
                        self.time_bar_build_with_no_updates,
                        self.time_bar_timestamp_on_close,
                        self.time_bar_interval_type,
                        None, 15, false, current_mint_address_key,
                    );
                    // NOTE: TimeBarAggregator::start() is not called here. Timer-based bar emission for these
                    // dynamically created TimeBarAggregators will not occur without calling their `start` method
                    // with a NewBarCallback<H> that wraps an Rc<RefCell<TimeBarAggregator<H>>>.
                    // This is a limitation due to the `Box<dyn BarAggregator>` abstraction.
                    // Direct calls to `handle_trade` on this boxed aggregator will still process ticks
                    // and can complete bars if the bar type is threshold-based within the time window,
                    // or if `stop()` is called.
                    Ok(Box::new(time_agg))
                },
                unsupported_agg => {
                    Err(format!("Unsupported base_bar_spec aggregation type for dynamic mint-specific aggregator: {:?}", unsupported_agg))
                }
            };

            match new_aggregator_result {
                Ok(new_aggregator) => {
                    self.aggregators.insert(current_mint_address_key, new_aggregator);
                }
                Err(e) => {
                    eprintln!("Failed to create aggregator: {}", e);
                    self.last_activity.remove(&current_mint_address_key);
                    return;
                }
            }
        }

        if let Some(aggregator) = self.aggregators.get_mut(&current_mint_address_key) {
            aggregator.handle_trade(trade.clone());
        }
    }

    /// Checks for and removes aggregators that have been inactive for longer than `inactivity_timeout_ns`.
    ///
    /// This method should be called periodically (e.g., by an external timer or a managing system component).
    pub fn check_timeouts(&mut self) {
        let current_time_ns = self.clock.borrow().now().nanos();
        let mut timed_out_mints: Vec<Option<[u8; 32]>> = Vec::new();

        for (mint_addr_option, last_event_time_ns) in self.last_activity.iter() {
            if current_time_ns.saturating_sub(last_event_time_ns.nanos()) > self.inactivity_timeout_ns {
                timed_out_mints.push(*mint_addr_option);
            }
        }

        for mint_addr_option in timed_out_mints {
            if let Some(mut aggregator) = self.aggregators.remove(&mint_addr_option) {
                aggregator.stop();
                // Consider proper logging if a logger is available
                // println!("Aggregator for mint {:?} timed out and was removed.", mint_addr_option);
            }
            self.last_activity.remove(&mint_addr_option);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nautilus_model::data::stubs::stub_instrument;
    use nautilus_model::enums::{BarAggregation, PriceType, AggressorSide};
    use nautilus_model::identifiers::TradeId;
    use nautilus_model::objects::{Price, Quantity};
    use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
    use std::time::Duration; // For advancing clock

    fn make_test_bar_spec(step: usize, agg: BarAggregation) -> BarSpecification {
        BarSpecification::new(step, agg, PriceType::Last)
    }

    fn make_test_bar_type(instrument_id: InstrumentId, step: usize, agg: BarAggregation) -> BarType {
        BarType::new(instrument_id, make_test_bar_spec(step, agg), AggregationSource::Internal)
    }

    fn make_dummy_mint_address(val: u8) -> Option<[u8; 32]> {
        let mut addr = [0u8; 32];
        addr[0] = val;
        Some(addr)
    }

    struct TestContext {
        instrument: nautilus_model::instruments::InstrumentAny, // Use concrete type from stubs
        clock: Rc<RefCell<TestClock>>,
        bar_handler_rx: Receiver<MintSpecificBar>,
        manager: DynamicAggregatorManager,
    }

    fn setup_manager(
        base_bar_spec_agg: BarAggregation,
        base_bar_spec_step: usize,
        inactivity_timeout_s: u64
    ) -> TestContext {
        let instrument = stub_instrument(); // Provides precisions
        let base_bar_type = make_test_bar_type(instrument.id, base_bar_spec_step, base_bar_spec_agg);
        let clock = Rc::new(RefCell::new(TestClock::new()));

        let (tx, rx): (Sender<MintSpecificBar>, Receiver<MintSpecificBar>) = mpsc::channel();
        let bar_handler_rc: Rc<RefCell<Box<dyn FnMut(MintSpecificBar) + Send>>> =
            Rc::new(RefCell::new(Box::new(move |bar| {
                tx.send(bar).unwrap_or_else(|e| eprintln!("Test Error: Failed to send bar: {:?}", e));
            })));

        let manager = DynamicAggregatorManager::new(
            instrument.id,
            base_bar_type,
            inactivity_timeout_s * NANOS_IN_SECOND,
            bar_handler_rc,
            clock.clone(),
            instrument.price_precision,
            instrument.size_precision,
            Some(true), // time_bar_build_with_no_updates
            Some(true), // time_bar_timestamp_on_close
            Some(BarIntervalType::LeftOpen) // time_bar_interval_type
        );
        TestContext { instrument, clock, bar_handler_rx: rx, manager }
    }

    #[test]
    fn test_dynamic_aggregator_manager_new() {
        let ctx = setup_manager(BarAggregation::Tick, 1, 10);
        assert_eq!(ctx.manager.instrument_id, ctx.instrument.id);
        assert_eq!(ctx.manager.base_bar_spec, make_test_bar_spec(1, BarAggregation::Tick));
        assert_eq!(ctx.manager.inactivity_timeout_ns, 10 * NANOS_IN_SECOND);
        assert!(ctx.manager.aggregators.is_empty());
        assert!(ctx.manager.last_activity.is_empty());
    }

    #[test]
    fn test_handle_trade_tick_creates_aggregators_and_produces_bars_for_tick_agg() {
        let mut ctx = setup_manager(BarAggregation::Tick, 2, 10); // 2 ticks for a bar

        let mint_addr1 = make_dummy_mint_address(1);
        let mint_addr2 = make_dummy_mint_address(2);
        let mut current_ts = ctx.clock.borrow().now();

        // Trades for mint_addr1
        let trade1_m1 = TradeTick::new(ctx.instrument.id, Price("100.0", ctx.instrument.price_precision).unwrap(), Quantity("10", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("t1").unwrap(), current_ts, current_ts, mint_addr1).unwrap();
        ctx.manager.handle_trade_tick(trade1_m1.clone());
        assert_eq!(ctx.manager.aggregators.len(), 1);
        current_ts = ctx.clock.borrow_mut().advance_time_ns(current_ts.nanos() + 1000);
        let trade2_m1 = TradeTick::new(ctx.instrument.id, Price("101.0", ctx.instrument.price_precision).unwrap(), Quantity("12", ctx.instrument.size_precision).unwrap(), AggressorSide::Seller, TradeId::from_str("t2").unwrap(), current_ts, current_ts, mint_addr1).unwrap();
        ctx.manager.handle_trade_tick(trade2_m1.clone()); // Bar for mint_addr1

        // Trades for mint_addr2
        current_ts = ctx.clock.borrow_mut().advance_time_ns(current_ts.nanos() + 1000);
        let trade1_m2 = TradeTick::new(ctx.instrument.id, Price("200.0", ctx.instrument.price_precision).unwrap(), Quantity("5", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("t3").unwrap(), current_ts, current_ts, mint_addr2).unwrap();
        ctx.manager.handle_trade_tick(trade1_m2.clone());
        assert_eq!(ctx.manager.aggregators.len(), 2);
        current_ts = ctx.clock.borrow_mut().advance_time_ns(current_ts.nanos() + 1000);
        let trade2_m2 = TradeTick::new(ctx.instrument.id, Price("201.0", ctx.instrument.price_precision).unwrap(), Quantity("7", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("t4").unwrap(), current_ts, current_ts, mint_addr2).unwrap();
        ctx.manager.handle_trade_tick(trade2_m2.clone()); // Bar for mint_addr2

        // Trades for None mint_address
        current_ts = ctx.clock.borrow_mut().advance_time_ns(current_ts.nanos() + 1000);
        let trade1_none = TradeTick::new(ctx.instrument.id, Price("300.0", ctx.instrument.price_precision).unwrap(), Quantity("2", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("t5").unwrap(), current_ts, current_ts, None).unwrap();
        ctx.manager.handle_trade_tick(trade1_none.clone());
        assert_eq!(ctx.manager.aggregators.len(), 3);
        current_ts = ctx.clock.borrow_mut().advance_time_ns(current_ts.nanos() + 1000);
        let trade2_none = TradeTick::new(ctx.instrument.id, Price("301.0", ctx.instrument.price_precision).unwrap(), Quantity("3", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("t6").unwrap(), current_ts, current_ts, None).unwrap();
        ctx.manager.handle_trade_tick(trade2_none.clone()); // Bar for None

        let mut bars_received = 0;
        for _ in 0..3 { // Expect 3 bars
            match ctx.bar_handler_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(mint_bar) => {
                    bars_received += 1;
                    if mint_bar.mint_address == mint_addr1 {
                        assert_eq!(mint_bar.bar.volume, Quantity("22", ctx.instrument.size_precision).unwrap());
                    } else if mint_bar.mint_address == mint_addr2 {
                        assert_eq!(mint_bar.bar.volume, Quantity("12", ctx.instrument.size_precision).unwrap());
                    } else if mint_bar.mint_address.is_none() {
                        assert_eq!(mint_bar.bar.volume, Quantity("5", ctx.instrument.size_precision).unwrap());
                    }
                },
                Err(e) => panic!("Failed to receive bar: {:?}", e),
            }
        }
        assert_eq!(bars_received, 3);
    }

    #[test]
    fn test_check_timeouts_removes_inactive_aggregators_and_flushes_partial_bar() {
        let mut ctx = setup_manager(BarAggregation::Tick, 2, 10); // 2 ticks for a bar, 10s timeout
        let mint_addr1 = make_dummy_mint_address(1);
        let mut current_ts = ctx.clock.borrow().now();

        // Feed one tick, creating a partial bar
        let trade1 = TradeTick::new(ctx.instrument.id, Price("100.0", ctx.instrument.price_precision).unwrap(), Quantity("10", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("t1").unwrap(), current_ts, current_ts, mint_addr1).unwrap();
        ctx.manager.handle_trade_tick(trade1);
        assert_eq!(ctx.manager.aggregators.len(), 1);

        // Advance clock beyond timeout
        ctx.clock.borrow_mut().advance_time_ns(current_ts.nanos() + (10 * NANOS_IN_SECOND) + 1);
        ctx.manager.check_timeouts();

        assert!(ctx.manager.aggregators.is_empty(), "Aggregator should be removed after timeout");
        assert!(ctx.manager.last_activity.is_empty(), "Last activity should be cleared");

        // Check if partial bar was flushed
        match ctx.bar_handler_rx.try_recv() {
            Ok(mint_bar) => {
                assert_eq!(mint_bar.mint_address, mint_addr1);
                assert_eq!(mint_bar.bar.open, Price("100.0", ctx.instrument.price_precision).unwrap());
                assert_eq!(mint_bar.bar.volume, Quantity("10", ctx.instrument.size_precision).unwrap());
            },
            Err(TryRecvError::Empty) => panic!("Expected a partial bar to be flushed on timeout, but channel was empty."),
            Err(e) => panic!("Error receiving flushed bar: {:?}", e),
        }
    }

    #[test]
    fn test_timeout_reset_by_activity() {
        let mut ctx = setup_manager(BarAggregation::Tick, 2, 10);
        let mint_addr1 = make_dummy_mint_address(1);
        let mut current_ts = ctx.clock.borrow().now();

        ctx.manager.handle_trade_tick(TradeTick::new(ctx.instrument.id, Price("100.0", ctx.instrument.price_precision).unwrap(), Quantity("10", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("t1").unwrap(), current_ts, current_ts, mint_addr1).unwrap());

        // Advance clock, but less than timeout
        current_ts = ctx.clock.borrow_mut().advance_time_ns(current_ts.nanos() + (5 * NANOS_IN_SECOND));
        ctx.manager.check_timeouts();
        assert_eq!(ctx.manager.aggregators.len(), 1, "Aggregator should not time out yet");

        // New activity for the same mint address
        ctx.manager.handle_trade_tick(TradeTick::new(ctx.instrument.id, Price("101.0", ctx.instrument.price_precision).unwrap(), Quantity("12", ctx.instrument.size_precision).unwrap(), AggressorSide::Seller, TradeId::from_str("t2").unwrap(), current_ts, current_ts, mint_addr1).unwrap());

        // Advance clock again, total time since first tick is > timeout, but not since second tick
        ctx.clock.borrow_mut().advance_time_ns(current_ts.nanos() + (6 * NANOS_IN_SECOND)); // Total 11s from start, 6s from last tick
        ctx.manager.check_timeouts();
        assert_eq!(ctx.manager.aggregators.len(), 1, "Aggregator should not time out due to reset activity");

        // Now let it time out
        ctx.clock.borrow_mut().advance_time_ns(ctx.clock.borrow().now().nanos() + (5 * NANOS_IN_SECOND)); // 6 + 5 = 11s since last activity
        ctx.manager.check_timeouts();
        assert!(ctx.manager.aggregators.is_empty(), "Aggregator should time out eventually");
    }

    #[test]
    fn test_volume_aggregator_dynamic_creation_and_bar() {
        let mut ctx = setup_manager(BarAggregation::Volume, 100, 10); // Volume step 100
        let mint_addr_vol = make_dummy_mint_address(10);
        let mut ts = ctx.clock.borrow().now();

        ctx.manager.handle_trade_tick(TradeTick::new(ctx.instrument.id, Price("50", ctx.instrument.price_precision).unwrap(), Quantity("70", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("v1").unwrap(), ts, ts, mint_addr_vol).unwrap());
        ts = ctx.clock.borrow_mut().advance_time_ns(ts.nanos() + 1000);
        ctx.manager.handle_trade_tick(TradeTick::new(ctx.instrument.id, Price("51", ctx.instrument.price_precision).unwrap(), Quantity("40", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("v2").unwrap(), ts, ts, mint_addr_vol).unwrap()); // Total vol 110, bar forms

        match ctx.bar_handler_rx.try_recv() {
            Ok(mint_bar) => {
                assert_eq!(mint_bar.mint_address, mint_addr_vol);
                assert_eq!(mint_bar.bar.volume, Quantity("100", ctx.instrument.size_precision).unwrap());
                assert_eq!(mint_bar.bar.close, Price("51", ctx.instrument.price_precision).unwrap());
            },
            Err(e) => panic!("Volume bar not received: {:?}", e),
        }
        assert_eq!(ctx.manager.aggregators.get(&mint_addr_vol).unwrap().bar_type().spec().aggregation, BarAggregation::Volume);
    }

    // Similar test for ValueBarAggregator
    #[test]
    fn test_value_aggregator_dynamic_creation_and_bar() {
        let mut ctx = setup_manager(BarAggregation::Value, 5000, 10); // Value step 5000
        let mint_addr_val = make_dummy_mint_address(11);
        let mut ts = ctx.clock.borrow().now();

        // Tick 1: Price 100, Size 30 => Value 3000
        ctx.manager.handle_trade_tick(TradeTick::new(ctx.instrument.id, Price("100", ctx.instrument.price_precision).unwrap(), Quantity("30", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("val1").unwrap(), ts, ts, mint_addr_val).unwrap());
        ts = ctx.clock.borrow_mut().advance_time_ns(ts.nanos() + 1000);
        // Tick 2: Price 100, Size 30 => Value 3000. Total Value 6000. Bar forms (Value 5000).
        ctx.manager.handle_trade_tick(TradeTick::new(ctx.instrument.id, Price("100", ctx.instrument.price_precision).unwrap(), Quantity("30", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("val2").unwrap(), ts, ts, mint_addr_val).unwrap());

        match ctx.bar_handler_rx.try_recv() {
            Ok(mint_bar) => {
                assert_eq!(mint_bar.mint_address, mint_addr_val);
                 // Volume for bar: 30 (all of first) + 20 (2000 value / 100 price from second) = 50
                assert_eq!(mint_bar.bar.volume, Quantity("50", ctx.instrument.size_precision).unwrap());
                assert_eq!(mint_bar.bar.close, Price("100", ctx.instrument.price_precision).unwrap());
            },
            Err(e) => panic!("Value bar not received: {:?}", e),
        }
        assert_eq!(ctx.manager.aggregators.get(&mint_addr_val).unwrap().bar_type().spec().aggregation, BarAggregation::Value);
    }

    // Test for TimeBarAggregator focusing on handle_trade driven completion and timeout
    #[test]
    fn test_time_aggregator_dynamic_creation_and_timeout_flush() {
        let mut ctx = setup_manager(BarAggregation::Second, 1, 5); // 1-Second bars, 5s timeout
        let mint_addr_time = make_dummy_mint_address(12);
        let mut ts = ctx.clock.borrow().now();

        // Feed one trade
        ctx.manager.handle_trade_tick(TradeTick::new(ctx.instrument.id, Price("1.0", ctx.instrument.price_precision).unwrap(), Quantity("10", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("time1").unwrap(), ts, ts, mint_addr_time).unwrap());
        assert_eq!(ctx.manager.aggregators.len(), 1);

        // Advance time beyond inactivity timeout
        ctx.clock.borrow_mut().advance_time_ns(ts.nanos() + (6 * NANOS_IN_SECOND)); // 6 seconds
        ctx.manager.check_timeouts();

        assert!(ctx.manager.aggregators.is_empty(), "TimeBarAggregator should be removed after timeout");

        // Check if partial bar was flushed by stop()
        match ctx.bar_handler_rx.try_recv() {
            Ok(mint_bar) => {
                assert_eq!(mint_bar.mint_address, mint_addr_time);
                assert_eq!(mint_bar.bar.volume, Quantity("10", ctx.instrument.size_precision).unwrap());
            },
            Err(e) => panic!("TimeBarAggregator partial bar not flushed on timeout: {:?}", e),
        }
        // Note: Actual timer-based bar emission for dynamically created TimeBarAggregators is not tested here
        // due to the complexities of calling its `start` method after boxing.
    }

    #[test]
    fn test_zero_inactivity_timeout() {
        let mut ctx = setup_manager(BarAggregation::Tick, 2, 0); // Zero timeout
        let mint_addr = make_dummy_mint_address(1);
        let ts = ctx.clock.borrow().now();

        // First tick creates aggregator
        ctx.manager.handle_trade_tick(TradeTick::new(ctx.instrument.id, Price("100", ctx.instrument.price_precision).unwrap(), Quantity("10", ctx.instrument.size_precision).unwrap(), AggressorSide::Buyer, TradeId::from_str("zt1").unwrap(), ts, ts, mint_addr).unwrap());
        assert_eq!(ctx.manager.aggregators.len(), 1);

        // Immediately check timeouts
        ctx.manager.check_timeouts();
        assert!(ctx.manager.aggregators.is_empty(), "Aggregator should be removed immediately with zero timeout if no new tick follows instantly");

        // Check for flushed bar
         match ctx.bar_handler_rx.try_recv() {
            Ok(mint_bar) => {
                assert_eq!(mint_bar.mint_address, mint_addr);
                assert_eq!(mint_bar.bar.volume, Quantity("10", ctx.instrument.size_precision).unwrap());
            },
            Err(e) => panic!("Zero timeout bar not flushed: {:?}", e),
        }
    }
}
