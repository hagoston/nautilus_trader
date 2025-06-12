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
import warnings
import base58
import numpy as np

from nautilus_trader.core import nautilus_pyo3

from cpython.datetime cimport timedelta
from cpython.mem cimport PyMem_Free
from cpython.mem cimport PyMem_Malloc
from cpython.pycapsule cimport PyCapsule_Destructor
from cpython.pycapsule cimport PyCapsule_GetPointer
from cpython.pycapsule cimport PyCapsule_New
from libc.stdint cimport uint8_t, uint32_t, uint64_t, uintptr_t
from libc.string cimport memcpy # For copying mint_address bytes if needed

from nautilus_trader.core.correctness cimport Condition
from nautilus_trader.core.data cimport Data
from nautilus_trader.core.rust.core cimport CVec
from nautilus_trader.core.rust.model cimport (
    DEPTH10_LEN, AggregationSource, AggressorSide, Bar_t, BarSpecification_t, BarType_t,
    BookAction, BookOrder_t, Data_t, Data_t_Tag, InstrumentCloseType, MarketStatusAction,
    MarkPriceUpdate_t, OrderSide, Price_t, PriceRaw, PriceType, Quantity_t, QuantityRaw,
    RecordFlag, bar_eq, bar_hash, bar_new, bar_specification_eq, bar_specification_ge,
    bar_specification_gt, bar_specification_hash, bar_specification_le, bar_specification_lt,
    bar_specification_new, bar_specification_to_cstr, bar_to_cstr, bar_type_aggregation_source,
    bar_type_check_parsing, bar_type_composite, bar_type_eq, bar_type_from_cstr, bar_type_ge,
    bar_type_gt, bar_type_hash, bar_type_instrument_id, bar_type_is_composite, bar_type_is_standard,
    bar_type_le, bar_type_lt, bar_type_new, bar_type_new_composite, bar_type_spec, bar_type_standard,
    bar_type_to_cstr, book_order_debug_to_cstr, book_order_eq, book_order_exposure, book_order_hash,
    book_order_new, book_order_signed_size, instrument_id_from_cstr, mark_price_update_eq,
    mark_price_update_hash, mark_price_update_new, mark_price_update_to_cstr, orderbook_delta_eq,
    orderbook_delta_hash, orderbook_delta_new, orderbook_deltas_clone, orderbook_deltas_drop,
    orderbook_deltas_flags, orderbook_deltas_instrument_id, orderbook_deltas_is_snapshot,
    orderbook_deltas_new, orderbook_deltas_sequence, orderbook_deltas_ts_event,
    orderbook_deltas_ts_init, orderbook_deltas_vec_deltas, orderbook_deltas_vec_drop,
    orderbook_depth10_ask_counts_array, orderbook_depth10_asks_array,
    orderbook_depth10_bid_counts_array, orderbook_depth10_bids_array, orderbook_depth10_clone,
    orderbook_depth10_eq, orderbook_depth10_hash, orderbook_depth10_new, quote_tick_eq,
    quote_tick_hash, quote_tick_new, quote_tick_to_cstr, symbol_new, trade_id_new, trade_tick_eq,
    trade_tick_hash, trade_tick_new, trade_tick_to_cstr, venue_new,
    MintSpecificBar_t,
    mint_specific_bar_new,
    mint_specific_bar_eq,
    mint_specific_bar_hash
)
from nautilus_trader.core.string cimport cstr_to_pystr, pystr_to_cstr, ustr_to_pystr
from nautilus_trader.model.data cimport BarAggregation
from nautilus_trader.model.functions cimport (
    aggregation_source_from_str, aggressor_side_from_str, aggressor_side_to_str,
    bar_aggregation_from_str, bar_aggregation_to_str, book_action_from_str, book_action_to_str,
    instrument_close_type_from_str, instrument_close_type_to_str, market_status_action_from_str,
    market_status_action_to_str, order_side_from_str, order_side_to_str, price_type_from_str,
    price_type_to_str
)
from nautilus_trader.model.identifiers cimport InstrumentId, Symbol, TradeId, Venue
from nautilus_trader.model.objects cimport Price, Quantity, price_new, quantity_new


