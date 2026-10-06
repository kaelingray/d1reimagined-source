"""Exercise actual renderer query control flow against mock render-pass scopes.

The full device-policy initializer, query-start draw block and query-end/cleanup
methods are extracted from private sources. CPU mocks reject mismatched scopes;
this is not a GPU test or a measurement of the extra render-pass closures.
"""
import argparse
from pathlib import Path

from rpcs3_present_lifetime_audit import block


def function(source, signature):
    return signature + "\n" + block(source, source.index(signature))


def generate(root):
    directory = root / "rpcs3/Emu/RSX/VK"
    helpers = (directory / "VKHelpers.cpp").read_text(encoding="utf-8")
    renderer = (directory / "VKGSRender.cpp").read_text(encoding="utf-8")
    draw = (directory / "VKDraw.cpp").read_text(encoding="utf-8")
    policy = function(helpers, "void set_current_renderer(const vk::render_device &device)")
    getter = function(helpers, "bool use_strict_query_scopes()")
    query_start = "if (m_current_command_buffer->flags & vk::command_buffer::cb_load_occluson_task)"
    start = query_start + "\n" + block(draw, draw.index(query_start, draw.index("// Faults are allowed during vertex upload")))
    methods = [function(renderer, signature) for signature in (
        "void VKGSRender::begin_occlusion_query(rsx::reports::occlusion_query_info* query)",
        "void VKGSRender::end_occlusion_query(rsx::reports::occlusion_query_info* query)",
        "void VKGSRender::emergency_query_cleanup(vk::command_buffer* commands)",
    )]
    return PREAMBLE + policy + getter + "}\n" + "\n".join(methods) + "\nvoid VKGSRender::start_query_for_draw() {\n" + start + "\n}\n" + TESTS


