# -------------------------------------------------------------------------------------------------
#  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
#  https://nautechsystems.io
#
#  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
#  You may not use this file except in compliance with the License.
#  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
#
#  Unless required by applicable law or agreed to in writing, software
#  distributed under the License is distributed on an "AS IS" BASIS,
#  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
#  See the License for the specific language governing permissions and
#  limitations under the License.
# -------------------------------------------------------------------------------------------------

import unittest
from decimal import Decimal
from functools import partial

from nautilus_trader.common.component import TestClock
from nautilus_trader.core.datetime import dt_to_unix_nanos, UNIX_EPOCH
from nautilus_trader.data.aggregation import (
    TickBarAggregator,
    VolumeBarAggregator,
    ValueBarAggregator,
    TimeBarAggregator,
)
from nautilus_trader.model.data import Bar, BarSpecification, BarType, QuoteTick, TradeTick
from nautilus_trader.model.enums import (
    AggregationSource,
    AggressorSide,
    BarAggregation,
    PriceType,
)
from nautilus_trader.model.identifiers import InstrumentId, TradeId
from nautilus_trader.model.instruments.currency_pair import CurrencyPair
from nautilus_trader.model.instruments.equity import Equity
from nautilus_trader.model.objects import Price, Quantity
from nautilus_trader.test_kit.providers import TestInstrumentProvider
from nautilus_trader.model.instruments.stubs import equity_aapl_stub, audusd_fx_ccy_stub

# Mint Address Constants for testing
MINT_ADDR_VALID_1 = "So11111111111111111111111111111111111111112"  # SOL
MINT_ADDR_VALID_2 = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"  # USDC
MINT_ADDR_VALID_3 = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"  # USDT

# Helper function to create TradeTick instances for tests
def _make_trade_tick(
    instrument: Equity | CurrencyPair,
    price_val: str,
    size_val: str,
    ts_event: int,
    aggressor_side: AggressorSide = AggressorSide.BUYER,
    trade_id_suffix: str = "",
    mint_address_str: str | None = None,
) -> TradeTick:
    return TradeTick(
        instrument_id=instrument.id,
        price=Price(price_val, instrument.price_precision),
        size=Quantity(size_val, instrument.size_precision),
        aggressor_side=aggressor_side,
        trade_id=TradeId(f"test_trade_{trade_id_suffix}{ts_event}"),
        ts_event=ts_event,
        ts_init=ts_event + 1,
        mint_address_str=mint_address_str,
    )

