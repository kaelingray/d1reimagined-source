"""Exercise actual command-buffer waits and cleanup callers with mock fence results.

The command-buffer chunk, full frame cleanup and DMA submission/wait/restart block
are extracted from private sources. Timeout/error injection checks pending state,
resource retention and subsequent completion. CPU mocks do not prove GPU fidelity.
"""
import argparse
from pathlib import Path

from rpcs3_present_lifetime_audit import block


def generate(root):
    directory = root / "rpcs3/Emu/RSX/VK"
    header = (directory / "VKGSRenderTypes.hpp").read_text(encoding="utf-8")
    present = (directory / "VKPresent.cpp").read_text(encoding="utf-8")
    texture = (directory / "VKTextureCache.cpp").read_text(encoding="utf-8")
    chunk = "struct command_buffer_chunk : public vk::command_buffer\n" + block(
        header, header.index("struct command_buffer_chunk")) + ";\n"
    signature = "void VKGSRender::frame_context_cleanup(vk::frame_context_t *ctx)"
    cleanup = signature + "\n" + block(present, present.index(signature))
    dma_start = texture.index("// Primary access command queue, must restart it after",
                              texture.index("void texture_cache::cleanup_after_dma_transfers"))
    fence_start = texture.rfind("\n", dma_start, texture.index("submit_fence", dma_start)) + 1
    declarations = texture[fence_start:texture.index("if (async_sema)", fence_start)]
    restart_start = texture.index("cmd.submit(submit_info, VK_TRUE);", fence_start)
    restart_end = texture.index("cmd.begin();", restart_start) + len("cmd.begin();")
    restart = texture[restart_start:restart_end]
    return (PREAMBLE + chunk + FRAME_MOCKS + cleanup
            + "\nvoid restart_dma(vk::command_buffer& cmd) {\n"
            + "int device=1; const int* m_device=&device; int m_submit_queue=1;\n"
            + declarations + restart + "\n}\n" + TESTS)


PREAMBLE = r'''
#include <cstdint>
#include <deque>
#include <functional>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>
using u32=std::uint32_t; using u64=std::uint64_t; using VkResult=int;
constexpr VkResult VK_SUCCESS=0, VK_NOT_READY=1, VK_TIMEOUT=2;
constexpr VkResult VK_ERROR_OUT_OF_HOST_MEMORY=-1, VK_ERROR_OUT_OF_DEVICE_MEMORY=-2;
constexpr VkResult VK_ERROR_DEVICE_LOST=-4, VK_ERROR_UNKNOWN=-13;
constexpr VkResult VK_ERROR_VALIDATION_FAILED=-1000011001;
constexpr int VK_TRUE=1;
constexpr u64 FRAME_PRESENT_TIMEOUT=10000000ull, GENERAL_WAIT_TIMEOUT=2000000ull;
struct api_error { VkResult result; };
#define CHECK_RESULT(expr) do { const VkResult result_=(expr); if(result_!=VK_SUCCESS) throw api_error{result_}; } while(false)
#define ensure(expr) do { if(!(expr)) throw std::runtime_error("ensure"); } while(false)
struct logger { template<class... T> void warning(const char*,T...){}; } rsx_log;
struct shared_mutex { int readers=0; bool writer=false; std::function<void()> on_upgrade; };
struct reader_lock {
    shared_mutex& guard; bool exclusive=false;
    explicit reader_lock(shared_mutex& value):guard(value){++guard.readers;}
    void upgrade(){--guard.readers; if(guard.on_upgrade)guard.on_upgrade(); guard.writer=true;exclusive=true;}
    ~reader_lock(){if(exclusive)guard.writer=false;else --guard.readers;}
};
std::deque<VkResult> results;
std::vector<u64> wait_timeouts, completed;
int fence_resets=0, buffer_resets=0, buffer_begins=0, unsafe_resets=0;
int unsafe_fence_destruction=0, trims=0, globals_reset=0, heap_restores=0, frame_advances=0;
VkResult poke_status=VK_NOT_READY;
namespace vk {
struct fence {
    bool flushed=true, gpu_pending=false, track_destruction=false; int handle=1;
    fence()=default;
    explicit fence(int):track_destruction(true){}
    ~fence(){if(track_destruction && gpu_pending)++unsafe_fence_destruction;}
    void reset(){++fence_resets;if(gpu_pending)++unsafe_resets;flushed=false;}
    void wait_flush(){}
};
struct command_pool { int get_owner()const{return 1;} };
struct queue_submit_t { int queue; fence* pfence; };
struct command_buffer {
    bool is_pending=false;
    fence own_fence; fence* m_submit_fence=&own_fence; fence* external_fence=nullptr;
    command_pool own_pool; command_pool* pool=&own_pool;
    void reset(){++buffer_resets;if(is_pending || (external_fence && external_fence->gpu_pending))++unsafe_resets;}
    void begin(){++buffer_begins;}
    void submit(queue_submit_t& info,int){external_fence=info.pfence;info.pfence->gpu_pending=true;}
};
struct gc_fixture {
    std::vector<std::unique_ptr<fence>> held;
    void dispose(std::unique_ptr<fence>& value){held.push_back(std::move(value));}
    void clear(){for(auto& value:held)value->gpu_pending=false;held.clear();}
} gc;
gc_fixture* get_gc(){return &gc;}
VkResult wait_for_fence(fence* value,u64 timeout=0){
    wait_timeouts.push_back(timeout);
    VkResult result=results.empty()?VK_SUCCESS:results.front();
    if(!results.empty())results.pop_front();
    if(result==VK_SUCCESS)value->gpu_pending=false;
    // The real helper reports errors itself on the infinite polling path.
    if(!timeout && result!=VK_SUCCESS)throw api_error{result};
    return result;
}
u64 get_event_id(){return 42;}
void on_event_completed(u64 value){completed.push_back(value);}
}
VkResult vkGetFenceStatus(int,int){return poke_status;}
namespace vk {
'''


