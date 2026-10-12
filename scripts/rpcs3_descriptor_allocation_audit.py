"""Extract the actual private descriptor allocator into CPU pool-generation fixtures.

Fake Vulkan allocations retain their pool, layout and reset generation. The tests
exercise allocation/cache/retirement ordering, not GPU execution or rendered pixels.
"""
import argparse
from pathlib import Path

from rpcs3_present_lifetime_audit import block


def generate(source: Path) -> str:
    header = (source / "descriptors.h").read_text(encoding="utf-8")
    cpp = (source / "descriptors.cpp").read_text(encoding="utf-8")
    declaration = "class descriptor_pool " + block(header, header.index("class descriptor_pool")) + ";"
    # Expose state to inspect actual allocation provenance; method bodies are unchanged.
    declaration = declaration.replace("private:", "public:")
    methods = []
    for signature in (
        "u32 descriptor_pool::autoscaling_config_t::get_pool_size()",
        "void descriptor_pool::create(",
        "void descriptor_pool::destroy()",
        "void descriptor_pool::reset(",
        "VkDescriptorSet descriptor_pool::allocate(",
        "void descriptor_pool::next_subpool()",
        "std::pair<VkResult, VkDescriptorPool> descriptor_pool::new_subpool()",
    ):
        start = cpp.index(signature)
        opening = cpp.index("{", start)
        methods.append(cpp[start:opening] + block(cpp, opening))
    return r'''
#include <algorithm>
#include <atomic>
#include <bit>
#include <cstdint>
#include <functional>
#include <iostream>
#include <limits>
#include <map>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <utility>
#include <vector>
using u32=std::uint32_t; using u64=std::uint64_t;
using VkDescriptorPool=u64; using VkDescriptorSet=u64; using VkDescriptorSetLayout=u64;
using VkBool32=u32; using VkDescriptorPoolResetFlags=u32; using VkResult=int;
constexpr u32 umax=std::numeric_limits<u32>::max();
constexpr u64 VK_NULL_HANDLE=0;
constexpr u32 VK_TRUE=1, VK_FALSE=0, VK_DESCRIPTOR_POOL_CREATE_UPDATE_AFTER_BIND_BIT=1;
constexpr u32 VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO=1, VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO=2;
constexpr int VK_SUCCESS=0, VK_ERROR_FRAGMENTATION_EXT=-1;
#define FORCE_INLINE inline
template<class T> using atomic_t=std::atomic<T>;
using shared_mutex=std::mutex; using reader_lock=std::lock_guard<std::mutex>;
void ensure(bool ok) { if (!ok) throw std::runtime_error("assertion"); }
#define CHECK_RESULT(r) ensure((r)==VK_SUCCESS)
struct logger { template<class... T> void notice(const char*,T...) {} template<class... T> void warning(const char*,T...) {} } d1_log,rsx_log;
namespace d1diag {
enum class descriptor_event_kind { reset, retire };
void descriptor_record(descriptor_event_kind, std::initializer_list<u64>) {}
bool descriptor_logging() { return false; }
}
namespace fmt { void throw_exception(const char*) { throw std::runtime_error("unreachable"); } }
struct VkDescriptorPoolSize { u32 type=0,descriptorCount=0; };
struct VkDescriptorPoolCreateInfo { u32 sType=0; const void* pNext=nullptr; u32 flags=0,maxSets=0,poolSizeCount=0; const VkDescriptorPoolSize* pPoolSizes=nullptr; };
struct VkDescriptorSetAllocateInfo { u32 sType=0; const void* pNext=nullptr; VkDescriptorPool descriptorPool=0; u32 descriptorSetCount=0; const VkDescriptorSetLayout* pSetLayouts=nullptr; };
namespace rsx {
template<class T> struct simple_array: std::vector<T> {
    using std::vector<T>::vector;
    T pop_back() { T item=this->back(); std::vector<T>::pop_back(); return item; }
    template<class F> auto map(F f) const {
        simple_array<decltype(f(std::declval<T>()))> result;
        for (const auto& item:*this) result.push_back(f(item));
        return result;
    }
};
}
struct pool_state { u32 size=0,used=0,generation=0; bool destroyed=false; };
struct set_state { VkDescriptorPool pool=0; VkDescriptorSetLayout layout=0; u32 generation=0; };
std::map<VkDescriptorPool,pool_state> pools;
std::map<VkDescriptorSet,set_state> sets;
u64 next_pool=1,next_set=10000; unsigned allocation_calls=0;
VkResult vkCreateDescriptorPool(u64,const VkDescriptorPoolCreateInfo* info,const void*,VkDescriptorPool* pool) {
    *pool=next_pool++; pools[*pool]={info->maxSets}; return VK_SUCCESS;
}
void vkDestroyDescriptorPool(u64,VkDescriptorPool pool,const void*) { pools.at(pool).destroyed=true; }
VkResult vkResetDescriptorPool(u64,VkDescriptorPool pool,u32) { auto& p=pools.at(pool); p.used=0; ++p.generation; return VK_SUCCESS; }
VkResult vkAllocateDescriptorSets(u64,const VkDescriptorSetAllocateInfo* info,VkDescriptorSet* result) {
    auto& p=pools.at(info->descriptorPool); ensure(!p.destroyed && p.used+info->descriptorSetCount<=p.size);
    ++allocation_calls; p.used+=info->descriptorSetCount;
    for (u32 i=0;i<info->descriptorSetCount;++i) { result[i]=next_set++; sets[result[i]]={info->descriptorPool,info->pSetLayouts[i],p.generation}; }
    return VK_SUCCESS;
}
bool valid(VkDescriptorSet set) { const auto& s=sets.at(set); const auto& p=pools.at(s.pool); return !p.destroyed && s.generation==p.generation; }
namespace vk {
struct render_device { operator u64() const { return 1; } u64 get_descriptor_update_after_bind_support() const { return 0; } } device;
u64 current_event_id() { return 1; } u64 last_completed_event_id() { return 0; }
void on_descriptor_pool_fragmentation(bool) {} void die_with_error(int) { throw std::runtime_error("Vulkan error"); }
struct gc_callback_t { std::function<void()> callback; explicit gc_callback_t(std::function<void()> f):callback(std::move(f)) {} ~gc_callback_t() { callback(); } };
struct collector {
    std::vector<std::unique_ptr<gc_callback_t>> pending;
    void dispose(std::unique_ptr<gc_callback_t>& object) { pending.push_back(std::move(object)); }
    void complete() { pending.clear(); }
} gc;
collector* get_gc() { return &gc; }
DECLARATION
METHODS
}
int main() {
    int checks=0,failures=0;
    auto check=[&](const char* name,bool ok) { ++checks; if(!ok) { ++failures; std::cout<<"FAIL "<<name<<'\n'; } };
    auto finish=[](vk::descriptor_pool& p) { vk::gc.complete(); p.destroy(); };
    const rsx::simple_array<VkDescriptorPoolSize> sizes{{0,1}};
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,128,128);
        const auto first=p.allocate(10), pool=sets.at(first).pool;
        const auto calls=allocation_calls;
        const auto second=p.allocate(10);
        check("same-layout cache avoids another Vulkan allocation",allocation_calls==calls && first!=second);
        check("cached set retains current pool and layout",sets.at(second).pool==pool && sets.at(second).layout==10 && valid(second));
        const auto other=p.allocate(20);
        check("other layout gets its requested layout",sets.at(other).layout==20 && valid(other));
        const auto cached=p.allocate(10);
        check("mixed layout inside one pool preserves valid cached entries",sets.at(cached).pool==pool && sets.at(cached).layout==10 && valid(cached));
        finish(p);
    }
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,128,128);
        const auto first=p.allocate(10), old=sets.at(first).pool;
        for(unsigned i=0;i<65;++i) p.allocate(20);
        check("mixed-layout exhaustion switches pools",p.m_current_pool_handle!=old && p.has_pending_resets());
        const auto fresh=p.allocate(10);
        check("cached handles cannot escape a retired pool",sets.at(fresh).pool==p.m_current_pool_handle);
        check("post-switch cached allocation has requested layout",sets.at(fresh).layout==10);
        vk::gc.complete();
        check("old pool reset cannot invalidate post-switch allocation",valid(fresh));
        check("retired source allocation is invalidated after reset",!valid(first));
        const auto reused=p.allocate(10);
        check("new pool cache remains valid after old asynchronous reset",valid(reused) && sets.at(reused).pool==p.m_current_pool_handle);
        finish(p);
    }
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,128,128);
        const auto first=p.allocate(10), old=sets.at(first).pool;
        for(unsigned i=0;i<65;++i) p.allocate(10,VK_FALSE);
        check("uncached exhaustion switches pools",p.m_current_pool_handle!=old);
        const auto fresh=p.allocate(10);
        check("uncached spill also drops old cached handles",sets.at(fresh).pool==p.m_current_pool_handle);
        vk::gc.complete();
        check("uncached spill allocation survives old reset",valid(fresh));
        finish(p);
    }
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,128,128);
        const auto first=p.allocate(10), old=sets.at(first).pool;
        for(unsigned i=0;i<128;++i) p.allocate(10);
        const auto fresh=p.allocate(10);
        check("single-layout rollover allocates valid current-pool sets",p.m_current_pool_handle!=old && valid(fresh) && sets.at(fresh).pool==p.m_current_pool_handle);
        vk::gc.complete();
        check("single-layout rollover survives old reset",valid(fresh));
        finish(p);
    }
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,128,128);
        p.allocate(10); const auto old=p.m_current_pool_handle;
        for(unsigned i=0;i<65;++i) p.allocate(20);
        vk::gc.complete();
        for(unsigned i=0;i<128;++i) p.allocate(20,VK_FALSE);
        const auto fresh=p.allocate(30);
        check("reclaimed pools remain usable after rollover",valid(fresh) && sets.at(fresh).pool==p.m_current_pool_handle);
        check("reclaimed pool is selected with reset capacity",p.m_current_pool_handle==old && pools.at(old).generation==1);
        check("new cache after reuse uses requested layout",sets.at(fresh).layout==30);
        finish(p);
    }
    {
        vk::descriptor_pool p; p.create(vk::device,sizes,2,64);
        bool correct=true;
        for(unsigned i=0;i<150;++i) { const auto set=p.allocate(10); correct &= valid(set) && sets.at(set).layout==10 && sets.at(set).pool==p.m_current_pool_handle; }
        check("autoscaling preserves allocation provenance",correct);
        check("autoscaling grows within configured bounds",p.m_autoscaling_config.current_size>=4 && p.m_autoscaling_config.current_size<=64);
        finish(p);
    }
    std::cout<<checks<<" descriptor allocation checks, "<<failures<<" failed\n";
    return failures ? 1 : 0;
}
'''.replace("DECLARATION", declaration).replace("METHODS", "\n".join(methods))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path, help="Directory containing actual descriptors.cpp/.h")
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    text = generate(args.source)
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out / "descriptor-allocation-audit.cpp"
    output.write_text(text, encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
