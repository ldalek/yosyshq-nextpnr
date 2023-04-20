/*
 *  nextpnr -- Next Generation Place and Route
 *
 *  Copyright (C) 2021-23  gatecat <gatecat@ds0.me>
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

#include "himbaechel_api.h"
#include "nextpnr.h"
#include "log.h"

NEXTPNR_NAMESPACE_BEGIN

HimbaechelArch *HimbaechelArch::list_head;
HimbaechelArch::HimbaechelArch(const std::string &name) : name(name)
{
    list_next = HimbaechelArch::list_head;
    HimbaechelArch::list_head = this;
}
std::string HimbaechelArch::list()
{
    std::string result;
    HimbaechelArch *cursor = HimbaechelArch::list_head;
    while (cursor) {
        if (!result.empty())
            result += ", ";
        result += cursor->name;
        cursor = cursor->list_next;
    }
    return result;
}
std::unique_ptr<HimbaechelAPI> HimbaechelArch::create(const std::string &name, const dict<std::string, std::string> &args)
{
    HimbaechelArch *cursor = HimbaechelArch::list_head;
    while (cursor) {
        if (cursor->name != name) {
            cursor = cursor->list_next;
            continue;
        }
        return cursor->create(args);
    }
    return {};
}

NEXTPNR_NAMESPACE_END

