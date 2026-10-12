"""Extract opt-in checkpoint registry/query code into bounded C++ API fixtures.

These tests validate diagnostics and opaque marker handling, not GPU execution or
the originating renderer fault. The private native checkout is supplied explicitly.
"""
import argparse
from pathlib import Path


def function(source, marker):
    start = source.index(marker)
    opening = source.index("{", start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == "{") - (source[index] == "}")
        if not depth:
            return source[start:index + 1]
    raise ValueError("Unterminated native function")


def generate(source):
    root = source / "rpcs3/Emu/RSX/VK"
    device = (root / "vkutils/device.cpp").read_text(encoding="utf-8")
    header = (root / "vkutils/device.h").read_text(encoding="utf-8")
    commands = (root / "vkutils/commands.cpp").read_text(encoding="utf-8")
    shared = (root / "vkutils/shared.cpp").read_text(encoding="utf-8")
    lost = shared.split("case VK_ERROR_DEVICE_LOST:", 1)[1].split("case ", 1)[0]
    if "retrieve_checkpoint_info()" not in lost or shared.count("retrieve_checkpoint_info()") != 1:
        raise ValueError("Checkpoint queries must occur only in the device-loss branch")
    registry = device[device.index("struct diagnostic_checkpoint_record"):
                      device.index("void render_device::record_checkpoint")]
    context = function(header, "struct diagnostic_readback_info") + ";\n"
    context += function(header, "struct diagnostic_checkpoint_context") + ";\n"
    native = registry + "\n" + function(device, "void render_device::record_checkpoint")
    native += "\n" + function(device, "std::string render_device::retrieve_checkpoint_info")
    native += "\n" + function(commands, "void command_buffer::clear_state_cache")
    native += "\n" + function(commands, "void command_buffer::checkpoint")
    native += "\n" + function(commands, "void command_buffer::checkpoint_readback")
    getter = function(header, "bool get_diagnostic_checkpoint_support() const")
    return FIXTURE.replace("ACTUAL_CONTEXT", context).replace("ACTUAL_GETTER", getter).replace("ACTUAL_NATIVE", native)


