/*
 *  nextpnr -- Next Generation Place and Route
 *
 *  Copyright (C) 2023  gatecat <gatecat@ds0.me>
 *  Copyright (C) 2023  Miodrag Milanovic <micko@yosyshq.com>
 *
 *  Permission to use, copy, modify, and/or distribute this software for any
 *  purpose with or without fee is hereby granted, provided that the above
 *  copyright notice and this permission notice appear in all copies.
 *
 *  THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 *  WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 *  MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
 *  ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 *  WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 *  ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 *  OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 *
 */

#ifndef HIMBAECHEL_NG_ULTRA_H
#define HIMBAECHEL_NG_ULTRA_H
#include <deque>

#include "extra_data.h"
#include "himbaechel_api.h"
#include "log.h"
#include "nextpnr.h"
#include "util.h"

#include "himbaechel_helpers.h"

#ifdef GTEST_API_
#define TESTABLE_PRIVATE public
#else
#define TESTABLE_PRIVATE private
#endif

NEXTPNR_NAMESPACE_BEGIN

struct NgUltraImpl : HimbaechelAPI
{
    ~NgUltraImpl();
    void init_database(Arch *arch) override;

    void init(Context *ctx) override;

    bool isBelLocationValid(BelId bel, bool explain_invalid = false) const override;
    IdString getBelBucketForCellType(IdString cell_type) const override;
    bool isValidBelForCellType(IdString cell_type, BelId bel) const override;

    // Flow management
    void pack() override;
    void postPlace() override;
    void preRoute() override;
    void postRoute() override;

    void configurePlacerHeap(PlacerHeapCfg &cfg) override;

    bool getClusterPlacement(ClusterId cluster, BelId root_bel,
                             std::vector<std::pair<CellInfo *, BelId>> &placement) const override;
    bool getChildPlacement(const BaseClusterInfo *cluster, Loc root_loc,
                                    std::vector<std::pair<CellInfo *, BelId>> &placement) const;

    BoundingBox getRouteBoundingBox(WireId src, WireId dst) const override;
    delay_t estimateDelay(WireId src, WireId dst) const override;
    delay_t predictDelay(BelId src_bel, IdString src_pin, BelId dst_bel, IdString dst_pin) const override;

    bool checkPipAvail(PipId pip) const override { return blocked_pips.count(pip)==0; }
    bool checkPipAvailForNet(PipId pip, const NetInfo *net) const override { return checkPipAvail(pip); };

public:
    int tile_lobe(int tile) const;
    IdString tile_name_id(int tile) const;
    std::string tile_name(int tile) const;

    bool is_fabric_clock_sink(const PortRef &ref);
    bool is_ring_clock_sink(const PortRef &ref);
    bool is_ring_over_tile_clock_sink(const PortRef &ref);
    bool is_tube_clock_sink(const PortRef &ref);

    bool is_ring_clock_source(const PortRef &ref);
    bool is_tube_clock_source(const PortRef &ref);

    dict<IdString,BelId> iom_bels;
    dict<std::string, std::string> bank_voltage;
    dict<BelId,IdString> global_capable_bels;
    dict<std::string,BelId> locations;
    dict<int,std::vector<GckConfig>> gck_per_lobe;

    pool<PipId> blocked_pips;
    dict<IdString, std::pair<IdString,IdString>> bank_to_ckg;
    dict<BelId, IdString> unused_wfg;
    dict<BelId, IdString> unused_pll;
    dict<BelId, BelId> dsp_cascade;

TESTABLE_PRIVATE:
    void write_bitstream_json(const std::string &filename);
    void route_lowskew();
    void parse_csv(const std::string &filename);
    void remove_constants();

    // Misc utility functions
    bool get_mux_data(BelId bel, IdString port, uint8_t *value);
    bool get_mux_data(WireId wire, uint8_t *value);

    const NGUltraTileInstExtraDataPOD *tile_extra_data(int tile) const;

    dict<IdString,pool<IdString>> fabric_clock_sinks;
    dict<IdString,pool<IdString>> ring_clock_sinks;
    dict<IdString,pool<IdString>> ring_over_tile_clock_sinks;
    dict<IdString,pool<IdString>> tube_clock_sinks;

    dict<IdString,pool<IdString>> ring_clock_source;
    dict<IdString,pool<IdString>> tube_clock_source;
};

NEXTPNR_NAMESPACE_END
#endif