class TestTickBarAggregatorMintFilter(unittest.TestCase):
    def setUp(self):
        self.instrument = equity_aapl_stub()
        self.bar_spec = BarSpecification(2, BarAggregation.TICK, PriceType.LAST)
        self.bar_type = BarType(self.instrument.id, self.bar_spec, AggregationSource.INTERNAL)
        self.aggregated_bars = []

    def bar_handler(self, bar: Bar):
        self.aggregated_bars.append(bar)

    def test_filters_by_mint_address(self):
        aggregator = TickBarAggregator(
            instrument=self.instrument,
            bar_type=self.bar_type,
            handler=self.bar_handler,
            mint_address_str=MINT_ADDR_VALID_1,
        )
        ticks = [
            _make_trade_tick(self.instrument, "150.00", "10", 1000, trade_id_suffix="m1", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "150.10", "12", 1001, trade_id_suffix="nm1", mint_address_str=MINT_ADDR_VALID_2),
            _make_trade_tick(self.instrument, "150.20", "15", 1002, trade_id_suffix="none1", mint_address_str=None),
            _make_trade_tick(self.instrument, "150.30", "18", 1003, trade_id_suffix="m2", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "150.40", "20", 1004, trade_id_suffix="m3", mint_address_str=MINT_ADDR_VALID_1),
        ]
        for tick in ticks:
            aggregator.handle_trade_tick(tick)

        self.assertEqual(len(self.aggregated_bars), 1)
        bar = self.aggregated_bars[0]
        self.assertEqual(bar.open, Price("150.00", self.instrument.price_precision))
        self.assertEqual(bar.high, Price("150.30", self.instrument.price_precision))
        self.assertEqual(bar.low, Price("150.00", self.instrument.price_precision))
        self.assertEqual(bar.close, Price("150.30", self.instrument.price_precision))
        self.assertEqual(bar.volume, Quantity("28", self.instrument.size_precision))
        self.assertEqual(aggregator._builder.count, 1)
        self.assertEqual(aggregator._builder._open, Price("150.40", self.instrument.price_precision))

    def test_no_filter(self):
        aggregator = TickBarAggregator(
            instrument=self.instrument,
            bar_type=self.bar_type,
            handler=self.bar_handler,
            mint_address_str=None,
        )
        ticks = [
            _make_trade_tick(self.instrument, "150.00", "10", 1000, trade_id_suffix="any1", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "150.10", "12", 1001, trade_id_suffix="any2", mint_address_str=MINT_ADDR_VALID_2),
            _make_trade_tick(self.instrument, "150.20", "15", 1002, trade_id_suffix="any3", mint_address_str=None),
            _make_trade_tick(self.instrument, "150.30", "18", 1003, trade_id_suffix="any4", mint_address_str=MINT_ADDR_VALID_1),
        ]
        for tick in ticks:
            aggregator.handle_trade_tick(tick)

        self.assertEqual(len(self.aggregated_bars), 2)
        self.assertEqual(self.aggregated_bars[0].volume, Quantity("22", self.instrument.size_precision))
        self.assertEqual(self.aggregated_bars[1].volume, Quantity("33", self.instrument.size_precision))

    def test_filter_with_no_matching_trades(self):
        aggregator = TickBarAggregator(
            instrument=self.instrument,
            bar_type=self.bar_type,
            handler=self.bar_handler,
            mint_address_str=MINT_ADDR_VALID_3,
        )
        ticks = [
            _make_trade_tick(self.instrument, "150.00", "10", 1000, trade_id_suffix="f1", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "150.10", "12", 1001, trade_id_suffix="f2", mint_address_str=MINT_ADDR_VALID_2),
            _make_trade_tick(self.instrument, "150.20", "15", 1002, trade_id_suffix="f3", mint_address_str=None),
        ]
        for tick in ticks:
            aggregator.handle_trade_tick(tick)
        self.assertEqual(len(self.aggregated_bars), 0)


class TestVolumeBarAggregatorMintFilter(unittest.TestCase):
    def setUp(self):
        self.instrument = equity_aapl_stub()
        self.bar_spec = BarSpecification(20, BarAggregation.VOLUME, PriceType.LAST)
        self.bar_type = BarType(self.instrument.id, self.bar_spec, AggregationSource.INTERNAL)
        self.aggregated_bars = []

    def bar_handler(self, bar: Bar):
        self.aggregated_bars.append(bar)

    def test_filters_by_mint_address(self):
        aggregator = VolumeBarAggregator(
            instrument=self.instrument,
            bar_type=self.bar_type,
            handler=self.bar_handler,
            mint_address_str=MINT_ADDR_VALID_1,
        )
        ticks = [
            _make_trade_tick(self.instrument, "150.00", "10", 1000, trade_id_suffix="m1", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "150.10", "5",  1001, trade_id_suffix="nm1", mint_address_str=MINT_ADDR_VALID_2),
            _make_trade_tick(self.instrument, "150.20", "8",  1002, trade_id_suffix="none1", mint_address_str=None),
            _make_trade_tick(self.instrument, "150.30", "15", 1003, trade_id_suffix="m2", mint_address_str=MINT_ADDR_VALID_1),
        ]
        for tick in ticks:
            aggregator.handle_trade_tick(tick)

        self.assertEqual(len(self.aggregated_bars), 1)
        bar = self.aggregated_bars[0]
        self.assertEqual(bar.volume, Quantity("20", self.instrument.size_precision))
        self.assertEqual(bar.close, Price("150.30", self.instrument.price_precision))
        self.assertEqual(aggregator._builder.volume, Quantity("5", self.instrument.size_precision))

    def test_no_filter(self):
        aggregator = VolumeBarAggregator(
            instrument=self.instrument, bar_type=self.bar_type, handler=self.bar_handler, mint_address_str=None
        )
        ticks = [
            _make_trade_tick(self.instrument, "150.00", "10", 1000, trade_id_suffix="any1", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "150.10", "5",  1001, trade_id_suffix="any2", mint_address_str=MINT_ADDR_VALID_2),
            _make_trade_tick(self.instrument, "150.20", "8",  1002, trade_id_suffix="any3", mint_address_str=None), # Vol: 10+5+8=23. Bar (20), Rem 3.
        ]
        for tick in ticks: aggregator.handle_trade_tick(tick)

        self.assertEqual(len(self.aggregated_bars), 1)
        self.assertEqual(self.aggregated_bars[0].volume, Quantity("20", self.instrument.size_precision))
        self.assertEqual(self.aggregated_bars[0].close, Price("150.20", self.instrument.price_precision))
        self.assertEqual(aggregator._builder.volume, Quantity("3", self.instrument.size_precision))

    def test_filter_with_no_matching_trades(self):
        aggregator = VolumeBarAggregator(
            instrument=self.instrument, bar_type=self.bar_type, handler=self.bar_handler, mint_address_str=MINT_ADDR_VALID_3
        )
        ticks = [
            _make_trade_tick(self.instrument, "150.00", "10", 1000, trade_id_suffix="f1", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "150.10", "12", 1001, trade_id_suffix="f2", mint_address_str=MINT_ADDR_VALID_2),
        ]
        for tick in ticks: aggregator.handle_trade_tick(tick)
        self.assertEqual(len(self.aggregated_bars), 0)