PREAMBLE = r'''
#include <array>
#include <cstdint>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
#include <vector>
#undef _WIN32
using u32=std::uint32_t; using u64=std::uint64_t;
constexpr u32 umax=std::numeric_limits<u32>::max();
#define ensure(value) do { if (!(value)) throw std::runtime_error("ensure"); } while(false)
#define CHECK_RESULT(value) ensure((value)==0)
struct logger {
    template<class... Args> void error(Args...){}
    template<class... Args> void warning(Args...){}
    template<class... Args> void notice(Args...){}
} rsx_log;
struct config { struct { bool strict_rendering_mode=false, relaxed_zcull_sync=false; } video; } g_cfg;
using VkFlags=u32; using VkBufferUsageFlags=u32; using VkBuffer=u64;
struct VkMemoryRequirements { u32 memoryTypeBits=1; };
struct VkBufferCreateInfo { int sType{}; u64 size{}; int sharingMode{}; VkFlags flags{},usage{}; };
constexpr u32 VK_BUFFER_USAGE_TRANSFER_SRC_BIT=1, VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT=2;
constexpr u32 VK_BUFFER_USAGE_UNIFORM_TEXEL_BUFFER_BIT=4, VK_BUFFER_USAGE_INDEX_BUFFER_BIT=8;
constexpr u32 VK_BUFFER_USAGE_STORAGE_BUFFER_BIT=16, VK_BUFFER_USAGE_VERTEX_BUFFER_BIT=32;
constexpr u32 VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT=1, VK_MEMORY_PROPERTY_HOST_COHERENT_BIT=2;
constexpr int VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO=12, VK_SHARING_MODE_EXCLUSIVE=0;
int vkCreateBuffer(u64,const VkBufferCreateInfo*,const void*,VkBuffer* buffer) { *buffer=1; return 0; }
void vkGetBufferMemoryRequirements(u64,VkBuffer,VkMemoryRequirements*){}
void vkDestroyBuffer(u64,VkBuffer,const void*){}
namespace rsx::reports { struct occlusion_query_info { u32 result=99,driver_handle=0; }; }
namespace vk {
enum class driver_vendor { AMD,RADV,NVIDIA,INTEL,ANV,MVK,LAVAPIPE,V3DV,DOZEN,HONEYKRISP,PANVK,ARM_MALI,QUALCOMM,TURNIP };
enum class chip_class { AMD_gcn_generic,other };
struct physical_device {
    driver_vendor vendor=driver_vendor::NVIDIA;
    const char* get_name() const { return "mock GPU"; }
    driver_vendor get_driver_vendor() const { return vendor; }
    chip_class get_chip_class() const { return chip_class::other; }
};
struct render_device {
    physical_device physical;
    const physical_device& gpu() const { return physical; }
    bool get_conditional_render_support() const { return true; }
    bool get_compatible_memory_type(u32,u32,void*) const { return true; }
    operator u64() const { return 1; }
};
const render_device* g_render_device=nullptr;
struct runtime_state { void clear(){} } g_runtime_state;
bool g_drv_no_primitive_restart=false, g_drv_sanitize_fp_values=false, g_drv_disable_fence_reset=false;
bool g_drv_strict_query_scopes=false, g_drv_emulate_cond_render=false;
u32 g_num_processed_frames=0,g_num_total_frames=0,g_heap_compatible_buffer_types=0;
driver_vendor g_driver_vendor{}; chip_class g_chip_class{};
namespace descriptors { void init(){} }
struct command_buffer {
    static constexpr u32 cb_load_occluson_task=1,cb_has_occlusion_task=2,cb_has_open_query=4;
    u32 flags=0,pass=0;
};
int closures=0;
bool is_renderpass_open(const command_buffer& cmd) { return cmd.pass!=0; }
void end_renderpass(command_buffer& cmd) { if (cmd.pass) ++closures; cmd.pass=0; }
}
struct query_data {
    std::vector<u32> indices;
    vk::command_buffer* sync=nullptr;
    void set_sync_command_buffer(vk::command_buffer* value) { sync=value; }
};
struct query_manager {
    u32 next=0;
    int begins=0,ends=0,mismatches=0;
    std::array<u32,32> scopes{};
    std::array<bool,32> active{};
    u32 allocate_query(vk::command_buffer&) { return next++; }
    void begin_query(vk::command_buffer& cmd,u32 index) {
        ensure(!active.at(index)); active.at(index)=true; scopes.at(index)=cmd.pass; ++begins;
    }
    void end_query(vk::command_buffer& cmd,u32 index) {
        ensure(active.at(index));
        if (scopes.at(index)!=cmd.pass) ++mismatches;
        active.at(index)=false; ++ends;
    }
};
struct VKGSRender {
    vk::command_buffer buffer;
    vk::command_buffer* m_current_command_buffer=&buffer;
    query_manager manager;
    query_manager* m_occlusion_query_manager=&manager;
    std::array<query_data,2> m_occlusion_map;
    rsx::reports::occlusion_query_info* m_active_query_info=nullptr;
    rsx::reports::occlusion_query_info* m_current_task=nullptr;
    bool m_occlusion_query_active=false;
    void begin_occlusion_query(rsx::reports::occlusion_query_info*);
    void end_occlusion_query(rsx::reports::occlusion_query_info*);
    void emergency_query_cleanup(vk::command_buffer*);
    void start_query_for_draw();
};
namespace ZCULL_control { void sync(VKGSRender*){} }
namespace vk {
'''