FIXTURE = r'''
#include <algorithm>
#include <array>
#include <bit>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>
using u64=unsigned long long; using u32=unsigned int; using usz=unsigned int; using uptr=std::uintptr_t;
using VkQueue=u64; using VkCommandBuffer=u64; using VkPipeline=u64; using VkPipelineBindPoint=int;
constexpr u64 VK_NULL_HANDLE=0; constexpr int VK_PIPELINE_BIND_POINT_GRAPHICS=0, VK_PIPELINE_BIND_POINT_COMPUTE=1;
constexpr u32 VK_STRUCTURE_TYPE_CHECKPOINT_DATA_NV=1000206000;
struct VkCheckpointDataNV {u32 sType=0; void* pNext=nullptr; u32 stage=0; void* pCheckpointMarker=nullptr;};
using SetCheckpoint=void(*)(VkCommandBuffer,const void*);
using GetCheckpoints=void(*)(VkQueue,u32*,VkCheckpointDataNV*);
#define ensure(condition) do { if (!(condition)) throw std::runtime_error("invalid checkpoint call"); } while (0)
namespace fmt {
template<class... T> void append(std::string& out,const char* format,T... args) {
    int n=std::snprintf(nullptr,0,format,args...); std::vector<char> buf(n+1);
    std::snprintf(buf.data(),buf.size(),format,args...); out.append(buf.data(),n);
}}
namespace vk {
ACTUAL_CONTEXT
struct render_device {
    VkQueue m_graphics_queue=1, m_transfer_queue=1;
    SetCheckpoint m_set_checkpoint=nullptr; GetCheckpoints m_get_checkpoints=nullptr;
    ACTUAL_GETTER
    void record_checkpoint(VkCommandBuffer,const char*,u64,const diagnostic_checkpoint_context& = {}) const;
    std::string retrieve_checkpoint_info() const;
};
struct command_pool {render_device* device; render_device& get_owner() const {return *device;}};
struct command_buffer {
    bool is_open=true; command_pool* pool; VkCommandBuffer commands=10;
    std::array<VkPipeline,2> m_bound_pipelines{0x111,0x222};
    std::array<int,2> m_bound_descriptor_sets{};
    mutable diagnostic_checkpoint_context m_checkpoint_context{};
    void clear_state_cache();
    void checkpoint(const char*,VkPipelineBindPoint=0) const;
    void checkpoint_readback(const char*,const diagnostic_readback_info&) const;
};
ACTUAL_NATIVE
}
u64 last_marker=0; int set_calls=0, query_calls=0, fill_calls=0;
u32 available=1, returned=1, supplied_capacity=0;
bool initialized=true;
std::array<u64,64> markers{};
void set(VkCommandBuffer,const void* marker) {++set_calls;last_marker=reinterpret_cast<uptr>(marker);}
void query(VkQueue,u32* count,VkCheckpointDataNV* data) {
    ++query_calls;
    if (!data) {*count=available;return;}
    ++fill_calls; supplied_capacity=*count;
    for(u32 i=0;i<*count;++i) initialized&=data[i].sType==VK_STRUCTURE_TYPE_CHECKPOINT_DATA_NV && !data[i].pNext;
    for(u32 i=0;i<std::min(*count,returned);++i) {data[i].stage=0x2000;data[i].pCheckpointMarker=reinterpret_cast<void*>(static_cast<uptr>(markers[i]));}
    *count=returned;
}
void reset_query(u32 count=1,u32 written=1) {available=count;returned=written;query_calls=fill_calls=0;initialized=true;markers.fill(0);}
int checks=0,failures=0;
void check(const char* name,bool pass) {++checks;if(!pass){++failures;std::cerr<<"FAIL "<<name<<"\n";}}
int main() {
    vk::render_device dev; vk::command_pool pool{&dev}; vk::command_buffer cmd{true,&pool};
    cmd.checkpoint("disabled");check("disabled path records no GPU marker",set_calls==0);
    check("disabled path queries no queue",dev.retrieve_checkpoint_info().empty() && query_calls==0);
    dev.m_set_checkpoint=set;check("one missing procedure disables diagnostics",!dev.get_diagnostic_checkpoint_support());
    dev.m_get_checkpoints=query;
    cmd.checkpoint("graphics.before");const auto graphics=last_marker;markers[0]=graphics;
    auto report=dev.retrieve_checkpoint_info();
    check("graphics marker preserves operation and bound pipeline",report.find("operation=graphics.before")!=std::string::npos && report.find("pipeline=0x111")!=std::string::npos);
    check("equal graphics/transfer queue is queried once",query_calls==2 && fill_calls==1);
    check("every output structure is initialized",initialized);
    cmd.checkpoint("compute.before",VK_PIPELINE_BIND_POINT_COMPUTE);reset_query();markers[0]=last_marker;
    report=dev.retrieve_checkpoint_info();
    check("compute uses its independent actual bound pipeline",report.find("pipeline=0x222")!=std::string::npos);
    cmd.checkpoint("graphics-draw.before");cmd.checkpoint("graphics-draw.after");
    cmd.checkpoint("compute-dispatch.before",1);cmd.checkpoint("compute-dispatch.after",1);
    vk::diagnostic_readback_info readback{0x444,0,0,256,0,8,8,32,44,3};
    cmd.checkpoint_readback("dma-readback.before",readback);const auto before=last_marker;
    readback.destination_buffer=0x555;readback.destination_offset=64;readback.destination_capacity=512;
    cmd.checkpoint_readback("dma-readback.mapped",readback);const auto mapped=last_marker;
    cmd.checkpoint_readback("dma-readback.after",readback);
    cmd.m_bound_pipelines[0]=0;cmd.checkpoint("command-buffer.end");const auto end=last_marker;
    reset_query();markers[0]=end;report=dev.retrieve_checkpoint_info();
    check("buffer end retains compute binding even with no graphics pipeline",report.find("graphics_pipeline=0x0 compute_pipeline=0x222")!=std::string::npos);
    check("command region counts before operations once",report.find("graphics_batches=1 compute_dispatches=1 readbacks=1")!=std::string::npos);
    check("buffer end retains mapped DMA range and capacity",report.find("readback_image=0x444 readback_buffer=0x555 readback_offset=64 readback_length=256 readback_capacity=512")!=std::string::npos);
    check("DMA format dimensions and transformations survive buffer end",report.find("readback_width=8 readback_height=8 readback_pitch=32 readback_format=44 readback_transforms=3")!=std::string::npos);
    reset_query();markers[0]=before;report=dev.retrieve_checkpoint_info();
    check("beginning DMA snapshot remains explicitly unmapped",report.find("readback_buffer=0x0 readback_offset=0 readback_length=256 readback_capacity=0")!=std::string::npos);
    readback.destination_buffer=0x666;cmd.m_checkpoint_context.readback=readback;
    reset_query();markers[0]=mapped;
    check("mapped snapshot does not alias later CPU context",dev.retrieve_checkpoint_info().find("readback_buffer=0x555")!=std::string::npos);
    cmd.clear_state_cache();cmd.checkpoint("command-buffer.begin");reset_query();markers[0]=last_marker;
    report=dev.retrieve_checkpoint_info();
    check("new recording clears both pipelines and all diagnostic counts",report.find("graphics_pipeline=0x0 compute_pipeline=0x0 graphics_batches=0 compute_dispatches=0 readbacks=0")!=std::string::npos);
    check("new recording cannot inherit previous readback resources",report.find("readback_image=")==std::string::npos);
    check("new recording clears the stored DMA details",cmd.m_checkpoint_context.readback.source_image==0 && cmd.m_checkpoint_context.readback.destination_buffer==0);
    dev.m_get_checkpoints=nullptr;const auto calls_before=set_calls;
    cmd.checkpoint_readback("dma-readback.before",readback);
    check("disabled readback path neither emits markers nor changes context",set_calls==calls_before && cmd.m_checkpoint_context.readbacks==0 && cmd.m_checkpoint_context.readback.source_image==0);
    dev.m_get_checkpoints=query;
    char name[]="original.operation";dev.record_checkpoint(10,name,0x333);reset_query();markers[0]=last_marker;name[0]='X';
    check("operation name survives caller storage mutation",dev.retrieve_checkpoint_info().find("operation=original.operation")!=std::string::npos);
    dev.record_checkpoint(10,nullptr,0);reset_query();markers[0]=last_marker;
    check("missing operation has a stable label",dev.retrieve_checkpoint_info().find("operation=unknown")!=std::string::npos);
    reset_query(0,0);report=dev.retrieve_checkpoint_info();
    check("zero-count query never calls fill",query_calls==1 && fill_calls==0 && report.find("records=0")!=std::string::npos);
    reset_query(100000,100000);report=dev.retrieve_checkpoint_info();
    check("large driver counts are bounded on allocation and output",supplied_capacity==64 && initialized && report.find("records=64 capacity=64")!=std::string::npos);
    reset_query(2,1);markers[0]=graphics;markers[1]=0xffffffffffffffffull;
    report=dev.retrieve_checkpoint_info();
    check("returned count shrink ignores unwritten entries",report.find("records=1 capacity=2")!=std::string::npos && report.find("expired-or-unknown")==std::string::npos);
    reset_query();markers[0]=0xffffffffffffffffull;
    check("unknown opaque marker is never dereferenced",dev.retrieve_checkpoint_info().find("expired-or-unknown")!=std::string::npos);
    for(u32 i=0;i<65536;++i)dev.record_checkpoint(10,"later",0);
    reset_query();markers[0]=graphics;
    check("expired registry entry is not misattributed",dev.retrieve_checkpoint_info().find("expired-or-unknown")!=std::string::npos);
    dev.m_transfer_queue=2;reset_query();markers[0]=last_marker;dev.retrieve_checkpoint_info();
    check("distinct execution queues are both queried",query_calls==4 && fill_calls==2);
    dev.m_transfer_queue=0;reset_query();dev.retrieve_checkpoint_info();
    check("null queue is excluded",query_calls==2);
    bool refused=false;try{cmd.checkpoint("invalid",2);}catch(const std::runtime_error&){refused=true;}
    check("unsupported pipeline bind point is refused",refused);
    refused=false;try{cmd.checkpoint("negative",-1);}catch(const std::runtime_error&){refused=true;}
    check("negative bind point never indexes outside pipeline state",refused);
    refused=false;cmd.is_open=false;try{cmd.checkpoint("closed");}catch(const std::runtime_error&){refused=true;}
    check("closed command buffer is refused",refused);
    refused=false;try{cmd.checkpoint_readback("dma-readback.before",readback);}catch(const std::runtime_error&){refused=true;}
    check("closed readback cannot mutate diagnostic context",refused && cmd.m_checkpoint_context.readbacks==0);
    std::cout<<checks<<" checkpoint checks, "<<failures<<" failed\n";return failures?1:0;
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out / "gpu-checkpoint-audit.cpp"
    output.write_text(generate(args.source), encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
