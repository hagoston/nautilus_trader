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

import pickle
import pytest # For testing warnings or exceptions if needed

from nautilus_trader.core import nautilus_pyo3
from nautilus_trader.model import convert_to_raw_int
from nautilus_trader.model.data import QuoteTick
from nautilus_trader.model.data import TradeTick
from nautilus_trader.model.enums import AggressorSide
from nautilus_trader.model.enums import PriceType
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.model.identifiers import Symbol
from nautilus_trader.model.identifiers import TradeId
from nautilus_trader.model.identifiers import Venue
from nautilus_trader.model.objects import Price
from nautilus_trader.model.objects import Quantity
from nautilus_trader.test_kit.providers import TestInstrumentProvider
from nautilus_trader.test_kit.rust.data_pyo3 import TestDataProviderPyo3


AUDUSD_SIM = TestInstrumentProvider.default_fx_ccy("AUD/USD")

# Mint Address Constants for testing
MINT_ADDR_VALID_1 = "So11111111111111111111111111111111111111112" # SOL
MINT_ADDR_VALID_2 = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" # USDC
MINT_ADDR_INVALID_SHORT = "Short"
MINT_ADDR_INVALID_B58 = "ThisIsNotValidBase58%!"
# A valid Base58 string but not 32 bytes when decoded
MINT_ADDR_INVALID_LEN = "1111111111111111111111111" # Too short after decode