class TestValueBarAggregatorMintFilter(unittest.TestCase):
    def setUp(self):
        self.instrument = equity_aapl_stub()
        self.bar_spec = BarSpecification(2000, BarAggregation.VALUE, PriceType.LAST) # PxS = 2000
        self.bar_type = BarType(self.instrument.id, self.bar_spec, AggregationSource.INTERNAL)
        self.aggregated_bars = []

    def bar_handler(self, bar: Bar):
        self.aggregated_bars.append(bar)

    def test_filters_by_mint_address(self):
        aggregator = ValueBarAggregator(
            instrument=self.instrument, bar_type=self.bar_type, handler=self.bar_handler, mint_address_str=MINT_ADDR_VALID_1
        )
        ticks = [
            _make_trade_tick(self.instrument, "100.00", "10", 1000, trade_id_suffix="m1", mint_address_str=MINT_ADDR_VALID_1), # Value 1000
            _make_trade_tick(self.instrument, "100.00", "5",  1001, trade_id_suffix="nm1", mint_address_str=MINT_ADDR_VALID_2),
            _make_trade_tick(self.instrument, "100.00", "15", 1003, trade_id_suffix="m2", mint_address_str=MINT_ADDR_VALID_1), # Value 1000+1500=2500. Bar (Value 2000), Rem Value 500
        ]
        for tick in ticks: aggregator.handle_trade_tick(tick)

        self.assertEqual(len(self.aggregated_bars), 1)
        bar = self.aggregated_bars[0]
        self.assertEqual(bar.volume, Quantity("20", self.instrument.size_precision))
        self.assertEqual(aggregator.get_cumulative_value(), Decimal("500.00"))

    def test_no_filter(self):
        aggregator = ValueBarAggregator(
            instrument=self.instrument, bar_type=self.bar_type, handler=self.bar_handler, mint_address_str=None
        )
        ticks = [ # Price 100. Value for bar 2000.
            _make_trade_tick(self.instrument, "100.00", "10", 1000, trade_id_suffix="any1", mint_address_str=MINT_ADDR_VALID_1), # Val 1000
            _make_trade_tick(self.instrument, "100.00", "5",  1001, trade_id_suffix="any2", mint_address_str=MINT_ADDR_VALID_2),  # Val 500. CumVal 1500
            _make_trade_tick(self.instrument, "100.00", "8",  1002, trade_id_suffix="any3", mint_address_str=None),             # Val 800. CumVal 1500+800=2300. Bar(2000), Rem 300
        ]
        for tick in ticks: aggregator.handle_trade_tick(tick)

        self.assertEqual(len(self.aggregated_bars), 1)
        self.assertEqual(self.aggregated_bars[0].volume, Quantity("20", self.instrument.size_precision))
        self.assertEqual(aggregator.get_cumulative_value(), Decimal("300.00"))

    def test_filter_with_no_matching_trades(self):
        aggregator = ValueBarAggregator(
            instrument=self.instrument, bar_type=self.bar_type, handler=self.bar_handler, mint_address_str=MINT_ADDR_VALID_3
        )
        ticks = [
            _make_trade_tick(self.instrument, "100.00", "10", 1000, trade_id_suffix="f1", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "100.00", "12", 1001, trade_id_suffix="f2", mint_address_str=MINT_ADDR_VALID_2),
        ]
        for tick in ticks: aggregator.handle_trade_tick(tick)
        self.assertEqual(len(self.aggregated_bars), 0)


