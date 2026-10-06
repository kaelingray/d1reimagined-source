"""Extract private descriptor retirement code into CPU lifetime-contract fixtures.

Mocks track submission completion and deferred callbacks; they do not execute Vulkan.
The exact previous source is supported for a before/after comparison.
"""
import argparse
from pathlib import Path
import re

from rpcs3_present_lifetime_audit import block


def generate(root, baseline=False):
    directory = root if baseline else root / "rpcs3/Emu/RSX/VK"
    pipeline = (directory / "VKProgramPipeline.cpp").read_text()
    header = (directory / "VKProgramPipeline.h").read_text()
    pools = directory if baseline else directory / "vkutils"
    descriptors = (pools / "descriptors.cpp").read_text()
    pool_header = (pools / "descriptors.h").read_text()
    functions = "\n".join(
        signature + block(pipeline, pipeline.index(signature))
        for signature in (
            "void release_idle_descriptor_tables()",
            "void descriptor_table_t::release_resources()",
            "VkDescriptorSet descriptor_table_t::commit()",
        )
    )
    next_pool = block(descriptors, descriptors.index("void descriptor_pool::next_subpool()"))
    # Extract the complete actual callback-enqueue branch. Allocation policy is irrelevant here.
    callback_branch = next_pool[1:next_pool.index("m_current_subpool_offset = 0;")]
    pending = re.search(r"bool has_pending_resets\(\) const[^\n]*", pool_header)
    pending_method = pending[0] if pending else "bool has_pending_resets() const { return false; }"
    pending_field = re.search(r"atomic_t<u32> m_pending_resets[^;]*;", pool_header)
    pending_declaration = pending_field[0] if pending_field else "atomic_t<u32> m_pending_resets = 0;"
    eid_field = re.search(r"u64 m_last_commit_eid[^;]*;", header)
    eid_declaration = eid_field[0] if eid_field else "u64 m_last_commit_eid = 0;"
    return r'''
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <functional>
#include <iostream>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <vector>
using u32 = std::uint32_t; using u64 = std::uint64_t; using usz = std::size_t;
using VkDescriptorSet = u64;
template<class T> using atomic_t = std::atomic<T>;
constexpr u32 umax = std::numeric_limits<u32>::max();
namespace d1diag { unsigned g_tables_released = 0; }
namespace vk {
u64 current_eid = 0, completed_eid = 0;
u64 current_event_id() { return current_eid; }
u64 last_completed_event_id() { return completed_eid; }
std::vector<std::function<void()>> callbacks;
struct gc_callback_t { std::function<void()> m_callback; gc_callback_t(std::function<void()> f): m_callback(f) {} };
struct fake_gc {
    void dispose(std::unique_ptr<gc_callback_t>& object) {
        callbacks.push_back(std::move(object->m_callback)); object.reset();
    }
} collector;
fake_gc* get_gc() { return &collector; }
struct descriptor_pool {
    u32 m_current_subpool_index = 0;
PENDING_DECLARATION
    unsigned resets = 0; bool pending_during_reset = false;
    void reset(u32, u32) { ++resets; pending_during_reset = has_pending_resets(); }
    void destroy() {}
    PENDING_METHOD
    void next_subpool() { CALLBACK_BRANCH }
};
struct descriptor_set {
    u64 handle = 0;
    u64 value() const { return handle; }
    explicit operator bool() const { return handle != 0; }
    descriptor_set& operator=(u64 h) { handle = h; return *this; }
    void on_bind() {}
};
namespace glsl {
struct descriptor_table_t {
    u64 m_last_commit_frame = 0;
EID_DECLARATION
    bool m_registered = false, m_any_descriptors_dirty = false;
    std::vector<bool> m_descriptors_dirty;
    std::vector<int> m_descriptor_template;
    u64 m_descriptor_template_cache_id = 0;
    std::unique_ptr<descriptor_pool> m_descriptor_pool;
    descriptor_set m_descriptor_set;
    bool allocation_flushes = false;
    u64 allocate_descriptor_set() {
        if (!m_descriptor_pool) m_descriptor_pool = std::make_unique<descriptor_pool>();
        if (allocation_flushes) ++current_eid;
        return current_eid + 100;
    }
    void create_descriptor_template() { m_descriptor_template.push_back(1); }
    void update_descriptor_template() {}
    u64 commit(); void release_resources();
};
std::mutex g_table_registry_mutex;
std::vector<descriptor_table_t*> g_table_registry;
atomic_t<u64> g_table_frame = 0;
FUNCTIONS
}
}
int main() {
    using namespace vk; using namespace vk::glsl;
    int checks=0, failures=0;
    auto check=[&](const char* name, bool ok) { ++checks; if (!ok) { ++failures; std::cout << "FAIL " << name << '\n'; } };
    auto initialize=[&] { g_table_registry.clear(); callbacks.clear(); g_table_frame=0; current_eid=0; completed_eid=0; };
    auto sweep=[&](u64 frame) { g_table_frame=frame-1; release_idle_descriptor_tables(); };
    {
        initialize(); descriptor_table_t t; t.commit(); sweep(320);
        check("age alone cannot release an unsubmitted zero-event table", t.m_registered && t.m_descriptor_pool);
    }
    {
        initialize(); current_eid=10; descriptor_table_t t; t.commit();
        check("dirty commit tracks current submission scope", t.m_last_commit_eid==10);
        completed_eid=9; sweep(320);
        check("GPU behind last use retains idle pool", t.m_registered && t.m_descriptor_pool);
        completed_eid=10; sweep(384);
        check("equal watermark retains conservative next-submission margin", t.m_registered && t.m_descriptor_pool);
        completed_eid=11; sweep(448);
        check("later completion releases eligible idle pool", !t.m_registered && !t.m_descriptor_pool && !t.m_descriptor_set);
        check("release invalidates descriptor template storage cache", t.m_descriptor_template_cache_id==umax);
        current_eid=12; t.commit();
        check("released table reactivates with a pool and rewritten set", t.m_registered && t.m_descriptor_pool && t.m_descriptor_set);
        check("reactivation tracks the new event scope", t.m_last_commit_eid==12);
    }
    {
        initialize(); descriptor_table_t t; t.commit(); current_eid=25; t.commit();
        check("unchanged descriptor commit still tracks later use", t.m_last_commit_eid==25);
        completed_eid=20; sweep(320);
        check("unchanged set's later GPU use prevents release", t.m_registered && t.m_descriptor_pool);
    }
    {
        initialize(); current_eid=30; descriptor_table_t t; t.allocation_flushes=true; t.commit();
        check("allocation reentry records resulting event scope", t.m_last_commit_eid==31);
        completed_eid=31; sweep(320);
        check("allocation reentry cannot retire at its own watermark", t.m_registered && t.m_descriptor_pool);
    }
    {
        initialize(); descriptor_pool pool; pool.next_subpool(); pool.next_subpool();
        check("all queued pool resets retain their owner", pool.has_pending_resets());
        auto first=std::move(callbacks[0]); auto second=std::move(callbacks[1]); callbacks.clear();
        first(); check("one remaining reset keeps owner retained", pool.has_pending_resets() && pool.resets==1);
        check("owner remains retained while reset executes", pool.pending_during_reset);
        second(); check("only finished final reset clears retention", !pool.has_pending_resets() && pool.resets==2);
        pool.m_current_subpool_index=umax; pool.next_subpool();
        check("absent old subpool queues no reset", callbacks.empty() && !pool.has_pending_resets());
    }
    {
        initialize(); current_eid=40; descriptor_table_t t; t.commit(); t.m_descriptor_pool->next_subpool();
        completed_eid=100; sweep(320);
        check("GPU completion with delayed GC retains idle callback owner", t.m_registered && t.m_descriptor_pool);
        if (t.m_descriptor_pool) { auto callback=std::move(callbacks[0]); callbacks.clear(); callback(); }
        else callbacks.clear(); // Never execute a dangling baseline callback in a CPU fixture.
        sweep(384); check("finished delayed callback allows idle release", !t.m_registered && !t.m_descriptor_pool);
    }
    {
        initialize(); current_eid=2; completed_eid=20; descriptor_table_t t; t.commit();
        sweep(256); check("idle threshold remains strict", t.m_registered && t.m_descriptor_pool);
        sweep(319); check("non-sweep frame retains eligible pool", t.m_registered && t.m_descriptor_pool);
        sweep(320); check("eligible pool is eventually reclaimed", !t.m_registered && !t.m_descriptor_pool);
    }
    {
        initialize(); current_eid=5; descriptor_table_t old, recent; old.commit(); recent.commit();
        g_table_frame=300; current_eid=10; recent.commit(); completed_eid=20; sweep(320);
        check("registry compaction retains a recently used table", !old.m_registered && recent.m_registered && g_table_registry.size()==1 && g_table_registry[0]==&recent);
    }
    std::cout << checks << " descriptor retirement checks, " << failures << " failed\n";
    return failures ? 1 : 0;
}
'''.replace("PENDING_DECLARATION", pending_declaration).replace("PENDING_METHOD", pending_method).replace(
        "CALLBACK_BRANCH", callback_branch).replace("EID_DECLARATION", eid_declaration).replace("FUNCTIONS", functions)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--baseline", action="store_true")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    text = generate(args.source, args.baseline)
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out / "descriptor-retirement-audit.cpp"
    output.write_text(text, encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