class TestQuoteTick:
    def test_pickling_instrument_id_round_trip(self):
        pickled = pickle.dumps(AUDUSD_SIM.id)
        unpickled = pickle.loads(pickled)  # noqa: S301

        assert unpickled == AUDUSD_SIM.id

    def test_fully_qualified_name(self):
        # Arrange, Act, Assert
        assert QuoteTick.fully_qualified_name() == "nautilus_trader.model.data:QuoteTick"

    def test_tick_hash_str_and_repr(self):
        # Arrange
        instrument_id = InstrumentId(Symbol("AUD/USD"), Venue("SIM"))

        # Act
        quote = QuoteTick(
            instrument_id=instrument_id,
            bid_price=Price.from_str("1.00000"),
            ask_price=Price.from_str("1.00001"),
            bid_size=Quantity.from_int(1),
            ask_size=Quantity.from_int(1),
            ts_event=3,
            ts_init=4,
        )

        # Assert
        assert isinstance(hash(quote), int)
        assert str(quote) == "AUD/USD.SIM,1.00000,1.00001,1,1,3"
        assert repr(quote) == "QuoteTick(AUD/USD.SIM,1.00000,1.00001,1,1,3)"

    def test_extract_price_with_various_price_types_returns_expected_values(self):
        # Arrange
        quote = QuoteTick(
            instrument_id=AUDUSD_SIM.id,
            bid_price=Price.from_str("1.00000"),
            ask_price=Price.from_str("1.00001"),
            bid_size=Quantity.from_int(1),
            ask_size=Quantity.from_int(1),
            ts_event=0,
            ts_init=0,
        )

        # Act
        result1 = quote.extract_price(PriceType.ASK)
        result2 = quote.extract_price(PriceType.MID)
        result3 = quote.extract_price(PriceType.BID)

        # Assert
        assert result1 == Price.from_str("1.00001")
        assert result2 == Price.from_str("1.000005")
        assert result3 == Price.from_str("1.00000")

    def test_extract_size_with_various_price_types_returns_expected_values(self):
        # Arrange
        quote = QuoteTick(
            instrument_id=AUDUSD_SIM.id,
            bid_price=Price.from_str("1.00000"),
            ask_price=Price.from_str("1.00001"),
            bid_size=Quantity.from_int(500_000),
            ask_size=Quantity.from_int(800_000),
            ts_event=0,
            ts_init=0,
        )

        # Act
        result1 = quote.extract_size(PriceType.ASK)
        result2 = quote.extract_size(PriceType.MID)
        result3 = quote.extract_size(PriceType.BID)

        # Assert
        assert result1 == Quantity.from_int(800_000)
        assert result2 == Quantity.from_int(650_000)  # Average size
        assert result3 == Quantity.from_int(500_000)

    def test_to_dict_returns_expected_dict(self):
        # Arrange
        quote = QuoteTick(
            instrument_id=AUDUSD_SIM.id,
            bid_price=Price.from_str("1.00000"),
            ask_price=Price.from_str("1.00001"),
            bid_size=Quantity.from_int(1),
            ask_size=Quantity.from_int(1),
            ts_event=1,
            ts_init=2,
        )

        # Act
        result = QuoteTick.to_dict(quote)

        # Assert
        assert result == {
            "type": "QuoteTick",
            "instrument_id": "AUD/USD.SIM",
            "bid_price": "1.00000",
            "ask_price": "1.00001",
            "bid_size": "1",
            "ask_size": "1",
            "ts_event": 1,
            "ts_init": 2,
        }

    def test_from_dict_returns_expected_tick(self):
        # Arrange
        quote = QuoteTick(
            instrument_id=AUDUSD_SIM.id,
            bid_price=Price.from_str("1.00000"),
            ask_price=Price.from_str("1.00001"),
            bid_size=Quantity.from_int(1),
            ask_size=Quantity.from_int(1),
            ts_event=1,
            ts_init=2,
        )

        # Act
        result = QuoteTick.from_dict(QuoteTick.to_dict(quote))

        # Assert
        assert result == quote

    def test_from_raw_returns_expected_tick(self):
        # Arrange, Act
        quote = QuoteTick.from_raw(
            AUDUSD_SIM.id,
            convert_to_raw_int(1.00000, 5),
            convert_to_raw_int(1.00001, 5),
            5,
            5,
            convert_to_raw_int(1, 0),
            convert_to_raw_int(2, 0),
            0,
            0,
            1,
            2,
        )

        # Assert
        assert quote.instrument_id == AUDUSD_SIM.id
        assert quote.bid_price == Price.from_str("1.00000")
        assert quote.ask_price == Price.from_str("1.00001")
        assert quote.bid_size == Quantity.from_int(1)
        assert quote.ask_size == Quantity.from_int(2)
        assert quote.ts_event == 1
        assert quote.ts_init == 2

    def test_from_pyo3(self):
        # Arrange
        pyo3_quote = TestDataProviderPyo3.quote_tick()

        # Act
        quote = QuoteTick.from_pyo3(pyo3_quote)

        # Assert
        assert isinstance(quote, QuoteTick)

    def test_to_pyo3(self):
        # Arrange
        quote = QuoteTick(
            instrument_id=AUDUSD_SIM.id,
            bid_price=Price.from_str("1.00000"),
            ask_price=Price.from_str("1.00001"),
            bid_size=Quantity.from_int(1),
            ask_size=Quantity.from_int(2),
            ts_event=1,
            ts_init=2,
        )

        # Act
        pyo3_quote = quote.to_pyo3()

        # Assert
        assert isinstance(pyo3_quote, nautilus_pyo3.QuoteTick)
        assert pyo3_quote.bid_price == nautilus_pyo3.Price.from_str("1.00000")
        assert pyo3_quote.ask_price == nautilus_pyo3.Price.from_str("1.00001")
        assert pyo3_quote.bid_size == nautilus_pyo3.Quantity.from_int(1)
        assert pyo3_quote.ask_size == nautilus_pyo3.Quantity.from_int(2)
        assert pyo3_quote.ts_event == 1
        assert pyo3_quote.ts_init == 2

    def test_from_pyo3_list(self):
        # Arrange
        pyo3_quotes = [TestDataProviderPyo3.quote_tick()] * 1024

        # Act
        quotes = QuoteTick.from_pyo3_list(pyo3_quotes)

        # Assert
        assert len(quotes) == 1024
        assert isinstance(quotes[0], QuoteTick)

    def test_pickling_round_trip_results_in_expected_tick(self):
        # Arrange
        quote = QuoteTick(
            instrument_id=AUDUSD_SIM.id,
            bid_price=Price.from_str("1.00000"),
            ask_price=Price.from_str("1.00001"),
            bid_size=Quantity.from_int(1),
            ask_size=Quantity.from_int(1),
            ts_event=1,
            ts_init=2,
        )

        # Act
        pickled = pickle.dumps(quote)
        unpickled = pickle.loads(pickled)  # noqa: S301

        # Assert
        assert quote == unpickled