class TestTimeBarAggregatorMintFilter(unittest.TestCase):
    def setUp(self):
        self.instrument = audusd_fx_ccy_stub()
        self.bar_spec = BarSpecification(1, BarAggregation.SECOND, PriceType.LAST)
        self.bar_type = BarType(self.instrument.id, self.bar_spec, AggregationSource.INTERNAL)
        self.aggregated_bars = []
        self.clock = TestClock(initial_time_ns=UNIX_EPOCH)

    def bar_handler(self, bar: Bar):
        self.aggregated_bars.append(bar)

    def test_filters_by_mint_address(self):
        aggregator = TimeBarAggregator(
            instrument=self.instrument, bar_type=self.bar_type, handler=self.bar_handler,
            clock=self.clock, mint_address_str=MINT_ADDR_VALID_1, build_with_no_updates=True
        )
        ticks = [
            _make_trade_tick(self.instrument, "1.0", "10", UNIX_EPOCH + 100_000_000, trade_id_suffix="m1", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "1.1", "12", UNIX_EPOCH + 200_000_000, trade_id_suffix="nm1", mint_address_str=MINT_ADDR_VALID_2),
            _make_trade_tick(self.instrument, "1.3", "18", UNIX_EPOCH + 400_000_000, trade_id_suffix="m2", mint_address_str=MINT_ADDR_VALID_1),
        ]
        for tick in ticks: aggregator.handle_trade_tick(tick)

        self.clock.advance_time_ns(UNIX_EPOCH + 1_000_000_000)

        self.assertEqual(len(self.aggregated_bars), 1)
        bar = self.aggregated_bars[0]
        self.assertEqual(bar.volume, Quantity("28", self.instrument.size_precision))
        self.assertEqual(bar.open, Price("1.0", self.instrument.price_precision))
        self.assertEqual(bar.close, Price("1.3", self.instrument.price_precision))

    def test_no_filter(self):
        aggregator = TimeBarAggregator(
            instrument=self.instrument, bar_type=self.bar_type, handler=self.bar_handler,
            clock=self.clock, mint_address_str=None, build_with_no_updates=True
        )
        ticks = [
            _make_trade_tick(self.instrument, "1.0", "10", UNIX_EPOCH + 100_000_000, trade_id_suffix="any1", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "1.1", "12", UNIX_EPOCH + 200_000_000, trade_id_suffix="any2", mint_address_str=MINT_ADDR_VALID_2),
            _make_trade_tick(self.instrument, "1.2", "15", UNIX_EPOCH + 300_000_000, trade_id_suffix="any3", mint_address_str=None),
        ]
        for tick in ticks: aggregator.handle_trade_tick(tick)
        self.clock.advance_time_ns(UNIX_EPOCH + 1_000_000_000)

        self.assertEqual(len(self.aggregated_bars), 1)
        bar = self.aggregated_bars[0]
        self.assertEqual(bar.volume, Quantity("37", self.instrument.size_precision))
        self.assertEqual(bar.close, Price("1.2", self.instrument.price_precision))

    def test_filter_with_no_matching_trades(self):
        aggregator = TimeBarAggregator(
            instrument=self.instrument, bar_type=self.bar_type, handler=self.bar_handler,
            clock=self.clock, mint_address_str=MINT_ADDR_VALID_3, build_with_no_updates=False # build_with_no_updates=False
        )
        ticks = [
            _make_trade_tick(self.instrument, "1.0", "10", UNIX_EPOCH + 100_000_000, trade_id_suffix="f1", mint_address_str=MINT_ADDR_VALID_1),
            _make_trade_tick(self.instrument, "1.1", "12", UNIX_EPOCH + 200_000_000, trade_id_suffix="f2", mint_address_str=MINT_ADDR_VALID_2),
        ]
        for tick in ticks: aggregator.handle_trade_tick(tick)
        self.clock.advance_time_ns(UNIX_EPOCH + 1_000_000_000)
        self.assertEqual(len(self.aggregated_bars), 0) # No bar if build_with_no_updates=False and no matching ticks


if __name__ == "__main__":
    unittest.main()
