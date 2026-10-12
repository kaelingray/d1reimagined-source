"""Generate CPU binding-contract fixtures from the actual private command-buffer cache."""
import argparse
from pathlib import Path
import re

from rpcs3_present_lifetime_audit import block


def generate(source, header):
    cpp, h = source.read_text(), header.read_text()
    declarations = h.split("// State cache", 1)[1].split("void clear_state_cache();", 1)[0]
    functions = []
    for match in re.finditer(r"void command_buffer::(?:clear_state_cache|bind_descriptor_sets)\(", cpp):
        start = cpp.index("{", match.start())
        functions.append(cpp[match.start():start] + block(cpp, start))
    if len(functions) != 3:
        raise ValueError("Expected reset and two descriptor-binding overloads")
    return r'''
#include <array>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <span>
#include <stdexcept>
#include <vector>
using u32 = std::uint32_t;
using VkDescriptorSet = std::uint64_t; using VkPipeline = std::uint64_t;
using VkPipelineLayout = std::uint64_t; using VkCommandBuffer = std::uint64_t;
enum VkPipelineBindPoint { VK_PIPELINE_BIND_POINT_GRAPHICS, VK_PIPELINE_BIND_POINT_COMPUTE };
constexpr std::uint64_t VK_NULL_HANDLE = 0;
void ensure(bool ok) { if (!ok) throw std::runtime_error("assertion"); }
template<class T> u32 size32(const T& t) { return static_cast<u32>(t.size()); }
struct binding { std::array<VkDescriptorSet,2> sets{}; VkPipelineLayout layout=0; std::vector<u32> offsets; };
std::array<binding,2> actual;
unsigned calls=0;
void vkCmdBindDescriptorSets(VkCommandBuffer, VkPipelineBindPoint point, VkPipelineLayout layout,
                            u32 first, u32 count, const VkDescriptorSet* sets, u32 offset_count, const u32* offsets) {
    ensure(first == 0 && count <= 2); ++calls;
    // A different layout conservatively disturbs earlier bindings. No Vulkan execution is simulated.
    auto& state=actual[point];
    if (layout != state.layout) state.sets.fill(0);
    state.layout=layout;
    std::copy_n(sets,count,state.sets.begin());
    state.offsets.clear();
    if (offset_count) state.offsets.assign(offsets,offsets+offset_count);
}
namespace vk {
struct command_buffer {
    bool is_open=true; VkCommandBuffer commands=1;
DECLARATIONS
    void clear_state_cache();
    void bind_descriptor_sets(const std::span<VkDescriptorSet>&, VkPipelineBindPoint, VkPipelineLayout) const;
    void bind_descriptor_sets(const std::span<VkDescriptorSet>&, const std::span<u32>&, VkPipelineBindPoint, VkPipelineLayout) const;
};
FUNCTIONS
}
int main() {
    int checks=0, failures=0;
    auto check=[&](const char* name,bool ok) { ++checks; if(!ok) { ++failures; std::cout<<"FAIL "<<name<<'\n'; } };
    const auto graphics=VK_PIPELINE_BIND_POINT_GRAPHICS, compute=VK_PIPELINE_BIND_POINT_COMPUTE;
    std::array<VkDescriptorSet,2> sets{11,12};
    std::span<VkDescriptorSet> both(sets), one(sets.data(),1);
    std::array<VkDescriptorSet,2> dynamic_sets{21,22};
    std::span<VkDescriptorSet> dynamic_both(dynamic_sets);
    std::array<u32,1> offsets{256}; std::span<u32> dynamic(offsets), empty;
    vk::command_buffer cmd;
    cmd.bind_descriptor_sets(both,graphics,100);
    check("initial bind reaches API",calls==1 && actual[graphics].sets==sets);
    cmd.bind_descriptor_sets(both,graphics,100);
    check("identical static binding stays cached",calls==1);
    cmd.bind_descriptor_sets(both,compute,100);
    check("compute receives its independent binding",actual[compute].sets==sets && actual[compute].layout==100);
    const auto before_graphics=calls;
    cmd.bind_descriptor_sets(both,graphics,100);
    check("compute binding preserves graphics cache",calls==before_graphics && actual[graphics].sets==sets);
    cmd.bind_descriptor_sets(both,graphics,200);
    check("changed pipeline layout reaches API",actual[graphics].layout==200);
    cmd.bind_descriptor_sets(one,graphics,300);
    check("single-set different layout reaches API",actual[graphics].layout==300);
    cmd.bind_descriptor_sets(both,graphics,200);
    check("higher set is rebound after layout disturbance",actual[graphics].layout==200 && actual[graphics].sets==sets);
    cmd.bind_descriptor_sets(dynamic_both,dynamic,graphics,400);
    check("dynamic offsets reach API",actual[graphics].offsets==std::vector<u32>{256});
    const auto before_dynamic=calls;
    offsets[0]=512;
    cmd.bind_descriptor_sets(dynamic_both,dynamic,graphics,400);
    check("dynamic binding is not elided",calls==before_dynamic+1);
    cmd.bind_descriptor_sets(both,graphics,200);
    check("different static set after dynamic call reaches API",actual[graphics].sets==sets && actual[graphics].offsets.empty());
    const auto before_empty=calls;
    cmd.bind_descriptor_sets(both,empty,graphics,200);
    check("empty dynamic overload shares static cache",calls==before_empty);
    cmd.clear_state_cache(); actual={};
    cmd.bind_descriptor_sets(both,graphics,200);
    check("reset forces graphics rebind",actual[graphics].sets==sets && actual[graphics].layout==200);
    cmd.bind_descriptor_sets(both,compute,100);
    check("reset forces compute rebind",actual[compute].sets==sets && actual[compute].layout==100);
    const auto final_calls=calls;
    cmd.bind_descriptor_sets(both,empty,compute,100);
    check("empty overload caches independent compute binding",calls==final_calls);
    std::cout<<checks<<" descriptor binding checks, "<<failures<<" failed\n";
    return failures ? 1 : 0;
}
'''.replace("DECLARATIONS", declarations).replace("FUNCTIONS", "\n".join(functions)).replace(
        "#include <array>", "#include <algorithm>\n#include <array>")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--header", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    text = generate(args.source, args.header)
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out / "descriptor-cache-audit.cpp"
    output.write_text(text, encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