cdef inline BookOrder order_from_mem_c(BookOrder_t mem):
    cdef BookOrder order = BookOrder.__new__(BookOrder)
    order._mem = mem
    return order


cdef inline OrderBookDelta delta_from_mem_c(OrderBookDelta_t mem):
    cdef OrderBookDelta delta = OrderBookDelta.__new__(OrderBookDelta)
    delta._mem = mem
    return delta


cdef inline OrderBookDeltas deltas_from_mem_c(OrderBookDeltas_API mem):
    cdef OrderBookDeltas deltas = OrderBookDeltas.__new__(OrderBookDeltas)
    deltas._mem = orderbook_deltas_clone(&mem)
    return deltas


cdef inline OrderBookDepth10 depth10_from_mem_c(OrderBookDepth10_t mem):
    cdef OrderBookDepth10 depth10 = OrderBookDepth10.__new__(OrderBookDepth10)
    depth10._mem = mem
    return depth10


cdef inline QuoteTick quote_from_mem_c(QuoteTick_t mem):
    cdef QuoteTick quote = QuoteTick.__new__(QuoteTick)
    quote._mem = mem
    return quote


cdef inline TradeTick trade_from_mem_c(TradeTick_t mem):
    cdef TradeTick trade = TradeTick.__new__(TradeTick)
    trade._mem = mem
    return trade


cdef inline Bar bar_from_mem_c(Bar_t mem):
    cdef Bar bar = Bar.__new__(Bar)
    bar._mem = mem
    return bar


cdef inline MarkPriceUpdate mark_price_from_mem_c(MarkPriceUpdate_t mem):
    cdef MarkPriceUpdate obj = MarkPriceUpdate.__new__(MarkPriceUpdate)
    obj._mem = mem
    return obj

# Assuming all other class definitions (BarSpecification, BarType, Bar, TradeTick, etc.) are present above this point.
# ... (omitted for brevity)

