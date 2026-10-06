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
    native = registry + "\n" + function(device, "void render_device::record_checkpoint")
    native += "\n" + function(device, "std::string render_device::retrieve_checkpoint_info")
    native += "\n" + function(commands, "void command_buffer::checkpoint")
    getter = function(header, "bool get_diagnostic_checkpoint_support() const")
    return FIXTURE.replace("ACTUAL_GETTER", getter).replace("ACTUAL_NATIVE", native)


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
struct render_device {
    VkQueue m_graphics_queue=1, m_transfer_queue=1;
    SetCheckpoint m_set_checkpoint=nullptr; GetCheckpoints m_get_checkpoints=nullptr;
    ACTUAL_GETTER
    void record_checkpoint(VkCommandBuffer,const char*,u64) const;
    std::string retrieve_checkpoint_info() const;
};
struct command_pool {render_device* device; render_device& get_owner() const {return *device;}};
struct command_buffer {
    bool is_open=true; command_pool* pool; VkCommandBuffer commands=10;
    std::array<VkPipeline,2> m_bound_pipelines{0x111,0x222};
    void checkpoint(const char*,VkPipelineBindPoint=0) const;
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