FRAME_MOCKS = r'''
struct frame_context_t { command_buffer_chunk* swap_command_buffer=nullptr; u64 last_frame_sync_time=10; int heap_snapshot=1; };
struct resource_manager { void trim(){++trims;} } resources;
resource_manager* get_resource_manager(){return &resources;}
void reset_global_resources(){++globals_reset;}
void advance_completed_frame_counter(){++frame_advances;}
struct data_heap_manager { static void restore_snapshot(int){++heap_restores;} };
struct ui_overlay_renderer { void remove_temp_resources(u32){} };
template<class T> T* get_overlay_pass(){static T value;return &value;}
}
struct overlay_view { u32 uid=1; };
struct overlay_manager {
    std::vector<overlay_view*> dirty;
    bool has_dirty(){return !dirty.empty();}
    void lock_shared(){} void unlock_shared(){}
    auto& get_dirty(){return dirty;}
    void dispose(const std::vector<u32>&){}
};
struct VKGSRender {
    bool swapchain_unavailable=false;
    overlay_manager* m_overlay_manager=nullptr;
    u64 m_last_heap_sync_time=0;
    std::deque<vk::frame_context_t*> m_queued_frames;
    void frame_context_cleanup(vk::frame_context_t *ctx);
};
'''


TESTS = r'''
int checks=0,failures=0;
void check(const std::string& name,bool pass){++checks;if(!pass)++failures;std::cout<<(pass?"PASS ":"FAIL ")<<name<<"\n";}
void reset_test(){
    vk::gc.clear();results.clear();wait_timeouts.clear();completed.clear();
    fence_resets=buffer_resets=buffer_begins=unsafe_resets=unsafe_fence_destruction=0;
    trims=globals_reset=heap_restores=frame_advances=0;poke_status=VK_NOT_READY;
}
void pending(vk::command_buffer_chunk& cmd){cmd.is_pending=true;cmd.eid_tag=41;cmd.reset_id=7;cmd.own_fence.gpu_pending=true;}
bool unlocked(vk::command_buffer_chunk& cmd){return cmd.guard_mutex.readers==0 && !cmd.guard_mutex.writer;}
int main(){
    for(const auto status:{VK_TIMEOUT,VK_ERROR_OUT_OF_HOST_MEMORY,VK_ERROR_OUT_OF_DEVICE_MEMORY,VK_ERROR_DEVICE_LOST,VK_ERROR_UNKNOWN,VK_ERROR_VALIDATION_FAILED}){
        reset_test();vk::command_buffer_chunk cmd;pending(cmd);results={status};
        const auto returned=cmd.wait(37);const auto suffix=" ("+std::to_string(status)+")";
        check("finite wait preserves exact noncompletion result"+suffix,returned==status);
        check("failed wait retains pending event and reuse generation"+suffix,cmd.is_pending && cmd.eid_tag==41 && cmd.reset_id==7);
        check("failed wait does not reset its fence or notify collection"+suffix,fence_resets==0 && completed.empty() && unsafe_resets==0);
        check("failed wait releases the reader guard"+suffix,unlocked(cmd));
        results={VK_SUCCESS};const auto retried=cmd.wait(39);
        check("later completion retires the original event exactly once"+suffix,retried==VK_SUCCESS && !cmd.is_pending && cmd.eid_tag==0 && completed==std::vector<u64>({41}) && fence_resets==1);
        check("completed chunk skips subsequent fence waits"+suffix,cmd.wait(41)==VK_SUCCESS && wait_timeouts==std::vector<u64>({37,39}));
    }
    {
        reset_test();vk::command_buffer_chunk cmd;
        check("idle wait requires no fence or collection operation",cmd.wait(7)==VK_SUCCESS && wait_timeouts.empty() && completed.empty() && fence_resets==0);
        cmd.reset();check("idle reset advances generation and resets only its buffer",cmd.reset_id==1 && buffer_resets==1 && wait_timeouts.empty());
    }
    {
        reset_test();vk::command_buffer_chunk cmd;pending(cmd);results={VK_TIMEOUT,VK_SUCCESS};cmd.reset();
        check("reuse retries an expired bounded wait until completion",wait_timeouts==std::vector<u64>({FRAME_PRESENT_TIMEOUT,0}));
        check("reuse resets the buffer only after confirmed completion",buffer_resets==1 && fence_resets==1 && !cmd.is_pending && cmd.reset_id==8 && unsafe_resets==0 && completed==std::vector<u64>({41}));
    }
    for(const auto status:{VK_ERROR_DEVICE_LOST,VK_ERROR_OUT_OF_HOST_MEMORY}){
        for(const bool timed_out:{false,true}){
            reset_test();vk::command_buffer_chunk cmd;pending(cmd);results=timed_out?std::deque<VkResult>{VK_TIMEOUT,status}:std::deque<VkResult>{status};
            VkResult escaped=VK_SUCCESS;try{cmd.reset();}catch(const api_error& e){escaped=e.result;}
            const auto suffix=" ("+std::to_string(status)+(timed_out?", timeout retry)":", direct)");
            check("reuse propagates the real wait failure"+suffix,escaped==status);
            check("failed reuse retains the pending buffer and resources"+suffix,cmd.is_pending && cmd.eid_tag==41 && cmd.reset_id==7 && buffer_resets==0 && fence_resets==0 && completed.empty() && unlocked(cmd));
        }
    }
    {
        reset_test();vk::command_buffer_chunk cmd;pending(cmd);results={VK_TIMEOUT,VK_SUCCESS};
        vk::frame_context_t frame;frame.swap_command_buffer=&cmd;VKGSRender renderer;renderer.m_queued_frames.push_back(&frame);
        renderer.frame_context_cleanup(&frame);
        check("frame cleanup retries timeout before releasing resources",wait_timeouts==std::vector<u64>({FRAME_PRESENT_TIMEOUT,0}) && completed==std::vector<u64>({41}));
        check("completed frame releases heap and advances once",heap_restores==1 && globals_reset==1 && trims==1 && frame_advances==1 && renderer.m_queued_frames.empty() && !frame.swap_command_buffer && !renderer.swapchain_unavailable);
    }
    for(const auto status:{VK_ERROR_DEVICE_LOST,VK_ERROR_OUT_OF_HOST_MEMORY}){
        for(const bool timed_out:{false,true}){
            reset_test();vk::command_buffer_chunk cmd;pending(cmd);results=timed_out?std::deque<VkResult>{VK_TIMEOUT,status}:std::deque<VkResult>{status};
            vk::frame_context_t frame;frame.swap_command_buffer=&cmd;VKGSRender renderer;renderer.m_queued_frames.push_back(&frame);
            VkResult escaped=VK_SUCCESS;try{renderer.frame_context_cleanup(&frame);}catch(const api_error& e){escaped=e.result;}
            const auto suffix=" ("+std::to_string(status)+(timed_out?", timeout retry)":", direct)");
            check("frame cleanup propagates the real wait failure"+suffix,escaped==status);
            check("failed frame keeps its heap, queue and pending buffer"+suffix,cmd.is_pending && frame.swap_command_buffer==&cmd && renderer.m_queued_frames.size()==1 && !heap_restores && !globals_reset && !trims && !frame_advances && completed.empty());
        }
    }
    {
        reset_test();vk::command_buffer cmd;results={VK_TIMEOUT,VK_SUCCESS};restart_dma(cmd);
        check("DMA restart retries the bounded wait before resetting",wait_timeouts==std::vector<u64>({GENERAL_WAIT_TIMEOUT,0}) && buffer_resets==1 && buffer_begins==1 && !unsafe_resets && !unsafe_fence_destruction && vk::gc.held.empty());
    }
    for(const auto status:{VK_ERROR_DEVICE_LOST,VK_ERROR_OUT_OF_HOST_MEMORY}){
        for(const bool timed_out:{false,true}){
            reset_test();vk::command_buffer cmd;results=timed_out?std::deque<VkResult>{VK_TIMEOUT,status}:std::deque<VkResult>{status};
            VkResult escaped=VK_SUCCESS;try{restart_dma(cmd);}catch(const api_error& e){escaped=e.result;}
            const auto suffix=" ("+std::to_string(status)+(timed_out?", timeout retry)":", direct)");
            check("DMA restart propagates the real wait failure"+suffix,escaped==status);
            check("failed DMA cannot reset or begin its buffer"+suffix,!buffer_resets && !buffer_begins && !unsafe_resets);
            check("failed DMA retains its submitted fence for later collection"+suffix,vk::gc.held.size()==1 && vk::gc.held.front()->gpu_pending && !unsafe_fence_destruction);
        }
    }
    reset_test();
    std::cout<<checks<<" fence completion checks, "<<failures<<" failed\n";
    return failures?1:0;
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out / "fence-completion-audit.cpp"
    output.write_text(generate(args.source), encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
