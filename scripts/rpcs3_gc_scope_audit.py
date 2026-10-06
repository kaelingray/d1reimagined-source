"""Audit the actual native GC prefix-detachment method with controlled contention.

RPCS3 reader_lock::upgrade documents that observed state may change. The fixture
injects a writer at that boundary, or before exclusive acquisition, and tracks
which resource callbacks run. No Vulkan execution or native root cause is claimed.
"""
import argparse
from pathlib import Path

from rpcs3_present_lifetime_audit import block


def generate(root):
    header = (root / "rpcs3/Emu/RSX/VK/VKResourceManager.h").read_text(encoding="utf-8")
    signature = "void eid_completed(u64 eid)"
    method = signature + "\n" + block(header, header.index(signature))
    return PREAMBLE + method + "\n};\n" + TESTS


PREAMBLE = r'''
#include <cstdint>
#include <functional>
#include <iostream>
#include <list>
#include <mutex>
#include <string>
#include <utility>
#include <vector>
using u64=std::uint64_t;
namespace d1diag { bool descriptor_logging() { return false; } }
struct diagnostic_logger { template<class... T> void notice(const char*, T...) {} } d1_log;
struct test_mutex {
    bool exclusive=false,reader=false;
    std::function<void()> before_exclusive;
    void writer_wins() { auto callback=std::exchange(before_exclusive,{}); if(callback) callback(); }
    void lock() { writer_wins(); exclusive=true; }
    void unlock() { exclusive=false; }
    void lock_shared() { reader=true; }
    void unlock_shared() { reader=false; }
};
struct reader_lock {
    test_mutex& mutex; bool upgraded=false;
    explicit reader_lock(test_mutex& value):mutex(value) { mutex.lock_shared(); }
    void upgrade() { mutex.reader=false; mutex.writer_wins(); mutex.exclusive=true; upgraded=true; }
    ~reader_lock() { if(upgraded) mutex.unlock(); else mutex.unlock_shared(); }
};
std::vector<int> released;
bool callbacks_outside_lock=true;
struct eid_scope_t {
    u64 eid; int resource; test_mutex* guard;
    eid_scope_t(u64 value,int id,test_mutex& lock):eid(value),resource(id),guard(&lock){}
    eid_scope_t(const eid_scope_t&)=delete;
    ~eid_scope_t() {
        released.push_back(resource);
        callbacks_outside_lock &= !guard->exclusive && !guard->reader;
    }
};
struct resource_manager {
    test_mutex m_eid_map_lock;
    std::list<eid_scope_t> m_eid_map;
    void add(u64 eid,int resource) { m_eid_map.emplace_back(eid,resource,m_eid_map_lock); }
'''

TESTS = r'''
int checks=0,failures=0;
void check(const std::string& name,bool pass) {
    ++checks; if(!pass) ++failures;
    std::cout<<(pass?"PASS ":"FAIL ")<<name<<"\n";
}
void reset() { released.clear(); callbacks_outside_lock=true; }
int main() {
    {
        reset(); resource_manager gc; gc.eid_completed(0);
        check("empty collector releases nothing",released.empty() && gc.m_eid_map.empty());
        check("empty collector releases its lock",!gc.m_eid_map_lock.exclusive && !gc.m_eid_map_lock.reader);
    }
    {
        reset(); resource_manager gc; gc.add(4,1); gc.add(5,2); gc.add(8,3);
        gc.eid_completed(5);
        check("only the completed prefix is detached",released==std::vector<int>({1,2}) && gc.m_eid_map.size()==1 && gc.m_eid_map.front().eid==8);
        check("resource callbacks run outside the map lock",callbacks_outside_lock);
        gc.eid_completed(8);
        check("later completion releases the retained scope",released==std::vector<int>({1,2,3}) && gc.m_eid_map.empty());
    }
    {
        reset(); resource_manager gc; gc.add(8,1); gc.eid_completed(7);
        check("future-only scope remains retained",released.empty() && gc.m_eid_map.size()==1);
    }
    {
        reset(); resource_manager gc; gc.add(4,1);
        gc.m_eid_map_lock.before_exclusive=[&] { gc.add(9,2); };
        gc.eid_completed(4);
        check("writer appending after observed end cannot release a pending scope",released==std::vector<int>({1}) && gc.m_eid_map.size()==1 && gc.m_eid_map.front().eid==9);
        check("appended pending resource remains available for later completion",gc.m_eid_map.size()==1);
        check("contended append callbacks still run outside the lock",callbacks_outside_lock);
        gc.eid_completed(9);
        check("appended resource is released when its event completes",released==std::vector<int>({1,2}));
    }
    {
        reset(); resource_manager gc; gc.add(4,1);
        gc.m_eid_map_lock.before_exclusive=[&] { ++gc.m_eid_map.back().eid; };
        gc.eid_completed(4);
        check("writer delaying the current scope cannot release it early",released.empty() && gc.m_eid_map.size()==1 && gc.m_eid_map.front().eid==5);
        gc.eid_completed(5);
        check("delayed scope releases at the updated watermark",released==std::vector<int>({1}));
    }
    {
        reset(); resource_manager gc; gc.add(2,1); gc.add(3,2); gc.add(7,3);
        gc.m_eid_map_lock.before_exclusive=[&] { gc.add(8,4); };
        gc.eid_completed(3);
        check("partial-prefix contention preserves all future resources",released==std::vector<int>({1,2}) && gc.m_eid_map.size()==2);
        check("partial-prefix callbacks are outside the lock",callbacks_outside_lock);
    }
    std::cout<<checks<<" GC scope checks, "<<failures<<" failed\n";
    return failures?1:0;
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out / "gc-scope-audit.cpp"
    output.write_text(generate(args.source), encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
