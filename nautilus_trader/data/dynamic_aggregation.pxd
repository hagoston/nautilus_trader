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

from libc.stdint cimport uint8_t, uint64_t
from libc.stddef cimport void # For void* context

# FFI types from Rust (these should match what cbindgen produces)
from nautilus_trader.core.rust.model cimport InstrumentId_t
from nautilus_trader.core.rust.model cimport BarType_t
from nautilus_trader.core.rust.model cimport TradeTick_t
from nautilus_trader.core.rust.model cimport MintSpecificBar_t
# Assuming BarIntervalType from Rust FFI is exposed as CRustBarIntervalType (likely an int-based enum)
from nautilus_trader.core.rust.model cimport BarIntervalType as CRustBarIntervalType

# Python-level Cython classes that will be passed as arguments or used as members
from nautilus_trader.common.component cimport Clock
from nautilus_trader.model.identifiers cimport InstrumentId
from nautilus_trader.model.data cimport BarType, TradeTick, MintSpecificBar
from nautilus_trader.model.enums cimport BarIntervalType # Python enum

# Opaque pointer for the Rust DynamicAggregatorManager
ctypedef void* DynamicAggregatorManager_t

# Pointer to the Rust Clock trait object.
ctypedef void* Clock_ptr

cdef extern from "nautilus_bindings.h": # Assumed C header name
    DynamicAggregatorManager_t* dynamic_aggregator_manager_new(
        InstrumentId_t instrument_id,
        BarType_t base_bar_type,
        uint64_t inactivity_timeout_ns,
        void (*bar_callback)(MintSpecificBar_t, void*), # C function pointer for callback
        void* callback_context, # Pointer to Cython DynamicAggregatorManager instance
        Clock_ptr clock_ptr,    # Pointer to the Rust Clock object
        uint8_t price_precision,
        uint8_t size_precision,
        bint time_bar_build_with_no_updates,
        bint time_bar_timestamp_on_close,
        CRustBarIntervalType time_bar_interval_type # Pass the C enum variant
    )

    void dynamic_aggregator_manager_handle_trade_tick(
        DynamicAggregatorManager_t* manager,
        TradeTick_t trade_tick # TradeTick_t is a struct, usually passed by value
    )

    void dynamic_aggregator_manager_check_timeouts(DynamicAggregatorManager_t* manager)
    void dynamic_aggregator_manager_drop(DynamicAggregatorManager_t* manager)

cdef class DynamicAggregatorManager:
    cdef DynamicAggregatorManager_t* _ptr  # Pointer to the Rust object
    cdef object _bar_handler_py  # Python callable: (MintSpecificBar) -> None
    cdef Clock _clock_py # Keep a reference to the Python Clock object passed in __init__

    # Configuration attributes, exposed as readonly properties
    cdef readonly InstrumentId instrument_id
    cdef readonly BarType base_bar_type
    cdef readonly uint64_t inactivity_timeout_ns
    cdef readonly uint8_t price_precision
    cdef readonly uint8_t size_precision
    cdef readonly bint time_bar_build_with_no_updates_config # Store resolved config
    cdef readonly bint time_bar_timestamp_on_close_config  # Store resolved config
    cdef readonly BarIntervalType time_bar_interval_type_config # Store Python enum

    cpdef void handle_trade_tick(self, TradeTick trade_tick)
    cpdef void check_timeouts(self)
    # __dealloc__ will handle drop, no explicit stop/close needed in .pxd for this.