cdef class MintSpecificBar(Data):
    """
    Represents a Bar that is specifically associated with a given Solana mint_address.

    This is typically used when bar aggregation is performed separately for different
    mint addresses, even if they pertain to the same primary trading instrument.

    Parameters
    ----------
    bar_instance : Bar
        The underlying Bar data (OHLCV, timestamps).
    mint_address_str : str, optional
        A Base58 encoded string representing a 32-byte Solana mint address.
        If `None`, no mint address is associated with this bar.
        If provided but invalid (not Base58, or not 32 bytes when decoded),
        it will be treated as `None`. Warnings for invalid formats are expected
        to be handled by the Rust layer if it performs validation during construction.

    Attributes
    ----------
    bar : Bar
        The underlying Bar object.
    mint_address : str or None
        The Base58 encoded 32-byte Solana mint address, or `None`.
    ts_event : int
        Inherited from Data, sourced from the underlying bar's ts_event.
    ts_init : int
        Inherited from Data, sourced from the underlying bar's ts_init.
    """

    def __init__(self, Bar bar_instance not None, str mint_address_str=None):
        cdef uint8_t c_mint_address[32]
        cdef bint c_has_mint_address = False
        cdef const uint8_t* mint_address_ptr = NULL

        if mint_address_str is not None:
            try:
                decoded_bytes = base58.b58decode(mint_address_str)
                if len(decoded_bytes) == 32:
                    for i in range(32):
                        c_mint_address[i] = decoded_bytes[i]
                    mint_address_ptr = c_mint_address
                    c_has_mint_address = True
                else:
                    # print(f"Warning: MintSpecificBar __init__: Decoded mint address has invalid length: {len(decoded_bytes)}")
                    pass
            except ValueError:
                # print(f"Warning: MintSpecificBar __init__: Failed to decode mint address: {mint_address_str}")
                pass

        self._mem = mint_specific_bar_new(bar_instance._mem, mint_address_ptr, c_has_mint_address)

    @staticmethod
    cdef MintSpecificBar from_mem_c(MintSpecificBar_t mem):
        cdef MintSpecificBar obj = MintSpecificBar.__new__(MintSpecificBar)
        obj._mem = mem
        return obj

    @property
    def bar(self) -> Bar:
        """The underlying `Bar` data (OHLCV, timestamps)."""
        return Bar.from_mem_c(self._mem.bar)

    @property
    def mint_address(self) -> str | None:
        """
        The Base58 encoded 32-byte Solana mint address associated with this bar.
        Returns `None` if no mint address is set.
        """
        if self._mem.has_mint_address:
            py_bytes_addr = bytes(self._mem.mint_address[:32])
            return base58.b58encode(py_bytes_addr).decode('utf-8')
        return None

    @property
    def ts_event(self) -> int:
        """UNIX timestamp (nanoseconds) of the underlying bar's event time."""
        return self._mem.bar.ts_event

    @property
    def ts_init(self) -> int:
        """UNIX timestamp (nanoseconds) of the underlying bar's initialization time."""
        return self._mem.bar.ts_init

    def __repr__(self) -> str:
        return (
            f"{type(self).__name__}("
            f"bar={self.bar!r}, "
            f"mint_address='{self.mint_address if self.mint_address is not None else 'None'}'"
            f")"
        )

    def __str__(self) -> str:
        return (
            f"{str(self.bar)}"
            f"|MINT={self.mint_address if self.mint_address is not None else 'NONE'}"
        )

    def __richcmp__(self, other, int op):
        if not isinstance(other, MintSpecificBar):
            if op == 2: return False # ==
            elif op == 3: return True # !=
            return NotImplemented

        cdef MintSpecificBar other_msb = <MintSpecificBar>other
        cdef bint result_bool

        if op == 2: # ==
            result_bool = mint_specific_bar_eq(&self._mem, &other_msb._mem)
            return result_bool
        elif op == 3: # !=
            result_bool = mint_specific_bar_eq(&self._mem, &other_msb._mem)
            return not result_bool
        return NotImplemented

    def __hash__(self) -> int:
        return <int>mint_specific_bar_hash(&self._mem)

    def __getstate__(self):
        return (Bar.to_dict_c(self.bar), self.mint_address)

    def __setstate__(self, state):
        bar_dict, mint_address_str = state
        temp_bar_obj = Bar.from_dict_c(bar_dict)

        cdef uint8_t c_mint_address[32]
        cdef bint c_has_mint_address = False
        cdef const uint8_t* mint_address_ptr = NULL

        if mint_address_str is not None:
            try:
                decoded_bytes = base58.b58decode(mint_address_str)
                if len(decoded_bytes) == 32:
                    for i in range(32):
                        c_mint_address[i] = decoded_bytes[i]
                    mint_address_ptr = c_mint_address
                    c_has_mint_address = True
            except ValueError:
                pass

        self._mem = mint_specific_bar_new(temp_bar_obj._mem, mint_address_ptr, c_has_mint_address)

    @staticmethod
    def from_dict(dict values) -> MintSpecificBar:
        """
        Create a MintSpecificBar from a dictionary representation.
        """
        Condition.not_none(values, "values")
        if "bar" not in values or not isinstance(values["bar"], dict):
            raise ValueError("Dictionary for MintSpecificBar missing 'bar' key or 'bar' is not a dict.")

        bar_instance = Bar.from_dict_c(values["bar"])
        mint_address_str = values.get("mint_address")

        return MintSpecificBar(bar_instance, mint_address_str)

    def to_dict(self):
        """
        Return a dictionary representation of this MintSpecificBar.
        """
        return {
            "type": type(self).__name__,
            "bar": Bar.to_dict_c(self.bar),
            "mint_address": self.mint_address,
        }

# Make sure this is after all other data classes, or at least after IndexPriceUpdate if it's the last one.
# (The rest of the file, including IndexPriceUpdate, is assumed to be above this)