TESTS = r'''
int checks=0,failures=0;
void check(const std::string& name,bool pass) {
    ++checks; if(!pass) ++failures;
    std::cout<<(pass?"PASS ":"FAIL ")<<name<<"\n";
}
int main() {
    vk::render_device device;
    for(const auto vendor:{vk::driver_vendor::NVIDIA,vk::driver_vendor::AMD,vk::driver_vendor::INTEL,vk::driver_vendor::TURNIP}) {
        for(const bool strict:{false,true}) {
            device.physical.vendor=vendor; g_cfg.video.strict_rendering_mode=strict;
            vk::set_current_renderer(device);
            const std::string suffix=" driver="+std::to_string(static_cast<int>(vendor))+" strict="+std::to_string(strict);
            VKGSRender renderer; rsx::reports::occlusion_query_info query;
            renderer.begin_occlusion_query(&query);
            renderer.start_query_for_draw();
            renderer.buffer.pass=7; // Draw inside a render pass after outside begin.
            renderer.end_occlusion_query(&query);
            check("outside query ends in its matching scope"+suffix,renderer.manager.begins==1 && renderer.manager.ends==1 && renderer.manager.mismatches==0);
            check("query completion preserves results and clears flags"+suffix,query.result==0 && renderer.m_occlusion_map[0].indices.size()==1 && !renderer.m_occlusion_query_active && !renderer.m_active_query_info && !(renderer.buffer.flags & (vk::command_buffer::cb_load_occluson_task|vk::command_buffer::cb_has_open_query)));
            VKGSRender existing_pass; query={};
            existing_pass.buffer.pass=3;
            existing_pass.begin_occlusion_query(&query);
            existing_pass.start_query_for_draw();
            vk::end_renderpass(existing_pass.buffer); // Attachment change between draws.
            existing_pass.buffer.pass=4;
            existing_pass.end_occlusion_query(&query);
            check("query spanning attachment changes has matching scope"+suffix,existing_pass.manager.scopes[0]==0 && existing_pass.manager.mismatches==0 && existing_pass.manager.ends==1);
        }
    }
    g_cfg.video.strict_rendering_mode=false; device.physical.vendor=vk::driver_vendor::NVIDIA;
    vk::set_current_renderer(device);
    VKGSRender empty; rsx::reports::occlusion_query_info query;
    empty.buffer.pass=8; vk::closures=0;
    empty.begin_occlusion_query(&query); empty.end_occlusion_query(&query);
    check("query without draws adds no query or render-pass commands",empty.manager.begins==0 && empty.manager.ends==0 && vk::closures==0 && empty.buffer.pass==8 && empty.buffer.flags==0);
    VKGSRender split; query={};
    split.begin_occlusion_query(&query); split.start_query_for_draw(); split.buffer.pass=1;
    vk::end_renderpass(split.buffer); split.emergency_query_cleanup(&split.buffer);
    check("flush cleanup ends query and preserves the logical task",split.manager.ends==1 && split.manager.mismatches==0 && split.m_occlusion_query_active && split.m_active_query_info==&query && !(split.buffer.flags & vk::command_buffer::cb_has_open_query));
    split.buffer.flags |= vk::command_buffer::cb_load_occluson_task;
    split.buffer.pass=2; split.start_query_for_draw(); split.buffer.pass=3; split.end_occlusion_query(&query);
    check("rearmed query retains both indices and matching end scopes",split.manager.begins==2 && split.manager.ends==2 && !split.manager.mismatches && split.m_occlusion_map[0].indices==std::vector<u32>({0,1}) && split.m_occlusion_map[0].sync==&split.buffer);
    VKGSRender untouched; untouched.buffer.pass=9;
    untouched.emergency_query_cleanup(&untouched.buffer);
    check("cleanup without an open query leaves render pass untouched",untouched.manager.ends==0 && untouched.buffer.pass==9);
    VKGSRender protected_query; query={}; rsx::reports::occlusion_query_info wrong;
    protected_query.begin_occlusion_query(&query); bool refused=false;
    try { protected_query.end_occlusion_query(&wrong); } catch(const std::runtime_error&) { refused=true; }
    check("wrong logical query cannot alter active state",refused && protected_query.m_active_query_info==&query && protected_query.m_occlusion_query_active && protected_query.manager.ends==0);
    std::cout<<checks<<" query scope checks, "<<failures<<" failed\n";
    return failures?1:0;
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out / "query-scope-audit.cpp"
    output.write_text(generate(args.source), encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