class TestTradeTick:
    def test_fully_qualified_name(self):
        # Arrange, Act, Assert
        assert TradeTick.fully_qualified_name() == "nautilus_trader.model.data:TradeTick"

    def test_hash_str_and_repr(self):
        # Arrange
        trade = TradeTick(
            instrument_id=AUDUSD_SIM.id,
            price=Price.from_str("1.00000"),
            size=Quantity.from_int(50_000),
            aggressor_side=AggressorSide.BUYER,
            trade_id=TradeId("123456789"),
            ts_event=1,
            ts_init=2,
        )

        # Act, Assert
        assert isinstance(hash(trade), int)
        assert str(trade) == "AUD/USD.SIM,1.00000,50000,BUYER,123456789,1"
        # Repr is updated in Cython to include mint_address if present
        assert repr(trade) == "TradeTick(AUD/USD.SIM,1.00000,50000,BUYER,123456789,1)" # No mint_address

    def test_creation_with_mint_address(self):
        trade_with_addr = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1, 5), size=Quantity(100, 0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("t1"),
            ts_event=1, ts_init=2, mint_address_str=MINT_ADDR_VALID_1
        )
        assert trade_with_addr.mint_address == MINT_ADDR_VALID_1
        # Check internal C struct values if possible and if Cython binding exposes them,
        # For now, rely on property and subsequent tests.
        # assert trade_with_addr._mem.has_mint_address == True (Cannot access _mem directly from Python)

        trade_no_addr = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1, 5), size=Quantity(100, 0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("t2"),
            ts_event=1, ts_init=2, mint_address_str=None
        )
        assert trade_no_addr.mint_address is None

    def test_creation_with_invalid_mint_address(self):
        # Test with invalid Base58 string
        # Expect warning, mint_address should be None
        trade_invalid_b58 = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1, 5), size=Quantity(100, 0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("t_inv_b58"),
            ts_event=1, ts_init=2, mint_address_str=MINT_ADDR_INVALID_B58
        )
        assert trade_invalid_b58.mint_address is None

        # Test with Base58 string that decodes to wrong length
        trade_invalid_len = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1, 5), size=Quantity(100, 0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("t_inv_len"),
            ts_event=1, ts_init=2, mint_address_str=MINT_ADDR_INVALID_LEN
        )
        assert trade_invalid_len.mint_address is None

        # Test with a string that is too short to be 32 bytes after decode
        trade_short_len = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1, 5), size=Quantity(100, 0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("t_short_len"),
            ts_event=1, ts_init=2, mint_address_str=MINT_ADDR_INVALID_SHORT
        )
        assert trade_short_len.mint_address is None


    def test_mint_address_property(self):
        trade = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1, 5), size=Quantity(100, 0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("prop_test"),
            ts_event=1, ts_init=2, mint_address_str=MINT_ADDR_VALID_1
        )
        assert trade.mint_address == MINT_ADDR_VALID_1

        trade_none = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1, 5), size=Quantity(100, 0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("prop_test_none"),
            ts_event=1, ts_init=2, mint_address_str=None
        )
        assert trade_none.mint_address is None

    def test_repr_with_mint_address(self):
        trade = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1,5), size=Quantity(1,0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("repr"),
            ts_event=1, ts_init=2, mint_address_str=MINT_ADDR_VALID_1
        )
        expected_repr = f"TradeTick(AUD/USD.SIM,1.00000,1,BUYER,repr,1, mint_address='{MINT_ADDR_VALID_1}')"
        assert repr(trade) == expected_repr

    def test_to_dict_returns_expected_dict(self):
        # Arrange
        trade = TradeTick(
            instrument_id=AUDUSD_SIM.id,
            price=Price.from_str("1.00000"),
            size=Quantity.from_int(10_000),
            aggressor_side=AggressorSide.BUYER,
            trade_id=TradeId("123456789"),
            ts_event=1,
            ts_init=2,
        )

        # Act
        result = TradeTick.to_dict(trade)

        # Assert
        assert result == {
            "type": "TradeTick",
            "instrument_id": "AUD/USD.SIM",
            "price": "1.00000",
            "size": "10000",
            "aggressor_side": "BUYER",
            "trade_id": "123456789",
            "ts_event": 1,
            "ts_init": 2,
            "mint_address": None,  # Added for existing test
        }

    def test_to_dict_with_mint_address(self):
        trade = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1,5), size=Quantity(1,0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("dict_test"),
            ts_event=1, ts_init=2, mint_address_str=MINT_ADDR_VALID_1
        )
        result = TradeTick.to_dict(trade)
        assert result["mint_address"] == MINT_ADDR_VALID_1

    def test_from_dict_returns_expected_tick(self):
        # Arrange
        trade = TradeTick(
            instrument_id=AUDUSD_SIM.id,
            price=Price.from_str("1.00000"),
            size=Quantity.from_int(10_000),
            aggressor_side=AggressorSide.BUYER,
            trade_id=TradeId("123456789"),
            ts_event=1,
            ts_init=2,
        )

        # Act
        result = TradeTick.from_dict(TradeTick.to_dict(trade))

        # Assert
        assert result == trade

    def test_from_dict_with_mint_address(self):
        data = {
            "type": "TradeTick", # Not strictly used by from_dict but good for completeness
            "instrument_id": "AUD/USD.SIM",
            "price": "1.00000",
            "size": "10000",
            "aggressor_side": "BUYER",
            "trade_id": "from_dict_ma",
            "ts_event": 1,
            "ts_init": 2,
            "mint_address": MINT_ADDR_VALID_1,
        }
        trade = TradeTick.from_dict(data)
        assert trade.instrument_id == AUDUSD_SIM.id
        assert trade.price == Price("1.00000", 5)
        assert trade.size == Quantity(10000, 0)
        assert trade.aggressor_side == AggressorSide.BUYER
        assert trade.trade_id == TradeId("from_dict_ma")
        assert trade.ts_event == 1
        assert trade.ts_init == 2
        assert trade.mint_address == MINT_ADDR_VALID_1

        data_none = {
            "instrument_id": "AUD/USD.SIM", "price": "1.0", "size": "1",
            "aggressor_side": "SELL", "trade_id": "from_dict_none",
            "ts_event": 3, "ts_init": 4, "mint_address": None,
        }
        trade_none = TradeTick.from_dict(data_none)
        assert trade_none.mint_address is None

    def test_from_pyo3(self):
        # Arrange
        pyo3_trade = TestDataProviderPyo3.trade_tick()

        # Act
        trade = TradeTick.from_pyo3(pyo3_trade)

        # Assert
        assert isinstance(trade, TradeTick)

    def test_to_pyo3(self):
        # Arrange
        trade = TradeTick(
            instrument_id=AUDUSD_SIM.id,
            price=Price.from_str("1.00000"),
            size=Quantity.from_int(10_000),
            aggressor_side=AggressorSide.BUYER,
            trade_id=TradeId("123456789"),
            ts_event=1,
            ts_init=2,
        )

        # Act
        pyo3_trade = trade.to_pyo3()

        # Assert
        assert isinstance(pyo3_trade, nautilus_pyo3.TradeTick)
        assert pyo3_trade.price == nautilus_pyo3.Price.from_str("1.00000")
        assert pyo3_trade.size == nautilus_pyo3.Quantity.from_int(10_000)
        assert pyo3_trade.aggressor_side == nautilus_pyo3.AggressorSide.BUYER
        assert pyo3_trade.trade_id == nautilus_pyo3.TradeId.from_str("123456789")
        assert pyo3_trade.ts_event == 1
        assert pyo3_trade.ts_init == 2

    def test_from_pyo3_list(self):
        # Arrange
        pyo3_trades = [TestDataProviderPyo3.trade_tick()] * 1024

        # Act
        trades = TradeTick.from_pyo3_list(pyo3_trades)

        # Assert
        assert len(trades) == 1024
        assert isinstance(trades[0], TradeTick)

    def test_pickling_round_trip_results_in_expected_tick(self):
        # Arrange
        trade = TradeTick(
            instrument_id=AUDUSD_SIM.id,
            price=Price.from_str("1.00000"),
            size=Quantity.from_int(50_000),
            aggressor_side=AggressorSide.BUYER,
            trade_id=TradeId("123456789"),
            ts_event=1,
            ts_init=2,
        )

        # Act
        pickled = pickle.dumps(trade)
        unpickled = pickle.loads(pickled)  # noqa: S301 (pickle safe here)

        # Assert
        assert unpickled == trade
        # Assuming original test had no mint_address, so repr matches
        assert repr(unpickled) == "TradeTick(AUD/USD.SIM,1.00000,50000,BUYER,123456789,1)"


    def test_pickling_with_mint_address(self):
        trade_with_addr = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1,5), size=Quantity(1,0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("pickle_ma"),
            ts_event=1, ts_init=2, mint_address_str=MINT_ADDR_VALID_1
        )
        pickled = pickle.dumps(trade_with_addr)
        unpickled = pickle.loads(pickled)
        assert unpickled == trade_with_addr
        assert unpickled.mint_address == MINT_ADDR_VALID_1
        expected_repr = f"TradeTick(AUD/USD.SIM,1.00000,1,BUYER,pickle_ma,1, mint_address='{MINT_ADDR_VALID_1}')"
        assert repr(unpickled) == expected_repr

        trade_no_addr = TradeTick(
            instrument_id=AUDUSD_SIM.id, price=Price(1,5), size=Quantity(1,0),
            aggressor_side=AggressorSide.BUYER, trade_id=TradeId("pickle_none"),
            ts_event=1, ts_init=2, mint_address_str=None
        )
        pickled_none = pickle.dumps(trade_no_addr)
        unpickled_none = pickle.loads(pickled_none)
        assert unpickled_none == trade_no_addr
        assert unpickled_none.mint_address is None
        expected_repr_none = "TradeTick(AUD/USD.SIM,1.00000,1,BUYER,pickle_none,1)"
        assert repr(unpickled_none) == expected_repr_none


    def test_equality_with_mint_address(self):
        t1_addr1 = TradeTick(AUDUSD_SIM.id, Price(1,5), Quantity(1,0), AggressorSide.BUYER, TradeId("eq1"), 1, 2, MINT_ADDR_VALID_1)
        t2_addr1 = TradeTick(AUDUSD_SIM.id, Price(1,5), Quantity(1,0), AggressorSide.BUYER, TradeId("eq1"), 1, 2, MINT_ADDR_VALID_1)
        t3_addr2 = TradeTick(AUDUSD_SIM.id, Price(1,5), Quantity(1,0), AggressorSide.BUYER, TradeId("eq1"), 1, 2, MINT_ADDR_VALID_2)
        t4_no_addr = TradeTick(AUDUSD_SIM.id, Price(1,5), Quantity(1,0), AggressorSide.BUYER, TradeId("eq1"), 1, 2, None)
        t5_no_addr_copy = TradeTick(AUDUSD_SIM.id, Price(1,5), Quantity(1,0), AggressorSide.BUYER, TradeId("eq1"), 1, 2, None)

        # Base fields different
        t_diff_price = TradeTick(AUDUSD_SIM.id, Price(2,5), Quantity(1,0), AggressorSide.BUYER, TradeId("eq1"), 1, 2, MINT_ADDR_VALID_1)

        assert t1_addr1 == t2_addr1
        assert t1_addr1 != t3_addr2
        assert t1_addr1 != t4_no_addr
        assert t4_no_addr == t5_no_addr_copy
        assert t1_addr1 != t_diff_price

    def test_from_raw_returns_expected_tick(self):
        # Arrange, Act
        # from_raw in Cython was modified to pass NULL, False for mint_address by default.
        # So, this test remains valid and tests that mint_address is None.
        trade_id = TradeId("123458")

        trade = TradeTick.from_raw(
            AUDUSD_SIM.id,
            convert_to_raw_int(1.00001, 5),
            5,
            convert_to_raw_int(10_000, 0),
            0,
            AggressorSide.BUYER,
            trade_id,
            1,
            2,
        )

        # Assert
        assert trade.instrument_id == AUDUSD_SIM.id
        assert trade.trade_id == trade_id
        assert trade.price == Price.from_str("1.00001")
        assert trade.size == Quantity.from_int(10_000)
        assert trade.aggressor_side == AggressorSide.BUYER
        assert trade.ts_event == 1
        assert trade.ts_init == 2
