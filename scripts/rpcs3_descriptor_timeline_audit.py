"""Exercise actual descriptor trace gating and disposable identity with CPU mocks.

Extracts the private C++ helpers. Tests opt-in polling, concurrent budget/order
allocation and unchanged ownership. This does not execute Vulkan or prove a fix.
"""
import argparse
from pathlib import Path

from rpcs3_present_lifetime_audit import block


def generate(root):
    header = (root / "rpcs3/Emu/RSX/d1diag.h").read_text(encoding="utf-8")
    gc = (root / "rpcs3/Emu/RSX/VK/vkutils/garbage_collector.h").read_text(encoding="utf-8")
    helpers = "\n".join(signature + "\n" + block(header, header.index(signature))
                        for signature in ("inline bool descriptor_logging()",
                                          "inline u64 next_descriptor_submit_order()"))
    disposable = "class disposable_t\n" + block(gc, gc.index("class disposable_t")) + ";\n"
    return PREAMBLE + helpers + "\n}\nnamespace vk {\n" + disposable + "\n}\n" + TESTS


PREAMBLE = r'''
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <functional>
#include <iostream>
#include <mutex>
#include <string>
#include <thread>
#include <utility>
#include <vector>
using u64=std::uint64_t;
std::atomic<u64> clock_us{0}, file_checks{0}, cap_notices{0};
std::atomic<bool> marker{false};
u64 get_system_time() { return clock_us.load(); }
namespace fs {
bool is_file(const std::string& name) {
    ++file_checks;
    if(name!="private-control/d1descriptor") throw std::runtime_error("wrong marker");
    return marker.load();
}
}
struct diagnostic_logger {
    void notice(const char*,u64 limit) {
        if(limit!=262'144) throw std::runtime_error("wrong cap");
        ++cap_notices;
    }
} d1_log;
namespace d1diag {
const std::string& ctl_dir() { static std::string dir="private-control/";return dir; }
'''


TESTS = r'''
int checks=0,failures=0;
void check(const std::string& name,bool pass) {
    ++checks;if(!pass)++failures;
    std::cout<<(pass?"PASS ":"FAIL ")<<name<<"\n";
}
struct resource {
    int& destroyed;
    explicit resource(int& value):destroyed(value){}
    ~resource(){++destroyed;}
};
int main() {
    check("default trace is disabled",!d1diag::descriptor_logging());
    clock_us=600'000;
    check("missing marker keeps trace disabled",!d1diag::descriptor_logging());
    check("first eligible check polls the exact private marker once",file_checks==1);
    marker=true;clock_us=700'000;
    check("marker polling is throttled",!d1diag::descriptor_logging() && file_checks==1);
    clock_us=1'200'000;
    std::atomic<u64> accepted{d1diag::descriptor_logging()?1u:0u};
    check("next poll arms the trace",accepted==1 && file_checks==2);
    auto reserve=[&]{for(int i=0;i<140'000;++i) if(d1diag::descriptor_logging()) ++accepted;};
    std::thread worker_a(reserve),worker_b(reserve);worker_a.join();worker_b.join();
    check("concurrent callers share exactly one process budget",accepted==262'144);
    check("exhaustion is reported exactly once",cap_notices==1);
    check("concurrent records do not poll the filesystem again",file_checks==2);
    marker=false;clock_us=1'800'000;
    check("removing the marker disables logging",!d1diag::descriptor_logging());
    marker=true;clock_us=2'400'000;
    check("reactivation cannot reset the process budget",!d1diag::descriptor_logging() && cap_notices==1);

    std::vector<u64> orders_a,orders_b;
    auto orders=[](std::vector<u64>& values){for(int i=0;i<100'000;++i)values.push_back(d1diag::next_descriptor_submit_order());};
    std::thread a(orders,std::ref(orders_a)),b(orders,std::ref(orders_b));a.join();b.join();
    orders_a.insert(orders_a.end(),orders_b.begin(),orders_b.end());
    std::sort(orders_a.begin(),orders_a.end());
    bool unique=true;
    for(u64 i=0;i<orders_a.size();++i) unique &= orders_a[i]==i+1;
    check("concurrent ordinal reservations are unique and contiguous",unique);
    check("later submissions retain monotonic order",d1diag::next_descriptor_submit_order()==200'001);

    int destroyed=0;auto* object=new resource(destroyed);
    {
        auto first=vk::disposable_t::make(object);
        const auto& read_only=first;
        check("read-only diagnostic identity retains the original pointer",read_only.debug_handle()==object);
        check("identity inspection does not destroy the resource",destroyed==0);
        {
            auto second=std::move(first);
            check("move transfers identity without changing ownership",first.debug_handle()==nullptr && second.debug_handle()==object && destroyed==0);
        }
        check("moved resource is destroyed exactly once",destroyed==1);
    }
    check("moved-from disposal cannot destroy the resource twice",destroyed==1);
    std::cout<<checks<<" descriptor diagnostic checks, "<<failures<<" failed\n";
    return failures?1:0;
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out / "descriptor-timeline-audit.cpp"
    output.write_text(generate(args.source), encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
